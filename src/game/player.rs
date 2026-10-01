use std::collections::HashSet;

use bevy_ecs::prelude::*;
use glam::Vec2;
use rand::RngExt;
use rand::seq::{IndexedRandom, SliceRandom};

use super::{
    ActiveSabotage, Alive, Body, EmergenciesLeft, EmergencyButton, EmergencyCooldownLeft,
    GamePhase, Ghost, KillCooldownLeft, KillRequest, KillRequests, LocalRole, MatchCleanup,
    MatchConfig, MatchRng, MatchStats, Position, ReportBodies, ReportBody, Role, SabotageCooldown,
    SabotageFixContribution, SabotageFixStation, SabotageRequests, SimTimer, SolidAabb,
    TaskAssignments, TaskStation, TimerMode, deterministic_task_ids,
};
use crate::save::SaveData;

#[derive(Component)]
pub struct Player {
    pub id: u64,
    pub name: String,
    #[allow(dead_code, reason = "Reserved for the network protocol")]
    pub color_index: u8,
    pub speed: f32,
}

#[derive(Component, Default, Clone, Copy, Debug)]
pub struct PlayerIntent {
    pub movement: Vec2,
    pub interact: bool,
}

#[derive(Component)]
pub struct LocalPlayer;

#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct LocalPlayerId(pub Option<u64>);

#[derive(Resource, Default, Clone)]
pub struct LocalPrompt(pub String);

/// Held-key intent captured from the platform layer each frame.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct LocalControls {
    pub direction: Vec2,
    pub interact: bool,
}

#[derive(Component)]
pub struct AiPlayer {
    pub repath: SimTimer,
    pub action: SimTimer,
    pub target_task: Option<Entity>,
    pub dir: Vec2,
    pub reported_this_body: bool,
}

/// A human-controlled remote client represented in the authoritative host world.
///
/// Its movement is applied from fixed-step network commands, not from the
/// normal latest-intent movement system.
#[derive(Component, Default)]
pub struct RemoteNetworkPlayer;

pub fn spawn_players_from_lobby(world: &mut World) {
    let save = world.resource::<SaveData>().clone();
    let cfg = world.resource::<MatchConfig>().clone();
    let mode = *world.resource::<crate::game::RuntimeMode>();
    let mut slots = world.resource::<super::LobbyState>().slots.clone();
    if slots.is_empty() {
        slots.push(super::LobbySlot {
            id: 1,
            name: save.player_name.clone(),
            color_index: save.preferred_color_index,
            ready: true,
            is_local: true,
            is_host: true,
            is_bot: false,
        });
        for i in 0..cfg.bot_count as u64 {
            slots.push(super::LobbySlot {
                id: 10 + i,
                name: format!("Agent-{}", i + 2),
                color_index: ((save.preferred_color_index as u64 + 1 + i)
                    % super::PLAYER_COLORS.len() as u64) as u8,
                ready: true,
                is_local: false,
                is_host: false,
                is_bot: true,
            });
        }
    }

    let n = slots.len();
    let imp_count = (cfg.impostor_count as usize)
        .min(n.saturating_sub(1))
        .max(if n >= 2 { 1 } else { 0 });
    let mut indices: Vec<usize> = (0..n).collect();
    {
        let mut rng = world.resource_mut::<MatchRng>();
        indices.shuffle(&mut rng.0);
    }
    let impostor_ids: HashSet<u64> = indices
        .into_iter()
        .take(imp_count)
        .map(|i| slots[i].id)
        .collect();

    let stats = MatchStats {
        players_spawned: slots.len() as u32,
        impostors_spawned: impostor_ids.len() as u32,
    };
    let mut task_board = super::TaskBoard {
        completed: 0,
        total: 0,
    };
    let tasks_per_player = cfg.tasks_per_crewmate as usize;
    let mut local_role = LocalRole(None);
    let mut local_player_id = LocalPlayerId(None);

    for (i, slot) in slots.iter().enumerate() {
        let role = if impostor_ids.contains(&slot.id) {
            Role::Impostor
        } else {
            Role::Crewmate
        };
        if slot.is_local {
            local_role.0 = Some(role);
            local_player_id.0 = Some(slot.id);
        }
        let pos = super::PLAYER_SPAWNS[i % super::PLAYER_SPAWNS.len()];

        let assigned_tasks = deterministic_task_ids(slot.id, i, tasks_per_player);
        if matches!(role, Role::Crewmate) {
            task_board.total = task_board.total.saturating_add(assigned_tasks.len() as u32);
        }

        let mut e = world.spawn((
            MatchCleanup,
            Player {
                id: slot.id,
                name: slot.name.clone(),
                color_index: slot.color_index,
                speed: cfg.player_speed,
            },
            role,
            Alive,
            PlayerIntent::default(),
            TaskAssignments::new(assigned_tasks),
            EmergenciesLeft(cfg.emergency_meetings),
            EmergencyCooldownLeft(cfg.emergency_cooldown),
            SabotageFixContribution::default(),
            Position(pos),
        ));

        if matches!(role, Role::Impostor) {
            e.insert(KillCooldownLeft(cfg.initial_kill_cooldown));
        }

        if slot.is_local {
            e.insert(LocalPlayer);
        } else if slot.is_bot {
            e.insert(AiPlayer {
                repath: SimTimer::from_seconds(0.4, TimerMode::Repeating),
                action: SimTimer::from_seconds(0.2, TimerMode::Repeating),
                target_task: None,
                dir: Vec2::ZERO,
                reported_this_body: false,
            });
        } else if matches!(mode, crate::game::RuntimeMode::Host) {
            e.insert(RemoteNetworkPlayer);
        }
    }

    world.insert_resource(local_role);
    world.insert_resource(local_player_id);
    world.insert_resource(stats);
    world.insert_resource(task_board);
}

