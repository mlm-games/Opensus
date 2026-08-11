use bevy::prelude::*;

use super::MatchCleanup;
use crate::app::AppState;

#[derive(Component)]
pub struct MapRoot;

#[derive(Component)]
pub struct Room {
    #[allow(dead_code, reason = "Reserved for the network protocol")]
    pub name: &'static str,
}

pub struct MapPlugin;
impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), spawn_placeholder_map);
    }
}

fn spawn_placeholder_map(mut commands: Commands) {
    // Floor
    commands.spawn((
        MatchCleanup,
        MapRoot,
        Sprite {
            color: Color::srgb(0.12, 0.16, 0.14),
            custom_size: Some(Vec2::new(1100.0, 640.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
    // Simple rooms as tinted panels
    let rooms = [
        (
            "Archives",
            Vec2::new(-280.0, 120.0),
            Vec2::new(220.0, 160.0),
            Color::srgb(0.18, 0.22, 0.2),
        ),
        (
            "Comms",
            Vec2::new(280.0, 120.0),
            Vec2::new(220.0, 160.0),
            Color::srgb(0.2, 0.18, 0.22),
        ),
        (
            "Reactor",
            Vec2::new(-280.0, -120.0),
            Vec2::new(220.0, 160.0),
            Color::srgb(0.22, 0.18, 0.16),
        ),
        (
            "Medbay",
            Vec2::new(280.0, -120.0),
            Vec2::new(220.0, 160.0),
            Color::srgb(0.16, 0.2, 0.24),
        ),
        (
            "Cafeteria",
            Vec2::new(0.0, 0.0),
            Vec2::new(200.0, 140.0),
            Color::srgb(0.2, 0.2, 0.18),
        ),
    ];
    for (name, pos, size, color) in rooms {
        commands.spawn((
            MatchCleanup,
            Room { name },
            Sprite {
                color,
                custom_size: Some(size),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 1.0),
        ));
    }
    // Bounds visual
    commands.spawn((
        MatchCleanup,
        Sprite {
            color: Color::srgba(0.4, 0.1, 0.1, 0.35),
            custom_size: Some(Vec2::new(1120.0, 8.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 320.0, 2.0),
    ));
    commands.spawn((
        MatchCleanup,
        Sprite {
            color: Color::srgba(0.4, 0.1, 0.1, 0.35),
            custom_size: Some(Vec2::new(1120.0, 8.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -320.0, 2.0),
    ));
}
