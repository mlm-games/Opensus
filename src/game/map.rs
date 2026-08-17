use bevy::prelude::*;

use super::{GameAssets, MatchCleanup, SolidAabb};
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
        (
            "Archives",
            Vec2::new(-280.0, 120.0),
            Vec2::new(220.0, 160.0),
            true,
        ),
        (
            "Comms",
            Vec2::new(280.0, 120.0),
            Vec2::new(220.0, 160.0),
            true,
        ),
        (
            "Reactor",
            Vec2::new(-280.0, -120.0),
            Vec2::new(220.0, 160.0),
            false,
        ),
        (
            "Medbay",
            Vec2::new(280.0, -120.0),
            Vec2::new(220.0, 160.0),
            true,
        ),
        (
            "Cafeteria",
            Vec2::new(0.0, 0.0),
            Vec2::new(240.0, 160.0),
            false,
        ),
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

        spawn_room_walls(&mut commands, &assets, pos, size);
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
        spawn_wall(
            &mut commands,
            &assets,
            Vec2::new(0.0, y),
            Vec2::new(1120.0, 12.0),
            2.5,
        );
    }
}

fn spawn_wall(commands: &mut Commands, assets: &GameAssets, pos: Vec2, size: Vec2, z: f32) {
    commands.spawn((
        MatchCleanup,
        SolidAabb {
            half_extents: size * 0.5,
        },
        Sprite {
            image: if size.x >= size.y {
                assets.wall_front.clone()
            } else {
                assets.wall_side.clone()
            },
            color: Color::srgba(1.0, 1.0, 1.0, 0.85),
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(pos.x, pos.y, z),
    ));
}

fn spawn_room_walls(commands: &mut Commands, assets: &GameAssets, center: Vec2, size: Vec2) {
    let half = size * 0.5;
    let gaps = room_gaps(center);
    let wall_z = 2.0;

    // Horizontal walls (top/bottom): segments along x, y fixed.
    for (y_off, which) in [(half.y, "top"), (-half.y, "bottom")] {
        let y = center.y + y_off;
        for (a, b) in split_spans(-half.x, half.x, gaps.get(which).map(Vec::as_slice)) {
            let x = (a + b) * 0.5;
            spawn_wall(
                commands,
                assets,
                Vec2::new(x, y),
                Vec2::new(b - a, 18.0),
                wall_z,
            );
        }
    }

    // Vertical walls (left/right): segments along y, x fixed.
    for (x_off, which) in [(-half.x, "left"), (half.x, "right")] {
        let x = center.x + x_off;
        for (a, b) in split_spans(-half.y, half.y, gaps.get(which).map(Vec::as_slice)) {
            let y = (a + b) * 0.5;
            spawn_wall(
                commands,
                assets,
                Vec2::new(x, y),
                Vec2::new(14.0, b - a),
                wall_z,
            );
        }
    }
}

fn room_gaps(center: Vec2) -> std::collections::HashMap<&'static str, Vec<(f32, f32)>> {
    let mut gaps = std::collections::HashMap::<&'static str, Vec<(f32, f32)>>::new();
    match (center.x, center.y) {
        // Cafeteria - hub, connects to all four rooms.
        (0.0, 0.0) => {
            gaps.insert("bottom", vec![(-70.0, -10.0), (10.0, 70.0)]);
            gaps.insert("left", vec![(10.0, 70.0)]);
            gaps.insert("right", vec![(10.0, 70.0)]);
        }
        // Archives - connects to Cafeteria (east) and Reactor (south).
        (-280.0, 120.0) => {
            gaps.insert("bottom", vec![(-40.0, 20.0)]);
            gaps.insert("right", vec![(40.0, 90.0)]);
        }
        // Comms - connects to Cafeteria (west) and Medbay (south).
        (280.0, 120.0) => {
            gaps.insert("bottom", vec![(-20.0, 50.0)]);
            gaps.insert("left", vec![(40.0, 90.0)]);
        }
        // Reactor - connects to Cafeteria (north) and Archives (north).
        (-280.0, -120.0) => {
            gaps.insert("top", vec![(-40.0, 20.0)]);
        }
        // Medbay - connects to Cafeteria (north) and Comms (north).
        (280.0, -120.0) => {
            gaps.insert("top", vec![(30.0, 90.0)]);
        }
        _ => {}
    }
    gaps
}

fn split_spans(start: f32, end: f32, gaps: Option<&[(f32, f32)]>) -> Vec<(f32, f32)> {
    let Some(gaps) = gaps else {
        return vec![(start, end)];
    };
    let mut result = Vec::new();
    let mut cursor = start;
    let mut sorted: Vec<(f32, f32)> = gaps.to_vec();
    sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    for (ga, gb) in sorted {
        let ga = ga.max(start);
        let gb = gb.min(end);
        if ga <= cursor || gb <= ga {
            continue;
        }
        result.push((cursor, ga));
        cursor = gb;
    }
    if cursor < end {
        result.push((cursor, end));
    }
    result
}
