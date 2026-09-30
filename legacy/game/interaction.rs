use bevy::prelude::*;

use super::{
    ActiveSabotage, Alive, GamePhase, Ghost, MatchConfig, Player, PlayerIntent, Role,
    SabotageFixStation, SabotageKind, TaskAssignments, TaskBoard, TaskStation,
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

fn progress_tasks_once(
    dt: f32,
    config: &MatchConfig,
    commands: &mut Commands,
    task_board: &mut TaskBoard,
    workers: &mut Query<
        (
            &Player,
            &Role,
            &PlayerIntent,
            &Transform,
            &mut TaskAssignments,
        ),
        Or<(With<Alive>, With<Ghost>)>,
    >,
    task_stations: &Query<(&TaskStation, &Transform), Without<SabotageFixStation>>,
) {
    let station_positions: Vec<(u32, Vec2)> = task_stations
        .iter()
        .map(|(station, transform)| (station.id, transform.translation.truncate()))
        .collect();

    let mut completed_positions = Vec::new();

    for (_player, role, intent, transform, mut assignments) in workers.iter_mut() {
        if !matches!(role, Role::Crewmate) || !intent.interact {
            assignments.clear_hold();
            continue;
        }

        let pos = transform.translation.truncate();

        let nearest = station_positions
            .iter()
            .copied()
            .filter(|(id, station_pos)| {
                assignments.has(*id)
                    && !assignments.is_done(*id)
                    && pos.distance(*station_pos) <= config.interact_range
            })
            .min_by(|a, b| {
                pos.distance(a.1)
                    .partial_cmp(&pos.distance(b.1))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

        let Some((task_id, station_pos)) = nearest else {
            assignments.clear_hold();
            continue;
        };

        assignments.reset_hold_if_not(task_id);
        assignments.active_progress += dt / config.task_hold_time.max(0.1);

        if assignments.active_progress < 1.0 {
            continue;
        }

        if assignments.complete_active().is_some() {
            if task_board.completed < task_board.total {
                task_board.completed = task_board.completed.saturating_add(1);
            }

            completed_positions.push(station_pos);
        }
    }

    for position in completed_positions {
        VfxSpawner::spawn_burst(
            commands,
            position,
            10,
            Color::srgb(0.4, 0.9, 0.5),
            (30.0, 80.0),
        );
    }
}

fn process_interactions(
    time: Res<Time>,
    config: Res<MatchConfig>,
    mut commands: Commands,
    mut sabotage: ResMut<ActiveSabotage>,
    mut task_board: ResMut<TaskBoard>,
    living: Query<(&Player, &Role, &PlayerIntent, &Transform), With<Alive>>,
    mut task_workers: Query<
        (
            &Player,
            &Role,
            &PlayerIntent,
            &Transform,
            &mut TaskAssignments,
        ),
        Or<(With<Alive>, With<Ghost>)>,
    >,
    task_stations: Query<(&TaskStation, &Transform), Without<SabotageFixStation>>,
    mut fix_stations: Query<
        (Entity, &mut SabotageFixStation, &mut Sprite, &Transform),
        Without<TaskStation>,
    >,
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

    progress_tasks_once(
        dt,
        &config,
        &mut commands,
        &mut task_board,
        &mut task_workers,
        &task_stations,
    );
}
