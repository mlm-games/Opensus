use bevy_ecs::prelude::*;
use glam::Vec2;

use super::{
    BRIEFING_CENTER, CORRIDORS, CorridorAxis, EMERGENCY_BUTTON_POSITION, MatchCleanup, Position,
    ROOMS, Side, SolidAabb,
};

const ROOM_WALL_THICKNESS: f32 = 18.0;
const CORRIDOR_WALL_THICKNESS: f32 = 14.0;

/// Map-mounted emergency call button.
///
/// A living player must stand within `interact_range` before calling a meeting.
#[derive(Component)]
pub struct EmergencyButton;

#[derive(Component)]
pub struct BriefingTable;

pub fn spawn_map(world: &mut World) {
    spawn_corridor_walls(world);

    for room in ROOMS {
        spawn_room_walls(world, room);
    }

    spawn_briefing_table(world);
    spawn_emergency_button(world);
}

fn spawn_solid(world: &mut World, position: Vec2, size: Vec2) {
    world.spawn((
        MatchCleanup,
        SolidAabb {
            half_extents: size * 0.5,
        },
        Position(position),
    ));
}

/// Each corridor is physical side walls, so the walkable space between rooms
/// is exactly the painted corridor.
fn spawn_corridor_walls(world: &mut World) {
    for corridor in CORRIDORS {
        let thickness = CORRIDOR_WALL_THICKNESS;

        match corridor.axis {
            CorridorAxis::Horizontal => {
                let offset = corridor.size.y * 0.5 + thickness * 0.5;

                for sign in [-1.0, 1.0] {
                    spawn_solid(
                        world,
                        corridor.center + Vec2::new(0.0, sign * offset),
                        Vec2::new(corridor.size.x, thickness),
                    );
                }
            }
            CorridorAxis::Vertical => {
                let offset = corridor.size.x * 0.5 + thickness * 0.5;

                for sign in [-1.0, 1.0] {
                    spawn_solid(
                        world,
                        corridor.center + Vec2::new(sign * offset, 0.0),
                        Vec2::new(thickness, corridor.size.y),
                    );
                }
            }
        }
    }
}

fn spawn_room_walls(world: &mut World, room: super::RoomSpec) {
    let half = room.size * 0.5;

    for side in [Side::Top, Side::Bottom] {
        let y = room.center.y + if side == Side::Top { half.y } else { -half.y };

        let gaps = doorway_spans(room, side);

        for (start, end) in split_spans(-half.x, half.x, &gaps) {
            spawn_solid(
                world,
                Vec2::new(room.center.x + (start + end) * 0.5, y),
                Vec2::new(end - start, ROOM_WALL_THICKNESS),
            );
        }
    }

    for side in [Side::Left, Side::Right] {
        let x = room.center.x + if side == Side::Right { half.x } else { -half.x };

        let gaps = doorway_spans(room, side);

        for (start, end) in split_spans(-half.y, half.y, &gaps) {
            spawn_solid(
                world,
                Vec2::new(x, room.center.y + (start + end) * 0.5),
                Vec2::new(ROOM_WALL_THICKNESS, end - start),
            );
        }
    }
}

fn doorway_spans(room: super::RoomSpec, side: Side) -> Vec<(f32, f32)> {
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

fn spawn_briefing_table(world: &mut World) {
    let position = BRIEFING_CENTER + Vec2::new(0.0, 10.0);

    world.spawn((
        MatchCleanup,
        BriefingTable,
        SolidAabb {
            half_extents: Vec2::new(72.0, 38.0),
        },
        Position(position),
    ));
}

fn spawn_emergency_button(world: &mut World) {
    world.spawn((
        MatchCleanup,
        EmergencyButton,
        Position(EMERGENCY_BUTTON_POSITION),
    ));
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

            assert!(room.center.x - half.x >= -super::super::MAP_BOUNDS.x);
            assert!(room.center.x + half.x <= super::super::MAP_BOUNDS.x);
            assert!(room.center.y - half.y >= -super::super::MAP_BOUNDS.y);
            assert!(room.center.y + half.y <= super::super::MAP_BOUNDS.y);
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
