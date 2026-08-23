use super::{
    ARCHIVES_CENTER, ARCHIVES_SIZE, BORDER_THICKNESS, BRIEFING_CENTER, BRIEFING_SIZE, COMMS_CENTER,
    COMMS_SIZE, CORRIDORS, CorridorAxis, ELECTRICAL_CENTER, ELECTRICAL_SIZE,
    EMERGENCY_BUTTON_POSITION, GameAssets, MAP_BOUNDS, MAP_FLOOR_SIZE, MEDBAY_CENTER, MEDBAY_SIZE,
    MatchCleanup, REACTOR_CENTER, REACTOR_SIZE, STORAGE_CENTER, STORAGE_SIZE, SolidAabb,
};
use crate::app::AppState;
use bevy::prelude::*;

const ROOM_WALL_THICKNESS: f32 = 14.0;
const DOOR_THRESHOLD_THICKNESS: f32 = 10.0;
const CORRIDOR_WALL_THICKNESS: f32 = 10.0;

#[derive(Component)]
pub struct MapRoot;

#[derive(Component)]
pub struct Room {
    #[allow(dead_code, reason = "Reserved for the network protocol")]
    pub name: &'static str,
}

/// Map-mounted emergency call button.
///
/// A living player must stand within `interact_range` before calling a meeting.
#[derive(Component)]
pub struct EmergencyButton;

#[derive(Clone, Copy)]
enum FloorKind {
    Wood,
    Carpet,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Clone, Copy)]
struct Doorway {
    side: Side,

    /// Offset along the wall: X for top/bottom, Y for left/right.
    offset: f32,

    width: f32,
}

#[derive(Clone, Copy)]
struct RoomSpec {
    name: &'static str,
    center: Vec2,
    size: Vec2,
    floor: FloorKind,
    tint: (f32, f32, f32),
    doors: &'static [Doorway],
}

const BRIEFING_DOORS: [Doorway; 6] = [
    Doorway {
        side: Side::Left,
        offset: 52.0,
        width: 72.0,
    },
    Doorway {
        side: Side::Left,
        offset: -52.0,
        width: 72.0,
    },
    Doorway {
        side: Side::Right,
        offset: 52.0,
        width: 72.0,
    },
    Doorway {
        side: Side::Right,
        offset: -52.0,
        width: 72.0,
    },
    Doorway {
        side: Side::Top,
        offset: 0.0,
        width: 84.0,
    },
    Doorway {
        side: Side::Bottom,
        offset: 0.0,
        width: 84.0,
    },
];

const ARCHIVES_DOORS: [Doorway; 2] = [
    Doorway {
        side: Side::Right,
        offset: -68.0,
        width: 72.0,
    },
    Doorway {
        side: Side::Bottom,
        offset: 0.0,
        width: 76.0,
    },
];

const COMMS_DOORS: [Doorway; 2] = [
    Doorway {
        side: Side::Left,
        offset: -68.0,
        width: 72.0,
    },
    Doorway {
        side: Side::Bottom,
        offset: 0.0,
        width: 76.0,
    },
];

const REACTOR_DOORS: [Doorway; 2] = [
    Doorway {
        side: Side::Right,
        offset: 68.0,
        width: 72.0,
    },
    Doorway {
        side: Side::Top,
        offset: 0.0,
        width: 76.0,
    },
];

const MEDBAY_DOORS: [Doorway; 2] = [
    Doorway {
        side: Side::Left,
        offset: 68.0,
        width: 72.0,
    },
    Doorway {
        side: Side::Top,
        offset: 0.0,
        width: 76.0,
    },
];

const ELECTRICAL_DOORS: [Doorway; 1] = [Doorway {
    side: Side::Bottom,
    offset: 0.0,
    width: 84.0,
}];

const STORAGE_DOORS: [Doorway; 1] = [Doorway {
    side: Side::Top,
    offset: 0.0,
    width: 84.0,
}];

