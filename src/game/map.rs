use bevy::prelude::*;

use super::{GameAssets, MatchCleanup};
use crate::app::AppState;

#[derive(Component)]
pub struct MapRoot;

#[derive(Component)]
pub struct Room {
    #[allow(dead_code, reason = "Reserved for the network protocol")]
    pub name: &'static str,
}

/// Map-mounted emergency call button. Emergencies may only be called by a
/// living player standing within `interact_range` of one of these.
#[derive(Component)]
pub struct EmergencyButton;

pub struct MapPlugin;
impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), spawn_map);
    }
}

fn spawn_map(mut commands: Commands, assets: Res<GameAssets>) {
    // Floor
    commands.spawn((
        MatchCleanup,
        MapRoot,
        Sprite {
            image: assets.floor_wood.clone(),
            custom_size: Some(Vec2::new(1100.0, 640.0)),
            image_mode: SpriteImageMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: 64.0,
            },
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    let rooms = [
        ("Archives", Vec2::new(-280.0, 120.0), Vec2::new(220.0, 160.0), true),
        ("Comms", Vec2::new(280.0, 120.0), Vec2::new(220.0, 160.0), true),
        ("Reactor", Vec2::new(-280.0, -120.0), Vec2::new(220.0, 160.0), false),
        ("Medbay", Vec2::new(280.0, -120.0), Vec2::new(220.0, 160.0), true),
        ("Cafeteria", Vec2::new(0.0, 0.0), Vec2::new(240.0, 160.0), false),
    ];

    for (name, pos, size, use_carpet) in rooms {
        let floor = if use_carpet {
            assets.floor_carpet.clone()
        } else {
            assets.floor_wood.clone()
        };

        commands.spawn((
            MatchCleanup,
            Room { name },
            Sprite {
                image: floor,
                custom_size: Some(size),
                image_mode: SpriteImageMode::Tiled {
                    tile_x: true,
                    tile_y: true,
                    stretch_value: 48.0,
                },
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 1.0),
        ));

        // Simple wall strips (top + sides)
        let half = size * 0.5;
        commands.spawn((
            MatchCleanup,
            Sprite {
                image: assets.wall_front.clone(),
                custom_size: Some(Vec2::new(size.x, 18.0)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y + half.y, 2.0),
        ));
        for x_sign in [-1.0, 1.0] {
            commands.spawn((
                MatchCleanup,
                Sprite {
                    image: assets.wall_side.clone(),
                    custom_size: Some(Vec2::new(14.0, size.y)),
                    ..default()
                },
                Transform::from_xyz(pos.x + x_sign * half.x, pos.y, 2.0),
            ));
        }
    }

    // Cafeteria props
    commands.spawn((
        MatchCleanup,
        Sprite {
            image: assets.table.clone(),
            custom_size: Some(Vec2::new(90.0, 60.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 3.0),
    ));
    for (x, y) in [(-50.0, -40.0), (50.0, -40.0), (-50.0, 40.0), (50.0, 40.0)] {
        commands.spawn((
            MatchCleanup,
            Sprite {
                image: assets.seat.clone(),
                custom_size: Some(Vec2::new(28.0, 28.0)),
                ..default()
            },
            Transform::from_xyz(x, y, 3.0),
        ));
    }

    // Emergency button in the cafeteria (map-gated "F" in handle_meeting_commands).
    commands.spawn((
        MatchCleanup,
        EmergencyButton,
        Sprite {
            color: Color::srgb(0.85, 0.25, 0.2),
            custom_size: Some(Vec2::splat(26.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -40.0, 6.0),
    ));

    // Bounds
    for y in [320.0, -320.0] {
        commands.spawn((
            MatchCleanup,
            Sprite {
                image: assets.wall_front.clone(),
                color: Color::srgba(1.0, 1.0, 1.0, 0.85),
                custom_size: Some(Vec2::new(1120.0, 12.0)),
                ..default()
            },
            Transform::from_xyz(0.0, y, 2.5),
        ));
    }
}