pub fn input_direction(up: bool, down: bool, left: bool, right: bool) -> Vec2 {
    let mut direction = Vec2::ZERO;

    if up {
        direction.y += 1.0;
    }
    if down {
        direction.y -= 1.0;
    }
    if left {
        direction.x -= 1.0;
    }
    if right {
        direction.x += 1.0;
    }

    direction.normalize_or_zero()
}

pub fn local_intent_and_move(
    time: Res<repame_sim::SimTime>,
    controls: Res<LocalControls>,
    phase: Res<GamePhase>,
    mode: Res<crate::game::RuntimeMode>,
    cfg: Res<MatchConfig>,
    solids: Query<(&Position, &super::SolidAabb), Without<Player>>,
    mut living: Query<
        (&Player, &mut PlayerIntent, &mut Position),
        (With<LocalPlayer>, With<Alive>),
    >,
    mut ghosts: Query<
        (&Player, &mut PlayerIntent, &mut Position),
        (With<LocalPlayer>, With<Ghost>, Without<Alive>),
    >,
) {
    if !matches!(*phase, GamePhase::Playing) {
        if let Ok((_, mut intent, _)) = living.single_mut() {
            intent.movement = Vec2::ZERO;
            intent.interact = false;
        }

        if let Ok((_, mut intent, _)) = ghosts.single_mut() {
            intent.movement = Vec2::ZERO;
            intent.interact = false;
        }

        return;
    }

    let direction = controls.direction;
    let interact = controls.interact;

    if matches!(*mode, crate::game::RuntimeMode::Client) {
        if let Ok((_, mut intent, _)) = living.single_mut() {
            intent.movement = direction;
            intent.interact = interact;
        } else if let Ok((_, mut intent, _)) = ghosts.single_mut() {
            intent.movement = direction;
            intent.interact = interact;
        }

        return;
    }

    let boxes = super::collision::solid_boxes(&solids);

    if let Ok((player, mut intent, mut position)) = living.single_mut() {
        intent.movement = direction;
        intent.interact = interact;

        let stepped = super::collision::step_player_position(
            position.0,
            direction,
            player.speed,
            time.delta_secs,
            true,
            &boxes,
        );

        position.0 = stepped;
        return;
    }

    if let Ok((player, mut intent, mut position)) = ghosts.single_mut() {
        intent.movement = direction;
        intent.interact = interact;

        let stepped = super::collision::step_player_position(
            position.0,
            direction,
            player.speed * cfg.ghost_speed_mul,
            time.delta_secs,
            false,
            &boxes,
        );

        position.0 = stepped;
    }
}

