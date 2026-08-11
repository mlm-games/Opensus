use bevy::prelude::*;

use super::MatchCleanup;
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
        app.add_systems(OnEnter(AppState::InGame), spawn_task_stations);
    }
}

fn spawn_task_stations(mut commands: Commands) {
    let stations = [
        (1, "Wire tap", Vec2::new(-280.0, 120.0)),
        (2, "Decode", Vec2::new(280.0, 120.0)),
        (3, "Stabilize", Vec2::new(-280.0, -120.0)),
        (4, "Scan", Vec2::new(280.0, -120.0)),
        (5, "Upload", Vec2::new(0.0, 40.0)),
    ];
    for (id, label, pos) in stations {
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
                    color: Color::srgb(0.35, 0.7, 0.45),
                    custom_size: Some(Vec2::splat(22.0)),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, 4.0),
            ))
            .id();
        Juice::pop_in(&mut commands, e, 0.2);
    }
}
