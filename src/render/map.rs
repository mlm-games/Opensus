use bevy_ecs::prelude::*;
use glam::Vec2;
use repose_core::Color;

use super::{DrawItem, push_rect, rgb, rgba};
use crate::game::{
    BriefingTable, CORRIDORS, EMERGENCY_BUTTON_POSITION, LocalPlayer, MAP_FLOOR_SIZE, Position,
    ROOMS, Role, Side, SolidAabb, TaskAssignments, TaskStation,
};

const DOOR_THRESHOLD_THICKNESS: f32 = 12.0;

const WAYFINDING_LIGHTS: [(Vec2, Color); 6] = [
    (Vec2::new(-450.0, 65.0), Color(89, 204, 242, 255)),
    (Vec2::new(-450.0, -65.0), Color(242, 102, 64, 255)),
    (Vec2::new(450.0, 65.0), Color(89, 204, 242, 255)),
    (Vec2::new(450.0, -65.0), Color(115, 230, 153, 255)),
    (Vec2::new(0.0, 290.0), Color(242, 204, 64, 255)),
    (Vec2::new(0.0, -290.0), Color(191, 166, 115, 255)),
];

const BRIEFING_SEATS: [Vec2; 6] = [
    Vec2::new(-68.0, 70.0),
    Vec2::new(0.0, 70.0),
    Vec2::new(68.0, 70.0),
    Vec2::new(-68.0, -52.0),
    Vec2::new(0.0, -52.0),
    Vec2::new(68.0, -52.0),
];

pub(crate) fn push_map_ground(world: &mut World, items: &mut Vec<DrawItem>) {
    push_rect(
        items,
        Vec2::ZERO,
        MAP_FLOOR_SIZE + Vec2::splat(56.0),
        rgb(0.025, 0.035, 0.045),
        0.0,
    );

    for corridor in CORRIDORS {
        push_rect(
            items,
            corridor.center + Vec2::new(5.0, -7.0),
            corridor.size + Vec2::splat(26.0),
            rgba(0.015, 0.025, 0.035, 0.98),
            0.0,
        );
        push_rect(
            items,
            corridor.center,
            corridor.size,
            rgb(0.34, 0.43, 0.49),
            0.0,
        );
    }

    for room in ROOMS {
        push_rect(
            items,
            room.center + Vec2::new(7.0, -9.0),
            room.size + Vec2::splat(34.0),
            rgba(0.01, 0.018, 0.027, 0.98),
            0.0,
        );
        push_rect(
            items,
            room.center,
            room.size,
            rgb(room.tint.0, room.tint.1, room.tint.2),
            0.0,
        );
    }

    push_thresholds(items);
    push_walls(world, items);
    push_table(world, items);
}

pub(crate) fn push_map_fixtures(world: &mut World, items: &mut Vec<DrawItem>) {
    for room in ROOMS {
        let center = room.center + Vec2::new(0.0, room.size.y * 0.5 - 20.0);
        let plaque_width = room.name.len() as f32 * 8.0 + 28.0;
        push_rect(
            items,
            center,
            Vec2::new(plaque_width, 24.0),
            rgba(0.025, 0.04, 0.055, 0.82),
            4.0,
        );
        items.push(DrawItem::Text {
            center,
            text: room.name.to_uppercase(),
            color: rgb(0.82, 0.93, 0.96),
            size: 12.0,
        });
    }

    for (position, color) in WAYFINDING_LIGHTS {
        push_rect(items, position, Vec2::splat(9.0), color, 2.0);
    }

    push_stations(world, items);
    push_rect(
        items,
        EMERGENCY_BUTTON_POSITION,
        Vec2::splat(24.0),
        rgb(0.92, 0.16, 0.12),
        4.0,
    );
}

fn push_thresholds(items: &mut Vec<DrawItem>) {
    let color = rgba(0.75, 0.86, 0.9, 0.72);
    for room in ROOMS {
        let half = room.size * 0.5;
        for door in room.doors {
            let (center, size) = match door.side {
                Side::Top => (
                    room.center + Vec2::new(door.offset, half.y),
                    Vec2::new(door.width, DOOR_THRESHOLD_THICKNESS),
                ),
                Side::Bottom => (
                    room.center + Vec2::new(door.offset, -half.y),
                    Vec2::new(door.width, DOOR_THRESHOLD_THICKNESS),
                ),
                Side::Left => (
                    room.center + Vec2::new(-half.x, door.offset),
                    Vec2::new(DOOR_THRESHOLD_THICKNESS, door.width),
                ),
                Side::Right => (
                    room.center + Vec2::new(half.x, door.offset),
                    Vec2::new(DOOR_THRESHOLD_THICKNESS, door.width),
                ),
            };
            push_rect(items, center, size, color, 0.0);
        }
    }
}

fn push_walls(world: &mut World, items: &mut Vec<DrawItem>) {
    let shadow = rgba(0.0, 0.0, 0.0, 0.34);
    let face = rgba(0.88, 0.94, 0.96, 0.94);
    let mut query = world.query::<(&Position, &SolidAabb, Option<&BriefingTable>)>();
    for (position, solid, table) in query.iter(world) {
        if table.is_some() {
            continue;
        }
        let size = solid.half_extents * 2.0;
        push_rect(
            items,
            position.0 + Vec2::new(3.0, -3.0),
            size + Vec2::splat(4.0),
            shadow,
            0.0,
        );
        push_rect(items, position.0, size, face, 0.0);
    }
}

fn push_table(world: &mut World, items: &mut Vec<DrawItem>) {
    let mut query = world.query_filtered::<&Position, With<BriefingTable>>();
    let Ok(position) = query.single(world) else {
        return;
    };
    for offset in BRIEFING_SEATS {
        push_rect(
            items,
            position.0 + offset,
            Vec2::splat(26.0),
            rgb(0.74, 0.78, 0.8),
            13.0,
        );
    }
    push_rect(
        items,
        position.0,
        Vec2::new(160.0, 92.0),
        rgb(0.16, 0.21, 0.25),
        16.0,
    );
}

fn push_stations(world: &mut World, items: &mut Vec<DrawItem>) {
    let tint = {
        let mut query =
            world.query_filtered::<(&TaskAssignments, Option<&Role>), With<LocalPlayer>>();
        query
            .single(world)
            .ok()
            .map(|(assignments, role)| (assignments.clone(), matches!(role, Some(Role::Impostor))))
    };
    let mut query = world.query::<(&TaskStation, &Position)>();
    for (station, position) in query.iter(world) {
        let color = match &tint {
            Some((assignments, _)) if assignments.is_done(station.id) => {
                rgba(0.45, 0.48, 0.50, 0.75)
            }
            Some((assignments, impostor)) if assignments.has(station.id) || *impostor => {
                Color::WHITE
            }
            Some(_) => rgba(0.55, 0.60, 0.64, 0.42),
            None => Color::WHITE,
        };
        push_rect(items, position.0, Vec2::splat(28.0), color, 6.0);
    }
}
