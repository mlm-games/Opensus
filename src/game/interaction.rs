use bevy_ecs::prelude::*;
use glam::Vec2;

use super::{
    ActiveSabotage, Alive, Ghost, MatchConfig, Player, PlayerIntent, Position, Role,
    SabotageFixStation, SabotageKind, TaskAssignments, TaskBoard, TaskStation,
};

fn reactor_fix_global(
    dt: f32,
    config: &MatchConfig,
    living: &Query<(&Player, &Role, &PlayerIntent, &Position), With<Alive>>,
    fix_stations: &mut Query<(Entity, &mut SabotageFixStation, &Position), Without<TaskStation>>,
) {
    let mut held: Vec<Entity> = Vec::new();
    let mut holder_ids: std::collections::HashSet<u64> = std::collections::HashSet::new();

    for (entity, station, position) in fix_stations.iter() {
        if station.kind != SabotageKind::Reactor || station.progress >= 1.0 {
            continue;
        }

        let position = position.0;

        let holders: Vec<u64> = living
            .iter()
            .filter(|(_, role, intent, player_position)| {
                matches!(role, Role::Crewmate)
                    && intent.interact
                    && player_position.0.distance(position) <= config.interact_range
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
        for (_, mut station, _) in fix_stations.iter_mut() {
            if station.kind == SabotageKind::Reactor {
                station.progress = 0.0;
            }
        }
        return;
    }

    for entity in held.iter().take(2) {
        if let Ok((_, mut station, _)) = fix_stations.get_mut(*entity) {
            station.progress += dt / config.sabotage_fix_time.max(0.1);
            if station.progress >= 1.0 {
                station.progress = 1.0;
            }
        }
    }
}

fn progress_fix_stations_once(
    dt: f32,
    config: &MatchConfig,
    active_kind: SabotageKind,
    living: &Query<(&Player, &Role, &PlayerIntent, &Position), With<Alive>>,
    fix_stations: &mut Query<(Entity, &mut SabotageFixStation, &Position), Without<TaskStation>>,
) {
    for (_, mut station, position) in fix_stations.iter_mut() {
        if station.kind != active_kind || station.progress >= 1.0 {
            continue;
        }

        let position = position.0;

        let holding = living.iter().any(|(_, role, intent, player_position)| {
            matches!(role, Role::Crewmate)
                && intent.interact
                && player_position.0.distance(position) <= config.interact_range
        });

        if !holding {
            continue;
        }

        station.progress += dt / config.sabotage_fix_time.max(0.1);

        if station.progress >= 1.0 {
            station.progress = 1.0;
        }
    }
}

fn progress_tasks_once(
    dt: f32,
    config: &MatchConfig,
    task_board: &mut TaskBoard,
    workers: &mut Query<
        (
            &Player,
            &Role,
            &PlayerIntent,
            &Position,
            &mut TaskAssignments,
        ),
        Or<(With<Alive>, With<Ghost>)>,
    >,
    task_stations: &Query<(&TaskStation, &Position), Without<SabotageFixStation>>,
) {
    let station_positions: Vec<(u32, Vec2)> = task_stations
        .iter()
        .map(|(station, position)| (station.id, position.0))
        .collect();

    for (_player, role, intent, position, mut assignments) in workers.iter_mut() {
        if !matches!(role, Role::Crewmate) || !intent.interact {
            assignments.clear_hold();
            continue;
        }

        let pos = position.0;

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

        let Some((task_id, _station_pos)) = nearest else {
            assignments.clear_hold();
            continue;
        };

        assignments.reset_hold_if_not(task_id);
        assignments.active_progress += dt / config.task_hold_time.max(0.1);

        if assignments.active_progress < 1.0 {
            continue;
        }

        if assignments.complete_active().is_some() && task_board.completed < task_board.total {
            task_board.completed = task_board.completed.saturating_add(1);
        }
    }
}

pub fn process_interactions(
    time: Res<repame_sim::SimTime>,
    config: Res<MatchConfig>,
    mut sabotage: ResMut<ActiveSabotage>,
    mut task_board: ResMut<TaskBoard>,
    living: Query<(&Player, &Role, &PlayerIntent, &Position), With<Alive>>,
    mut task_workers: Query<
        (
            &Player,
            &Role,
            &PlayerIntent,
            &Position,
            &mut TaskAssignments,
        ),
        Or<(With<Alive>, With<Ghost>)>,
    >,
    task_stations: Query<(&TaskStation, &Position), Without<SabotageFixStation>>,
    mut fix_stations: Query<(Entity, &mut SabotageFixStation, &Position), Without<TaskStation>>,
) {
    let dt = time.delta_secs;

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
            .filter(|(_, s, _)| s.kind == active_kind && s.progress >= 1.0)
            .count();
        sabotage.fixes_done = done.min(sabotage.fixes_needed as usize) as u8;
    }

    progress_tasks_once(
        dt,
        &config,
        &mut task_board,
        &mut task_workers,
        &task_stations,
    );
}