pub fn apply_intent_movement(
    time: Res<repame_sim::SimTime>,
    mode: Res<crate::game::RuntimeMode>,
    cfg: Res<MatchConfig>,
    solids: Query<(&Position, &super::SolidAabb), Without<Player>>,
    mut living: Query<
        (&Player, &PlayerIntent, &mut Position),
        (
            With<Alive>,
            Without<LocalPlayer>,
            Without<RemoteNetworkPlayer>,
        ),
    >,
    mut ghosts: Query<
        (&Player, &PlayerIntent, &mut Position),
        (
            With<Ghost>,
            Without<Alive>,
            Without<LocalPlayer>,
            Without<RemoteNetworkPlayer>,
        ),
    >,
) {
    if matches!(*mode, crate::game::RuntimeMode::Client) {
        return;
    }

    let boxes = super::collision::solid_boxes(&solids);

    for (player, intent, mut position) in &mut living {
        let stepped = super::collision::step_player_position(
            position.0,
            intent.movement,
            player.speed,
            time.delta_secs,
            true,
            &boxes,
        );

        position.0 = stepped;
    }

    for (player, intent, mut position) in &mut ghosts {
        let stepped = super::collision::step_player_position(
            position.0,
            intent.movement,
            player.speed * cfg.ghost_speed_mul,
            time.delta_secs,
            false,
            &boxes,
        );

        position.0 = stepped;
    }
}

