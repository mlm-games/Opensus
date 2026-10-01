use std::collections::HashSet;

use bevy_ecs::prelude::*;
use glam::Vec2;

use super::{
    Alive, Body, GamePhase, Ghost, KillCooldownLeft, LocalPlayer, MatchCleanup, MatchConfig,
    MeetingCommand, MeetingCommands, MeetingState, Player, Position, Role, SolidAabb, TaskBoard,
    Trauma, make_ghost,
};
use crate::app::InputEdges;
use crate::save::SaveData;

#[derive(Clone, Copy, Debug)]
pub struct KillRequest {
    pub actor_id: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct ReportBody {
    pub reporter_id: u64,
}

#[derive(Resource, Default)]
pub struct KillRequests(pub Vec<KillRequest>);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RumbleRequest {
    pub strong: f32,
    pub weak: f32,
    pub duration_ms: u32,
}

#[derive(Resource, Default)]
pub struct PendingRumble(pub Vec<RumbleRequest>);

#[derive(Resource, Default)]
pub struct ReportBodies(pub Vec<ReportBody>);

/// Consume the local device's one-frame action edges (Q/R/F) into durable
/// request queues. Edges expire every tick (legacy ButtonInput parity), and
/// requests are only enqueued while the match is in open play.
pub fn read_local_action_edges(
    mut edges: ResMut<InputEdges>,
    phase: Res<GamePhase>,
    local: Query<&Player, (With<LocalPlayer>, With<Alive>)>,
    mut kills: ResMut<KillRequests>,
    mut reports: ResMut<ReportBodies>,
    mut meeting: ResMut<MeetingCommands>,
) {
    let (kill, report, emergency) = (edges.kill, edges.report, edges.emergency);
    edges.kill = false;
    edges.report = false;
    edges.emergency = false;

    if !kill && !report && !emergency {
        return;
    }
    if !matches!(*phase, GamePhase::Playing) {
        return;
    }
    let Ok(player) = local.single() else {
        return;
    };

    if kill {
        kills.0.push(KillRequest {
            actor_id: player.id,
        });
    }
    if report {
        reports.0.push(ReportBody {
            reporter_id: player.id,
        });
    }
    if emergency {
        meeting.0.push(MeetingCommand::Emergency {
            actor_id: player.id,
        });
    }
}

pub fn tick_kill_cds(time: Res<repame_sim::SimTime>, mut q: Query<&mut KillCooldownLeft>) {
    for mut cd in &mut q {
        cd.0 = (cd.0 - time.delta_secs).max(0.0);
    }
}

pub fn do_kill(
    mut requests: ResMut<KillRequests>,
    mut commands: Commands,
    cfg: Res<MatchConfig>,
    mut phase: ResMut<GamePhase>,
    tasks: Res<TaskBoard>,
    stats: Res<super::MatchStats>,
    mut save: ResMut<SaveData>,
    mut trauma: ResMut<Trauma>,
    mut rumble: ResMut<PendingRumble>,
    mut actors: Query<(&Position, &Role, &Player, &mut KillCooldownLeft), With<Alive>>,
    targets: Query<(Entity, &Player, &Position, &Role), With<Alive>>,
    solids: Query<(&Position, &SolidAabb)>,
) {
    let requests = std::mem::take(&mut requests.0);
    if matches!(*phase, GamePhase::GameOver { .. } | GamePhase::None) {
        return;
    }

    let solid_boxes: Vec<(Vec2, Vec2)> = solids
        .iter()
        .map(|(position, solid)| (position.0, solid.half_extents))
        .collect();

    let mut killed_this_frame = HashSet::<Entity>::new();

    for request in requests {
        if !matches!(*phase, GamePhase::Playing) {
            break;
        }

        let actor_id = request.actor_id;

        let Some((actor_position, actor_role, _, mut cd)) = actors
            .iter_mut()
            .find(|(_, _, player, _)| player.id == actor_id)
        else {
            continue;
        };

        if cd.0 > 0.0 || !matches!(actor_role, Role::Impostor) {
            continue;
        }

        let actor_position = actor_position.0;
        let mut best: Option<(Entity, Vec2, u64, String)> = None;
        let mut best_d = cfg.kill_range;
        for (e, p, t, r) in &targets {
            if p.id == actor_id {
                continue; // no self kill
            }
            if matches!(r, Role::Impostor) {
                continue; // no team kill in v1
            }
            let target_position = t.0;
            let d = actor_position.distance(target_position);
            if d < best_d
                && crate::game::collision::segment_clear(
                    actor_position,
                    target_position,
                    &solid_boxes,
                    2.0,
                )
            {
                best_d = d;
                best = Some((e, target_position, p.id, p.name.clone()));
            }
        }
        let Some((victim, pos, id, name)) = best else {
            continue;
        };
        if killed_this_frame.contains(&victim) {
            continue;
        }
        killed_this_frame.insert(victim);
        cd.0 = cfg.kill_cooldown;

        make_ghost(&mut commands, victim);

        let shove = (pos - actor_position).normalize_or_zero() * 8.0;
        let body_pos = pos + shove + Vec2::new(0.0, -10.0);

        commands.spawn((
            MatchCleanup,
            Body {
                player_id: id,
                name,
                reported: false,
            },
            Position(body_pos),
        ));
        trauma.add(0.55);
        rumble.0.push(RumbleRequest {
            strong: 0.9,
            weak: 0.5,
            duration_ms: 200,
        });

        let mut crew = 0u32;
        let mut imps = 0u32;
        for (e, _, _, r) in &targets {
            if e == victim {
                continue;
            }
            match r {
                Role::Crewmate => crew += 1,
                Role::Impostor => imps += 1,
            }
        }
        if let Some(reason) = super::compute_win_from_counts(&tasks, crew, imps, &stats) {
            super::apply_game_over(&mut phase, reason, &mut save);
            break;
        }
    }
}

pub fn do_report(
    mut requests: ResMut<ReportBodies>,
    mut phase: ResMut<GamePhase>,
    mut meeting: ResMut<MeetingState>,
    config: Res<MatchConfig>,
    mut sabotage: ResMut<super::ActiveSabotage>,
    mut fix_stations: Query<&mut super::SabotageFixStation>,
    reporters: Query<(&Player, &Position), With<Alive>>,
    mut bodies: Query<(Entity, &mut Body, &Position)>,
    players: Query<(&Player, Option<&Alive>, Option<&Ghost>)>,
    mut trauma: ResMut<Trauma>,
) {
    let requests = std::mem::take(&mut requests.0);
    for request in requests {
        if !matches!(*phase, GamePhase::Playing) {
            continue;
        }

        let Some((_, reporter_position)) = reporters
            .iter()
            .find(|(player, _)| player.id == request.reporter_id)
        else {
            continue;
        };

        let reporter_position = reporter_position.0;

        let nearest = bodies
            .iter_mut()
            .filter(|(_, body, _)| !body.reported)
            .filter_map(|(entity, body, position)| {
                let distance = reporter_position.distance(position.0);

                (distance <= config.report_range).then_some((entity, body.name.clone(), distance))
            })
            .min_by(|left, right| {
                left.2
                    .partial_cmp(&right.2)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

        let Some((body_entity, victim_name, _)) = nearest else {
            continue;
        };

        if let Ok((_, mut body, _)) = bodies.get_mut(body_entity) {
            body.reported = true;
        }

        super::clear_sabotage_world(&mut sabotage, &mut fix_stations);
        trauma.add(0.4);

        meeting.begin_meeting(
            format!("{victim_name}'s body was reported!"),
            &players,
            config.discussion_time,
        );

        *phase = GamePhase::Meeting;
    }
}
