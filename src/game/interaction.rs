use bevy::prelude::*;

use super::{
    ActiveSabotage, Alive, GamePhase, Ghost, MatchConfig, Player, PlayerIntent, PlayerTasks, Role,
    SabotageFixStation, SabotageKind, TaskBoard, TaskKind, TaskStation,
};
use crate::app::{AppState, Paused};
use game_utils_bevy::transitions::Transition;
use game_utils_bevy::vfx::VfxSpawner;

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            process_interactions
                .in_set(super::ResolveStep::Interact)
                .run_if(in_state(AppState::InGame))
                .run_if(|paused: Res<Paused>| !paused.0)
                .run_if(|transition: Res<Transition<AppState>>| !transition.block_input)
                .run_if(|phase: Res<GamePhase>| matches!(*phase, GamePhase::Playing))
                .run_if(super::has_authority),
        );
    }
}

fn reactor_fix_global(
    dt: f32,
    config: &MatchConfig,
    living: &Query<(&Player, &Role, &PlayerIntent, &Transform), With<Alive>>,
    fix_stations: &mut Query<
        (Entity, &mut SabotageFixStation, &mut Sprite, &Transform),
        Without<TaskStation>,
    >,
) {
    let mut held: Vec<Entity> = Vec::new();
    let mut holder_ids: std::collections::HashSet<u64> = std::collections::HashSet::new();

    for (entity, station, _, transform) in fix_stations.iter() {
        if station.kind != SabotageKind::Reactor || station.progress >= 1.0 {
            continue;
        }

        let position = transform.translation.truncate();

        let holders: Vec<u64> = living
            .iter()
            .filter(|(_, role, intent, player_transform)| {
                matches!(role, Role::Crewmate)
                    && intent.interact
                    && player_transform.translation.truncate().distance(position)
                        <= config.interact_range
            })
            .map(|(player, _, _, _)| player.id)
            .collect();

        if holders.is_empty() {
            continue;
        }

        held.push(entity);
        holder_ids.extend(holders);
    }

    if held.len() < 2 || holder_ids.len() < 2 {
        for (_, mut station, mut sprite, _) in fix_stations.iter_mut() {
            if station.kind == SabotageKind::Reactor {
                station.progress = 0.0;
                sprite.color = Color::srgba(1.0, 0.6, 0.15, 1.0);
            }
        }
        return;
    }

    for entity in held.iter().take(2) {
        if let Ok((_, mut station, mut sprite, _)) = fix_stations.get_mut(*entity) {
            station.progress += dt / config.sabotage_fix_time.max(0.1);
            if station.progress >= 1.0 {
                station.progress = 1.0;
                sprite.color = Color::srgba(0.45, 0.75, 0.5, 0.9);
            }
        }
    }
}

fn progress_fix_stations_once(
    dt: f32,
    config: &MatchConfig,
    active_kind: SabotageKind,
    living: &Query<(&Player, &Role, &PlayerIntent, &Transform), With<Alive>>,
    fix_stations: &mut Query<
        (Entity, &mut SabotageFixStation, &mut Sprite, &Transform),
        Without<TaskStation>,
    >,
) {
    for (_, mut station, mut sprite, transform) in fix_stations.iter_mut() {
        if station.kind != active_kind || station.progress >= 1.0 {
            continue;
        }

        let position = transform.translation.truncate();

        let holding = living.iter().any(|(_, role, intent, player_transform)| {
            matches!(role, Role::Crewmate)
                && intent.interact
                && player_transform.translation.truncate().distance(position)
                    <= config.interact_range
        });

        if !holding {
            continue;
        }

        station.progress += dt / config.sabotage_fix_time.max(0.1);

        if station.progress >= 1.0 {
            station.progress = 1.0;
            sprite.color = Color::srgba(0.45, 0.75, 0.5, 0.9);
        }
    }
}

