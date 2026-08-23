use bevy::prelude::*;

use super::{
    ActiveSabotage, Alive, GamePhase, Ghost, MatchConfig, Player, PlayerIntent, Role,
    SabotageFixStation, SabotageKind, TaskBoard, TaskStation,
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

/// Reactor is a simultaneous dual-hold: both consoles must be held in the same
/// frame by two distinct living crewmates. A single player cannot clear it.
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

/// Progress each active O2/Lights console once per frame while at least one
/// living crewmate is holding near it — extra holders never speed it up.
///
/// One player may legitimately run both O2 consoles (serially); ghosts cannot
/// fix sabotages.
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

/// Progress every incomplete task once per frame when at least one eligible
/// worker (living or crewmate-ghost) holds near it.
fn progress_tasks_once(
    dt: f32,
    config: &MatchConfig,
    commands: &mut Commands,
    task_board: &mut TaskBoard,
    living: &Query<(&Player, &Role, &PlayerIntent, &Transform), With<Alive>>,
    ghosts: &Query<(&Player, &Role, &PlayerIntent, &Transform), (With<Ghost>, Without<Alive>)>,
    task_stations: &mut Query<
        (Entity, &mut TaskStation, &mut Sprite, &Transform),
        Without<SabotageFixStation>,
    >,
) {
    let mut completed_positions = Vec::new();

    for (_, mut station, mut sprite, transform) in task_stations.iter_mut() {
        if station.done {
            continue;
        }

        let position = transform.translation.truncate();

        let living_worker = living.iter().any(|(_, role, intent, player_transform)| {
            matches!(role, Role::Crewmate)
                && intent.interact
                && player_transform.translation.truncate().distance(position)
                    <= config.interact_range
        });

        let ghost_worker = ghosts.iter().any(|(_, role, intent, player_transform)| {
            matches!(role, Role::Crewmate)
                && intent.interact
                && player_transform.translation.truncate().distance(position)
                    <= config.interact_range
        });

        if !living_worker && !ghost_worker {
            continue;
        }

        station.progress += dt / config.task_hold_time.max(0.1);

        if station.progress < 1.0 {
            continue;
        }

        station.progress = 1.0;
        station.done = true;
        sprite.color = Color::srgba(0.55, 0.55, 0.55, 0.85);

        // Never count past the win threshold (extra stations / double-complete).
        if task_board.completed < task_board.total {
            task_board.completed = task_board.completed.saturating_add(1);
        }

        completed_positions.push(position);
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
    ghosts: Query<(&Player, &Role, &PlayerIntent, &Transform), (With<Ghost>, Without<Alive>)>,
    mut fix_stations: Query<
        (Entity, &mut SabotageFixStation, &mut Sprite, &Transform),
        Without<TaskStation>,
    >,
    mut task_stations: Query<
        (Entity, &mut TaskStation, &mut Sprite, &Transform),
        Without<SabotageFixStation>,
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

        // Recount completed stations instead of incrementing on complete:
        // survives partial re-entry / frame glitches without double-counting.
        let done = fix_stations
            .iter()
            .filter(|(_, s, _, _)| s.kind == active_kind && s.progress >= 1.0)
            .count();
        sabotage.fixes_done = done.min(sabotage.fixes_needed as usize) as u8;
    }

    // Ghosts keep completing tasks (Among Us style); crewmate ghosts only.
    progress_tasks_once(
        dt,
        &config,
        &mut commands,
        &mut task_board,
        &living,
        &ghosts,
        &mut task_stations,
    );
}