const ROOMS: [RoomSpec; 7] = [
    RoomSpec {
        name: "Briefing",
        center: BRIEFING_CENTER,
        size: BRIEFING_SIZE,
        floor: FloorKind::Carpet,
        tint: (0.72, 0.80, 0.84),
        doors: &BRIEFING_DOORS,
    },
    RoomSpec {
        name: "Archives",
        center: ARCHIVES_CENTER,
        size: ARCHIVES_SIZE,
        floor: FloorKind::Carpet,
        tint: (0.72, 0.66, 0.58),
        doors: &ARCHIVES_DOORS,
    },
    RoomSpec {
        name: "Comms",
        center: COMMS_CENTER,
        size: COMMS_SIZE,
        floor: FloorKind::Carpet,
        tint: (0.58, 0.72, 0.78),
        doors: &COMMS_DOORS,
    },
    RoomSpec {
        name: "Reactor",
        center: REACTOR_CENTER,
        size: REACTOR_SIZE,
        floor: FloorKind::Wood,
        tint: (0.72, 0.58, 0.50),
        doors: &REACTOR_DOORS,
    },
    RoomSpec {
        name: "Medbay",
        center: MEDBAY_CENTER,
        size: MEDBAY_SIZE,
        floor: FloorKind::Carpet,
        tint: (0.66, 0.78, 0.72),
        doors: &MEDBAY_DOORS,
    },
    RoomSpec {
        name: "Electrical",
        center: ELECTRICAL_CENTER,
        size: ELECTRICAL_SIZE,
        floor: FloorKind::Wood,
        tint: (0.72, 0.68, 0.48),
        doors: &ELECTRICAL_DOORS,
    },
    RoomSpec {
        name: "Storage",
        center: STORAGE_CENTER,
        size: STORAGE_SIZE,
        floor: FloorKind::Wood,
        tint: (0.62, 0.58, 0.52),
        doors: &STORAGE_DOORS,
    },
];

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), spawn_map);
    }
}

fn spawn_map(mut commands: Commands, assets: Res<GameAssets>) {
    spawn_backdrop(&mut commands);
    spawn_base_floor(&mut commands, &assets);
    spawn_corridors(&mut commands, &assets);

    for room in ROOMS {
        spawn_room(&mut commands, &assets, room);
    }

    spawn_outer_walls(&mut commands, &assets);
    spawn_briefing_table(&mut commands, &assets);
    spawn_emergency_button(&mut commands);
    spawn_wayfinding_lights(&mut commands);
}