fn process_interactions(
    time: Res<Time>,
    config: Res<MatchConfig>,
    mut commands: Commands,
    mut sabotage: ResMut<ActiveSabotage>,
    mut task_board: ResMut<TaskBoard>,
    living: Query<(&Player, &Role, &PlayerIntent, &Transform), With<Alive>>,
    mut players_with_tasks: Query<
        (&Player, &Role, &PlayerIntent, &Transform, &mut PlayerTasks),
        Or<(With<Alive>, With<Ghost>)>,
    >,
    solids: Query<(&Transform, &super::SolidAabb), Without<Player>>,
    mut fix_stations: Query<
        (Entity, &mut SabotageFixStation, &mut Sprite, &Transform),
        Without<TaskStation>,
    >,
    task_stations: Query<(Entity, &Transform, &TaskStation), Without<SabotageFixStation>>,
) {
    let dt = time.delta_secs();

    if let Some(active_kind) = sabotage.kind {
        match active_kind {
            SabotageKind::Reactor => {
                reactor_fix_global(dt, &config, &living, &mut fix_stations);
            }
            _ => {
                progress_fix_stations_once(dt, &config, active_kind, &living, &mut fix_stations);
            }
        }

        let done = fix_stations
            .iter()
            .filter(|(_, s, _, _)| s.kind == active_kind && s.progress >= 1.0)
            .count();
        sabotage.fixes_done = done.min(sabotage.fixes_needed as usize) as u8;
    }

    let solid_boxes = solids
        .iter()
        .map(|(t, s)| (t.translation.truncate(), s.half_extents))
        .collect::<Vec<_>>();

    // Per-player task progression: only closest assigned station.
    let mut completed_positions = Vec::new();
    for (_player, role, intent, transform, mut player_tasks) in &mut players_with_tasks {
        if !matches!(role, Role::Crewmate) {
            continue;
        }
        let player_position = transform.translation.truncate();

        let target_station = task_stations
            .iter()
            .filter(|(_, _, station)| {
                player_tasks
                    .items
                    .iter()
                    .any(|task| task.station_id == station.id && !task.done)
            })
            .filter_map(|(entity, trans, station)| {
                let station_pos = trans.translation.truncate();
                let distance = player_position.distance(station_pos);
                if distance > config.interact_range {
                    return None;
                }
                // LOS validation: don't allow through walls.
                if !crate::game::collision::segment_clear(
                    player_position,
                    station_pos,
                    &solid_boxes,
                    2.0,
                ) {
                    return None;
                }
                Some((entity, distance, station))
            })
            .min_by(|left, right| {
                left.1
                    .partial_cmp(&right.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

        if let Some((_, _, station)) = target_station {
            if let Some(task) = player_tasks.get_mut(station.id) {
                if intent.interact {
                    let duration = match station.kind {
                        TaskKind::ShortHold => config.task_hold_time,
                        TaskKind::LongHold => config.task_hold_time * 1.8,
                        TaskKind::TwoStage => config.task_hold_time * 1.25,
                    };

                    task.progress =
                        (task.progress + time.delta_secs() / duration.max(0.1)).min(1.0);

                    if task.progress >= 1.0 && !task.done {
                        if matches!(station.kind, TaskKind::TwoStage) && task.stage == 0 {
                            task.stage = 1;
                            task.progress = 0.0;
                        } else {
                            task.done = true;
                            task_board.completed = task_board.completed.saturating_add(1);
                            completed_positions.push(player_position);
                        }
                    }
                } else {
                    task.progress = (task.progress - time.delta_secs() * 0.35).max(0.0);
                }
            }
        } else {
            // No target in range: decay all incomplete task progress slightly? Only decay active if previously progressing - simplified to no decay.
            // To match spec, decay only when interacting elsewhere? We'll decay if interact false? Spec decays when not interacting after targeting; but if no target, nothing.
        }
    }

    for position in completed_positions {
        VfxSpawner::spawn_burst(
            &mut commands,
            position,
            10,
            Color::srgb(0.4, 0.9, 0.5),
            (30.0, 80.0),
        );
    }
}
