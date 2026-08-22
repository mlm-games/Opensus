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

/// Reactor is a simultaneous dual-hold: both consoles must be held by two
/// living crewmates in the same frame. A single player cannot clear it.
fn reactor_fix_global(
    dt: f32,
    config: &MatchConfig,
    sabotage: &mut ActiveSabotage,
    living: &Query<(&Player, &Role, &PlayerIntent, &Transform), With<Alive>>,
    fix_stations: &mut Query<
        (Entity, &mut SabotageFixStation, &mut Sprite, &Transform),
        Without<TaskStation>,
    >,
) {
    let held: Vec<Entity> = fix_stations
        .iter()
        .filter(|(_, station, _, _)| {
            station.kind == SabotageKind::Reactor && station.progress < 1.0
        })
        .filter(|(_, _, _, transform)| {
            living.iter().any(|(_, role, intent, pt)| {
                matches!(role, Role::Crewmate)
                    && intent.interact
                    && pt
                        .translation
                        .truncate()
                        .distance(transform.translation.truncate())
                        <= config.interact_range
            })
        })
        .map(|(entity, _, _, _)| entity)
        .collect();

    if held.len() < 2 {
        for (_, mut station, mut sprite, _) in fix_stations.iter_mut() {
            if station.kind == SabotageKind::Reactor {
                station.progress = 0.0;
                sprite.color = Color::srgba(1.0, 0.6, 0.15, 1.0);
            }
        }
        sabotage.fixes_done = 0;
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
    let done = fix_stations
        .iter()
        .filter(|(_, s, _, _)| s.kind == SabotageKind::Reactor && s.progress >= 1.0)
        .count();
    sabotage.fixes_done = done.min(2) as u8;
}

fn try_complete_task(
    dt: f32,
    config: &MatchConfig,
    commands: &mut Commands,
    task_board: &mut TaskBoard,
    task_stations: &mut Query<
        (Entity, &mut TaskStation, &mut Sprite, &Transform),
        Without<SabotageFixStation>,
    >,
    player_position: Vec2,
) {
    let mut nearest: Option<(Entity, f32)> = None;

    for (entity, station, _, transform) in task_stations.iter() {
        if station.done {
            continue;
        }

        let distance = player_position.distance(transform.translation.truncate());

        if distance > config.interact_range {
            continue;
        }

        if nearest.is_none_or(|(_, nearest_distance)| distance < nearest_distance) {
            nearest = Some((entity, distance));
        }
    }

    let Some((entity, _)) = nearest else {
        return;
    };

    let Ok((_, mut station, mut sprite, transform)) = task_stations.get_mut(entity) else {
        return;
    };

    station.progress += dt / config.task_hold_time.max(0.1);

    if station.progress < 1.0 {
        return;
    }

    station.progress = 1.0;
    station.done = true;
    sprite.color = Color::srgba(0.55, 0.55, 0.55, 0.85);

    // Never count past the win threshold (extra stations / double-complete).
    if task_board.completed < task_board.total {
        task_board.completed = task_board.completed.saturating_add(1);
    }

    VfxSpawner::spawn_burst(
        commands,
        transform.translation.truncate(),
        10,
        Color::srgb(0.4, 0.9, 0.5),
        (30.0, 80.0),
    );
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

    // Reactor: global simultaneous dual-hold, once per frame.
    if matches!(sabotage.kind, Some(SabotageKind::Reactor)) {
        reactor_fix_global(dt, &config, &mut sabotage, &living, &mut fix_stations);
    }

    for (_player, role, intent, player_transform) in &living {
        if !intent.interact || !matches!(role, Role::Crewmate) {
            continue;
        }

        let player_position = player_transform.translation.truncate();

        // Priority one: repair active non-reactor sabotage (O2/Lights) —
        // serial is correct there (one player can run both O2 consoles).
        if let Some(active_kind) = sabotage.kind
            && !matches!(active_kind, SabotageKind::Reactor)
        {
            let mut fixed_station = false;

            for (_, mut station, mut sprite, transform) in &mut fix_stations {
                if station.kind != active_kind || station.progress >= 1.0 {
                    continue;
                }

                let distance = player_position.distance(transform.translation.truncate());

                if distance > config.interact_range {
                    continue;
                }

                station.progress += dt / config.sabotage_fix_time.max(0.1);

                if station.progress >= 1.0 {
                    station.progress = 1.0;
                    sprite.color = Color::srgba(0.45, 0.75, 0.5, 0.9);
                }

                fixed_station = true;
                break;
            }

            if fixed_station {
                continue;
            }
        }

        // Priority two: nearest incomplete task.
        try_complete_task(
            dt,
            &config,
            &mut commands,
            &mut task_board,
            &mut task_stations,
            player_position,
        );
    }

    // Recount completed stations instead of incrementing on complete: survives
    // partial re-entry / frame glitches without double-counting.
    if let Some(active_kind) = sabotage.kind
        && !matches!(active_kind, SabotageKind::Reactor)
    {
        let done = fix_stations
            .iter()
            .filter(|(_, s, _, _)| s.kind == active_kind && s.progress >= 1.0)
            .count();
        sabotage.fixes_done = done.min(sabotage.fixes_needed as usize) as u8;
    }

    // Ghosts keep completing tasks (Among Us style); crewmate ghosts only.
    for (_player, role, intent, player_transform) in &ghosts {
        if !intent.interact || !matches!(role, Role::Crewmate) {
            continue;
        }
        try_complete_task(
            dt,
            &config,
            &mut commands,
            &mut task_board,
            &mut task_stations,
            player_transform.translation.truncate(),
        );
    }
}
