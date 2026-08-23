use bevy::prelude::*;

use super::{GameAssets, MatchCleanup, TASK_STATIONS};
use crate::app::AppState;
use game_utils_bevy::juice::Juice;

#[derive(Resource, Default)]
pub struct TaskBoard {
    pub completed: u32,
    pub total: u32,
}

#[derive(Component)]
pub struct TaskStation {
    #[allow(dead_code, reason = "Reserved for the network protocol")]
    pub id: u32,
    #[allow(dead_code, reason = "Reserved for the HUD and network protocol")]
    pub label: &'static str,
    pub progress: f32,
    pub done: bool,
}

pub struct TasksPlugin;

impl Plugin for TasksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::InGame),
            spawn_task_stations.after(super::setup_match),
        );
    }
}

fn spawn_task_stations(
    mut commands: Commands,
    assets: Res<GameAssets>,
    config: Res<super::MatchConfig>,
    mut board: ResMut<TaskBoard>,
) {
    let images = [
        assets.task_beaker.clone(),
        assets.task_flask.clone(),
        assets.task_burner.clone(),
        assets.task_flask.clone(),
        assets.task_beaker.clone(),
    ];

    board.total = config.tasks_to_win.min(TASK_STATIONS.len() as u32).max(1);

    board.completed = 0;

    // Spawn every available task. The configured threshold determines how
    // many must be completed, rather than deleting map content.
    for ((id, label, position), image) in TASK_STATIONS.into_iter().zip(images) {
        let entity = commands
            .spawn((
                MatchCleanup,
                TaskStation {
                    id,
                    label,
                    progress: 0.0,
                    done: false,
                },
                Sprite {
                    image,
                    custom_size: Some(Vec2::splat(28.0)),
                    ..default()
                },
                Transform::from_xyz(position.x, position.y, 4.0),
            ))
            .id();

        Juice::pop_in(&mut commands, entity, 0.2);
    }
}