fn spawn_backdrop(commands: &mut Commands) {
    commands.spawn((
        MatchCleanup,
        MapRoot,
        Sprite {
            color: Color::srgb(0.025, 0.035, 0.045),
            custom_size: Some(MAP_FLOOR_SIZE + Vec2::splat(56.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, -1.0),
    ));
}

fn spawn_base_floor(commands: &mut Commands, assets: &GameAssets) {
    commands.spawn((
        MatchCleanup,
        Sprite {
            image: assets.floor_wood.clone(),
            color: Color::srgb(0.52, 0.55, 0.58),
            custom_size: Some(MAP_FLOOR_SIZE),
            image_mode: SpriteImageMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: 64.0,
            },
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}

/// Spawn each corridor as painted flooring plus physical side walls, so the
/// walkable space between rooms is exactly the painted corridor.
fn spawn_corridors(commands: &mut Commands, assets: &GameAssets) {
    for corridor in CORRIDORS {
        commands.spawn((
            MatchCleanup,
            Sprite {
                image: assets.floor_carpet.clone(),
                color: Color::srgb(0.42, 0.50, 0.56),
                custom_size: Some(corridor.size),
                image_mode: SpriteImageMode::Tiled {
                    tile_x: true,
                    tile_y: true,
                    stretch_value: 40.0,
                },
                ..default()
            },
            Transform::from_xyz(corridor.center.x, corridor.center.y, 0.6),
        ));

        let thickness = CORRIDOR_WALL_THICKNESS;

        match corridor.axis {
            CorridorAxis::Horizontal => {
                let offset = corridor.size.y * 0.5 + thickness * 0.5;

                for sign in [-1.0, 1.0] {
                    spawn_wall(
                        commands,
                        assets,
                        corridor.center + Vec2::new(0.0, sign * offset),
                        Vec2::new(corridor.size.x, thickness),
                        2.15,
                    );
                }
            }
            CorridorAxis::Vertical => {
                let offset = corridor.size.x * 0.5 + thickness * 0.5;

                for sign in [-1.0, 1.0] {
                    spawn_wall(
                        commands,
                        assets,
                        corridor.center + Vec2::new(sign * offset, 0.0),
                        Vec2::new(thickness, corridor.size.y),
                        2.15,
                    );
                }
            }
        }
    }
}

fn spawn_room(commands: &mut Commands, assets: &GameAssets, room: RoomSpec) {
    let image = match room.floor {
        FloorKind::Wood => assets.floor_wood.clone(),
        FloorKind::Carpet => assets.floor_carpet.clone(),
    };

    commands.spawn((
        MatchCleanup,
        Room { name: room.name },
        Sprite {
            image,
            color: Color::srgb(room.tint.0, room.tint.1, room.tint.2),
            custom_size: Some(room.size),
            image_mode: SpriteImageMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: 48.0,
            },
            ..default()
        },
        Transform::from_xyz(room.center.x, room.center.y, 1.0),
    ));

    spawn_room_walls(commands, assets, room);
    spawn_room_label(commands, room);
}

fn spawn_room_walls(commands: &mut Commands, assets: &GameAssets, room: RoomSpec) {
    let half = room.size * 0.5;

    for side in [Side::Top, Side::Bottom] {
        let y = room.center.y + if side == Side::Top { half.y } else { -half.y };

        let gaps = doorway_spans(room, side);

        for (start, end) in split_spans(-half.x, half.x, &gaps) {
            spawn_wall(
                commands,
                assets,
                Vec2::new(room.center.x + (start + end) * 0.5, y),
                Vec2::new(end - start, ROOM_WALL_THICKNESS),
                2.2,
            );
        }
    }

    for side in [Side::Left, Side::Right] {
        let x = room.center.x + if side == Side::Right { half.x } else { -half.x };

        let gaps = doorway_spans(room, side);

        for (start, end) in split_spans(-half.y, half.y, &gaps) {
            spawn_wall(
                commands,
                assets,
                Vec2::new(x, room.center.y + (start + end) * 0.5),
                Vec2::new(ROOM_WALL_THICKNESS, end - start),
                2.2,
            );
        }
    }

    for doorway in room.doors {
        spawn_door_threshold(commands, assets, room, *doorway);
    }
}

fn doorway_spans(room: RoomSpec, side: Side) -> Vec<(f32, f32)> {
    room.doors
        .iter()
        .filter(|door| door.side == side)
        .map(|door| {
            (
                door.offset - door.width * 0.5,
                door.offset + door.width * 0.5,
            )
        })
        .collect()
}

fn split_spans(start: f32, end: f32, gaps: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut gaps = gaps.to_vec();

    gaps.sort_by(|left, right| {
        left.0
            .partial_cmp(&right.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut spans = Vec::new();
    let mut cursor = start;

    for (gap_start, gap_end) in gaps {
        let gap_start = gap_start.clamp(start, end);
        let gap_end = gap_end.clamp(start, end);

        if gap_end <= cursor || gap_end <= gap_start {
            continue;
        }

        if gap_start > cursor {
            spans.push((cursor, gap_start));
        }

        cursor = cursor.max(gap_end);
    }

    if cursor < end {
        spans.push((cursor, end));
    }

    spans
}

fn spawn_door_threshold(
    commands: &mut Commands,
    assets: &GameAssets,
    room: RoomSpec,
    doorway: Doorway,
) {
    let half = room.size * 0.5;

    let (position, size) = match doorway.side {
        Side::Top => (
            room.center + Vec2::new(doorway.offset, half.y),
            Vec2::new(doorway.width, DOOR_THRESHOLD_THICKNESS),
        ),
        Side::Bottom => (
            room.center + Vec2::new(doorway.offset, -half.y),
            Vec2::new(doorway.width, DOOR_THRESHOLD_THICKNESS),
        ),
        Side::Left => (
            room.center + Vec2::new(-half.x, doorway.offset),
            Vec2::new(DOOR_THRESHOLD_THICKNESS, doorway.width),
        ),
        Side::Right => (
            room.center + Vec2::new(half.x, doorway.offset),
            Vec2::new(DOOR_THRESHOLD_THICKNESS, doorway.width),
        ),
    };

    commands.spawn((
        MatchCleanup,
        Sprite {
            image: assets.door.clone(),
            color: Color::srgba(0.75, 0.86, 0.9, 0.72),
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(position.x, position.y, 2.05),
    ));
}

fn spawn_room_label(commands: &mut Commands, room: RoomSpec) {
    let position = room.center + Vec2::new(0.0, room.size.y * 0.5 - 20.0);

    let plaque_width = room.name.len() as f32 * 8.0 + 28.0;

    commands.spawn((
        MatchCleanup,
        Sprite {
            color: Color::srgba(0.025, 0.04, 0.055, 0.82),
            custom_size: Some(Vec2::new(plaque_width, 24.0)),
            ..default()
        },
        Transform::from_xyz(position.x, position.y, 3.2),
    ));

    commands.spawn((
        MatchCleanup,
        Text2d::new(room.name.to_uppercase()),
        TextFont::from_font_size(12.0),
        TextColor(Color::srgb(0.82, 0.93, 0.96)),
        Transform::from_xyz(position.x, position.y, 3.3),
    ));
}

fn spawn_outer_walls(commands: &mut Commands, assets: &GameAssets) {
    let horizontal_size = Vec2::new(MAP_FLOOR_SIZE.x + BORDER_THICKNESS * 2.0, BORDER_THICKNESS);

    let vertical_size = Vec2::new(BORDER_THICKNESS, MAP_FLOOR_SIZE.y + BORDER_THICKNESS * 2.0);

    let wall_x = MAP_BOUNDS.x + BORDER_THICKNESS * 0.5;
    let wall_y = MAP_BOUNDS.y + BORDER_THICKNESS * 0.5;

    for y in [wall_y, -wall_y] {
        spawn_wall(commands, assets, Vec2::new(0.0, y), horizontal_size, 2.5);
    }

    for x in [wall_x, -wall_x] {
        spawn_wall(commands, assets, Vec2::new(x, 0.0), vertical_size, 2.5);
    }
}

fn spawn_wall(commands: &mut Commands, assets: &GameAssets, position: Vec2, size: Vec2, z: f32) {
    // Small drop shadow makes the wall silhouette readable against the floor.
    commands.spawn((
        MatchCleanup,
        Sprite {
            color: Color::srgba(0.0, 0.0, 0.0, 0.34),
            custom_size: Some(size + Vec2::splat(4.0)),
            ..default()
        },
        Transform::from_xyz(position.x + 3.0, position.y - 3.0, z - 0.04),
    ));

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
            color: Color::srgba(0.88, 0.94, 0.96, 0.94),
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(position.x, position.y, z),
    ));
}

fn spawn_briefing_table(commands: &mut Commands, assets: &GameAssets) {
    let position = BRIEFING_CENTER + Vec2::new(0.0, 10.0);
    let size = Vec2::new(100.0, 60.0);

    commands.spawn((
        MatchCleanup,
        SolidAabb {
            // Slightly smaller than the visible sprite to avoid snagging.
            half_extents: Vec2::new(44.0, 24.0),
        },
        Sprite {
            image: assets.table.clone(),
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(position.x, position.y, 3.0),
    ));

    let seats = [
        Vec2::new(-44.0, 50.0),
        Vec2::new(0.0, 50.0),
        Vec2::new(44.0, 50.0),
        Vec2::new(-44.0, -34.0),
        Vec2::new(0.0, -34.0),
        Vec2::new(44.0, -34.0),
    ];

    for offset in seats {
        let seat_position = position + offset;

        commands.spawn((
            MatchCleanup,
            Sprite {
                image: assets.seat.clone(),
                color: Color::srgb(0.74, 0.78, 0.8),
                custom_size: Some(Vec2::splat(26.0)),
                ..default()
            },
            Transform::from_xyz(seat_position.x, seat_position.y, 2.9),
        ));
    }
}

fn spawn_emergency_button(commands: &mut Commands) {
    commands.spawn((
        MatchCleanup,
        EmergencyButton,
        Sprite {
            color: Color::srgb(0.92, 0.16, 0.12),
            custom_size: Some(Vec2::splat(24.0)),
            ..default()
        },
        Transform::from_xyz(
            EMERGENCY_BUTTON_POSITION.x,
            EMERGENCY_BUTTON_POSITION.y,
            6.0,
        ),
    ));
}

fn spawn_wayfinding_lights(commands: &mut Commands) {
    let lights = [
        (Vec2::new(-185.0, 55.0), Color::srgb(0.35, 0.8, 0.95)),
        (Vec2::new(-185.0, -55.0), Color::srgb(0.95, 0.4, 0.25)),
        (Vec2::new(185.0, 55.0), Color::srgb(0.35, 0.8, 0.95)),
        (Vec2::new(185.0, -55.0), Color::srgb(0.45, 0.9, 0.6)),
        (Vec2::new(0.0, 145.0), Color::srgb(0.95, 0.8, 0.25)),
        (Vec2::new(0.0, -145.0), Color::srgb(0.75, 0.65, 0.45)),
    ];

    for (position, color) in lights {
        commands.spawn((
            MatchCleanup,
            Sprite {
                color,
                custom_size: Some(Vec2::splat(7.0)),
                ..default()
            },
            Transform::from_xyz(position.x, position.y, 3.6),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_spans_removes_sorted_and_unsorted_gaps() {
        let spans = split_spans(-100.0, 100.0, &[(20.0, 40.0), (-40.0, -20.0)]);

        assert_eq!(spans, vec![(-100.0, -40.0), (-20.0, 20.0), (40.0, 100.0)]);
    }

    #[test]
    fn every_room_stays_inside_map_bounds() {
        for room in ROOMS {
            let half = room.size * 0.5;

            assert!(room.center.x - half.x >= -MAP_BOUNDS.x);
            assert!(room.center.x + half.x <= MAP_BOUNDS.x);
            assert!(room.center.y - half.y >= -MAP_BOUNDS.y);
            assert!(room.center.y + half.y <= MAP_BOUNDS.y);
        }
    }

    #[test]
    fn every_door_is_wide_enough_for_a_player() {
        for room in ROOMS {
            for door in room.doors {
                assert!(door.width > super::super::PLAYER_RADIUS * 2.0);
            }
        }
    }

    #[test]
    fn corridor_widths_clear_player_diameter() {
        for corridor in CORRIDORS {
            let width = match corridor.axis {
                CorridorAxis::Horizontal => corridor.size.y,
                CorridorAxis::Vertical => corridor.size.x,
            };

            assert!(width > super::super::PLAYER_RADIUS * 2.0 + 8.0);
        }
    }
}
