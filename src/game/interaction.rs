use bevy::prelude::*;

use super::{
    ActiveSabotage, Alive, GamePhase, MatchConfig, Player, PlayerIntent, Role, SabotageFixStation,
    TaskBoard, TaskStation,
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
                .run_if(in_state(AppState::InGame))
                .run_if(|paused: Res<Paused>| !paused.0)
                .run_if(|transition: Res<Transition<AppState>>| !transition.block_input)
                .run_if(|phase: Res<GamePhase>| matches!(*phase, GamePhase::Playing))
                .run_if(super::has_authority),
        );
    }
}

fn process_interactions(
    time: Res<Time>,
    config: Res<MatchConfig>,
    mut commands: Commands,
    mut sabotage: ResMut<ActiveSabotage>,
    mut task_board: ResMut<TaskBoard>,
    players: Query<(&Player, &Role, &PlayerIntent, &Transform), With<Alive>>,
    mut fix_stations: Query<
        (&mut SabotageFixStation, &mut Sprite, &Transform),
        Without<TaskStation>,
    >,
    mut task_stations: Query<
        (Entity, &mut TaskStation, &mut Sprite, &Transform),
        Without<SabotageFixStation>,
    >,
) {
    for (_player, role, intent, player_transform) in &players {
        if !intent.interact || !matches!(role, Role::Crewmate) {
            continue;
        }

        let player_position = player_transform.translation.truncate();

        // Priority one: repair active sabotage.
        if let Some(active_kind) = sabotage.kind {
            let mut fixed_station = false;

            for (mut station, mut sprite, transform) in &mut fix_stations {
                if station.kind != active_kind || station.progress >= 1.0 {
                    continue;
                }

                let distance = player_position.distance(transform.translation.truncate());

                if distance > config.interact_range {
                    continue;
                }

                station.progress += time.delta_secs() / config.sabotage_fix_time.max(0.1);

                if station.progress >= 1.0 {
                    station.progress = 1.0;
                    sprite.color = Color::srgba(0.45, 0.75, 0.5, 0.9);
                    sabotage.fixes_done = sabotage.fixes_done.saturating_add(1);
                }

                fixed_station = true;
                break;
            }

            if fixed_station {
                continue;
            }
        }

        // Priority two: nearest incomplete task.
        let mut nearest: Option<(Entity, f32)> = None;

        for (entity, station, _, transform) in &task_stations {
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
            continue;
        };

        let Ok((_, mut station, mut sprite, transform)) = task_stations.get_mut(entity) else {
            continue;
        };

        station.progress += time.delta_secs() / config.task_hold_time.max(0.1);

        if station.progress < 1.0 {
            continue;
        }

        station.progress = 1.0;
        station.done = true;
        sprite.color = Color::srgba(0.55, 0.55, 0.55, 0.85);

        task_board.completed = task_board.completed.saturating_add(1);

        VfxSpawner::spawn_burst(
            &mut commands,
            transform.translation.truncate(),
            10,
            Color::srgb(0.4, 0.9, 0.5),
            (30.0, 80.0),
        );
    }
}