pub fn ai_brain(
    time: Res<repame_sim::SimTime>,
    cfg: Res<MatchConfig>,
    phase: Res<GamePhase>,
    mut kill_tx: ResMut<KillRequests>,
    mut report_tx: ResMut<ReportBodies>,
    mut sabo_tx: ResMut<SabotageRequests>,
    sabotage: Res<ActiveSabotage>,
    cooldown: Res<SabotageCooldown>,
    mut match_rng: ResMut<MatchRng>,
    tasks: Query<(Entity, &Position, &TaskStation)>,
    fix_stations: Query<(&SabotageFixStation, &Position)>,
    bodies: Query<(&Position, &Body)>,
    solids: Query<(&Position, &SolidAabb), Without<Player>>,
    mut ais: Query<
        (
            &Player,
            &Role,
            &TaskAssignments,
            &mut AiPlayer,
            &mut PlayerIntent,
            &Position,
            Option<&mut KillCooldownLeft>,
        ),
        With<Alive>,
    >,
) {
    if !matches!(*phase, GamePhase::Playing) {
        for (_, _, _, _, mut intent, _, _) in &mut ais {
            intent.movement = Vec2::ZERO;
            intent.interact = false;
        }
        return;
    }

    let boxes = super::collision::solid_boxes(&solids);
    let rng = &mut match_rng.0;

    for (player, role, tasks_for_player, mut ai, mut intent, position, kill_cd) in &mut ais {
        ai.repath.tick(time.delta_secs);
        ai.action.tick(time.delta_secs);
        let pos = position.0;

        let near_body = bodies
            .iter()
            .any(|(bt, b)| !b.reported && pos.distance(bt.0) <= cfg.bot_report_range);
        if near_body {
            if !ai.reported_this_body {
                report_tx.0.push(ReportBody {
                    reporter_id: player.id,
                });
                ai.reported_this_body = true;
            }
            intent.movement = Vec2::ZERO;
            intent.interact = false;
            continue;
        }
        ai.reported_this_body = false;

        if matches!(role, Role::Impostor) {
            let cd_ok = kill_cd.as_ref().is_some_and(|cd| cd.0 <= 0.0);
            let chance = cfg.bot_kill_aggression * time.delta_secs * 2.0;
            if cd_ok && rng.random::<f32>() < chance {
                kill_tx.0.push(KillRequest {
                    actor_id: player.id,
                });
            }
        }

        if matches!(role, Role::Impostor)
            && !sabotage.is_active()
            && cooldown.remaining <= 0.0
            && ai.action.just_finished()
            && rng.random::<f32>() < 0.08
        {
            let kind = *[
                super::SabotageKind::Lights,
                super::SabotageKind::Oxygen,
                super::SabotageKind::Reactor,
            ]
            .choose(rng)
            .unwrap();
            sabo_tx.0.push(super::SabotageRequest {
                actor_id: player.id,
                kind,
            });
        }

        if matches!(role, Role::Crewmate)
            && let Some(kind) = sabotage.kind
        {
            let mut best: Option<(f32, Vec2)> = None;
            for (station, station_position) in &fix_stations {
                if station.kind != kind || station.progress >= 1.0 {
                    continue;
                }
                let d = pos.distance(station_position.0);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, station_position.0));
                }
            }
            if let Some((dist, target)) = best {
                if dist <= cfg.interact_range * 0.85 {
                    intent.movement = Vec2::ZERO;
                    intent.interact = true;
                } else {
                    let wp = super::navigation::next_waypoint(pos, target, &boxes);
                    intent.movement = (wp - pos).normalize_or_zero();
                    intent.interact = false;
                }
                continue;
            }
        }

        if ai.repath.just_finished() || ai.target_task.is_none() {
            let mut best: Option<(Entity, f32)> = None;
            for (te, tt, st) in &tasks {
                if !tasks_for_player.has(st.id) || tasks_for_player.is_done(st.id) {
                    continue;
                }

                if matches!(role, Role::Impostor) && rng.random::<f32>() > 0.35 {
                    continue;
                }

                let d = pos.distance(tt.0);
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((te, d));
                }
            }
            ai.target_task = best.map(|(e, _)| e);
            if ai.target_task.is_none() {
                let angle = rng.random::<f32>() * std::f32::consts::TAU;
                ai.dir = Vec2::new(angle.cos(), angle.sin());
            }
        }

        if let Some(tid) = ai.target_task {
            if let Ok((_, tt, st)) = tasks.get(tid) {
                if !tasks_for_player.has(st.id) || tasks_for_player.is_done(st.id) {
                    ai.target_task = None;
                    intent.interact = false;
                    intent.movement = ai.dir;
                    continue;
                }
                let target = tt.0;
                let dist = pos.distance(target);
                if dist <= cfg.interact_range * 0.85 {
                    intent.movement = Vec2::ZERO;
                    intent.interact = matches!(role, Role::Crewmate);
                } else {
                    let wp = super::navigation::next_waypoint(pos, target, &boxes);
                    intent.movement = (wp - pos).normalize_or_zero();
                    intent.interact = false;
                }
            } else {
                ai.target_task = None;
            }
        } else {
            intent.movement = ai.dir;
            intent.interact = false;
        }
    }
}

