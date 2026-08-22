use bevy::prelude::*;

use super::{GameAssets, MatchCleanup};
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
    cfg: Res<super::MatchConfig>,
    mut board: ResMut<TaskBoard>,
) {
    let stations = [
        (
            1,
            "Wire tap",
            Vec2::new(-280.0, 120.0),
            assets.task_beaker.clone(),
        ),
        (
            2,
            "Decode",
            Vec2::new(280.0, 120.0),
            assets.task_flask.clone(),
        ),
        (
            3,
            "Stabilize",
            Vec2::new(-280.0, -120.0),
            assets.task_burner.clone(),
        ),
        (
            4,
            "Scan",
            Vec2::new(280.0, -120.0),
            assets.task_flask.clone(),
        ),
        (
            5,
            "Upload",
            Vec2::new(0.0, 40.0),
            assets.task_beaker.clone(),
        ),
    ];

    // Win threshold = min(config, available stations) so the bar is reachable
    // and never requires more completions than exist.
    let available = stations.len() as u32;
    board.total = cfg.tasks_to_win.min(available).max(1);
    board.completed = 0;

    for (id, label, pos, image) in stations.into_iter().take(board.total as usize) {
        let e = commands
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
                Transform::from_xyz(pos.x, pos.y, 4.0),
            ))
            .id();
        Juice::pop_in(&mut commands, e, 0.2);
    }
}