pub fn ai_ghost_brain(
    time: Res<repame_sim::SimTime>,
    cfg: Res<MatchConfig>,
    phase: Res<GamePhase>,
    mut match_rng: ResMut<MatchRng>,
    tasks: Query<(Entity, &Position, &TaskStation)>,
    mut ais: Query<
        (
            &Role,
            &TaskAssignments,
            &mut AiPlayer,
            &mut PlayerIntent,
            &Position,
        ),
        (With<Ghost>, With<AiPlayer>, Without<Alive>),
    >,
) {
    if !matches!(*phase, GamePhase::Playing) {
        for (_, _, _, mut intent, _) in &mut ais {
            intent.movement = Vec2::ZERO;
            intent.interact = false;
        }
        return;
    }

    let rng = &mut match_rng.0;

    for (role, tasks_for_player, mut ai, mut intent, position) in &mut ais {
        if !matches!(role, Role::Crewmate) {
            intent.movement = Vec2::ZERO;
            intent.interact = false;
            continue;
        }

        ai.repath.tick(time.delta_secs);
        let pos = position.0;

        if ai.repath.just_finished() || ai.target_task.is_none() {
            let mut best: Option<(Entity, f32)> = None;
            for (te, tt, st) in &tasks {
                if !tasks_for_player.has(st.id) || tasks_for_player.is_done(st.id) {
                    continue;
                }

                let d = pos.distance(tt.0);
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((te, d));
                }
            }
            ai.target_task = best.map(|(e, _)| e);
            if ai.target_task.is_none() {
                let angle = rng.random::<f32>() * std::f32::consts::TAU;
                ai.dir = Vec2::new(angle.cos(), angle.sin());
            }
        }

        if let Some(tid) = ai.target_task {
            if let Ok((_, tt, st)) = tasks.get(tid) {
                if !tasks_for_player.has(st.id) || tasks_for_player.is_done(st.id) {
                    ai.target_task = None;
                    intent.interact = false;
                    intent.movement = ai.dir;
                    continue;
                }
                let delta = tt.0 - pos;
                if delta.length() <= cfg.interact_range * 0.85 {
                    intent.movement = Vec2::ZERO;
                    intent.interact = true;
                } else {
                    intent.movement = delta.normalize_or_zero();
                    intent.interact = false;
                }
            } else {
                ai.target_task = None;
                intent.movement = ai.dir;
                intent.interact = false;
            }
        } else {
            intent.movement = ai.dir;
            intent.interact = false;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_local_prompt(
    cfg: Res<MatchConfig>,
    phase: Res<GamePhase>,
    sabotage: Res<ActiveSabotage>,
    local: Query<
        (
            &Player,
            &Position,
            Option<&Alive>,
            Option<&Role>,
            Option<&KillCooldownLeft>,
            Option<&TaskAssignments>,
        ),
        With<LocalPlayer>,
    >,
    bodies: Query<(&Position, &Body)>,
    tasks: Query<(&Position, &TaskStation)>,
    buttons: Query<&Position, (With<EmergencyButton>, Without<Player>)>,
    fix: Query<(&Position, &SabotageFixStation)>,
    targets: Query<(&Player, &Position, &Role), (With<Alive>, Without<LocalPlayer>)>,
    mut prompt: ResMut<LocalPrompt>,
) {
    prompt.0.clear();

    if !matches!(*phase, GamePhase::Playing) {
        return;
    }

    let Ok((me, position, alive, role, kill_cd, assignments)) = local.single() else {
        return;
    };

    let pos = position.0;

    if alive.is_some() {
        if bodies
            .iter()
            .any(|(bt, b)| !b.reported && pos.distance(bt.0) <= cfg.report_range)
        {
            prompt.0 = "R - Report body".into();
            return;
        }

        if matches!(role, Some(Role::Impostor)) {
            let cd = kill_cd.map(|c| c.0).unwrap_or(0.0);
            if cd <= 0.0 {
                let mut best: Option<(&str, f32)> = None;

                for (target, target_position, target_role) in &targets {
                    if target.id == me.id || matches!(target_role, Role::Impostor) {
                        continue;
                    }

                    let d = pos.distance(target_position.0);
                    if d <= cfg.kill_range && best.is_none_or(|(_, bd)| d < bd) {
                        best = Some((target.name.as_str(), d));
                    }
                }

                if let Some((name, _)) = best {
                    prompt.0 = format!("Q - Kill {name}");
                    return;
                }
            } else {
                prompt.0 = format!("Kill cooldown: {:.0}s", cd);
            }
        }

        if let Some(kind) = sabotage.kind
            && fix.iter().any(|(ft, s)| {
                s.kind == kind && s.progress < 1.0 && pos.distance(ft.0) <= cfg.interact_range
            })
        {
            prompt.0 = "E - Hold to fix sabotage".into();
            return;
        }

        if buttons
            .iter()
            .any(|bt| pos.distance(bt.0) <= cfg.interact_range)
        {
            prompt.0 = "F - Emergency meeting".into();
            return;
        }
    }

    let Some(assignments) = assignments else {
        return;
    };

    if tasks.iter().any(|(tt, station)| {
        assignments.has(station.id)
            && !assignments.is_done(station.id)
            && pos.distance(tt.0) <= cfg.interact_range
    }) {
        prompt.0 = "E - Hold to complete task".into();
    }
}
