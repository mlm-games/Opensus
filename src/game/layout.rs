use bevy::prelude::Vec2;

/// World limit used by movement and camera clamping.
pub const MAP_BOUNDS: Vec2 = Vec2::new(1180.0, 720.0);
pub const MAP_FLOOR_SIZE: Vec2 = Vec2::new(MAP_BOUNDS.x * 2.0, MAP_BOUNDS.y * 2.0);

pub const BORDER_THICKNESS: f32 = 16.0;

pub const BRIEFING_CENTER: Vec2 = Vec2::new(0.0, 0.0);
pub const BRIEFING_SIZE: Vec2 = Vec2::new(360.0, 260.0);

pub const ARCHIVES_CENTER: Vec2 = Vec2::new(-900.0, 175.0);
pub const ARCHIVES_SIZE: Vec2 = Vec2::new(360.0, 250.0);

pub const COMMS_CENTER: Vec2 = Vec2::new(900.0, 175.0);
pub const COMMS_SIZE: Vec2 = Vec2::new(360.0, 250.0);

pub const REACTOR_CENTER: Vec2 = Vec2::new(-900.0, -175.0);
pub const REACTOR_SIZE: Vec2 = Vec2::new(360.0, 250.0);

pub const MEDBAY_CENTER: Vec2 = Vec2::new(900.0, -175.0);
pub const MEDBAY_SIZE: Vec2 = Vec2::new(360.0, 250.0);

pub const ELECTRICAL_CENTER: Vec2 = Vec2::new(0.0, 560.0);
pub const ELECTRICAL_SIZE: Vec2 = Vec2::new(320.0, 220.0);

pub const STORAGE_CENTER: Vec2 = Vec2::new(0.0, -560.0);
pub const STORAGE_SIZE: Vec2 = Vec2::new(360.0, 220.0);

pub const TASK_STATIONS: [(u32, &str, Vec2); 10] = [
    (1, "Align records", Vec2::new(-1010.0, 210.0)),
    (2, "Restore archive power", Vec2::new(-810.0, 110.0)),
    (3, "Decode signal", Vec2::new(1010.0, 210.0)),
    (4, "Calibrate antenna", Vec2::new(810.0, 110.0)),
    (5, "Prime coolant", Vec2::new(-1010.0, -210.0)),
    (6, "Stabilize core", Vec2::new(-810.0, -110.0)),
    (7, "Medical scan", Vec2::new(1010.0, -205.0)),
    (8, "Sort samples", Vec2::new(810.0, -105.0)),
    (9, "Reset breakers", Vec2::new(-90.0, 560.0)),
    (10, "Chart cargo", Vec2::new(90.0, -560.0)),
];

pub const OXYGEN_STATIONS: [Vec2; 2] = [Vec2::new(790.0, 175.0), Vec2::new(790.0, -175.0)];

pub const REACTOR_STATIONS: [Vec2; 2] = [Vec2::new(-1010.0, -175.0), Vec2::new(-790.0, -175.0)];

pub const LIGHTS_STATION: Vec2 = Vec2::new(0.0, 560.0);
pub const EMERGENCY_BUTTON_POSITION: Vec2 = Vec2::new(0.0, -35.0);

pub const PLAYER_SPAWNS: [Vec2; 10] = [
    Vec2::new(-120.0, 90.0),
    Vec2::new(-40.0, 100.0),
    Vec2::new(40.0, 100.0),
    Vec2::new(120.0, 90.0),
    Vec2::new(-120.0, -90.0),
    Vec2::new(-40.0, -100.0),
    Vec2::new(40.0, -100.0),
    Vec2::new(120.0, -90.0),
    Vec2::new(-150.0, 0.0),
    Vec2::new(150.0, 0.0),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CorridorAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug)]
pub struct CorridorSpec {
    pub center: Vec2,
    pub size: Vec2,
    pub axis: CorridorAxis,
}

pub const CORRIDORS: [CorridorSpec; 8] = [
    CorridorSpec {
        center: Vec2::new(-450.0, 65.0),
        size: Vec2::new(540.0, 84.0),
        axis: CorridorAxis::Horizontal,
    },
    CorridorSpec {
        center: Vec2::new(-450.0, -65.0),
        size: Vec2::new(540.0, 84.0),
        axis: CorridorAxis::Horizontal,
    },
    CorridorSpec {
        center: Vec2::new(450.0, 65.0),
        size: Vec2::new(540.0, 84.0),
        axis: CorridorAxis::Horizontal,
    },
    CorridorSpec {
        center: Vec2::new(450.0, -65.0),
        size: Vec2::new(540.0, 84.0),
        axis: CorridorAxis::Horizontal,
    },
    CorridorSpec {
        center: Vec2::new(-900.0, 0.0),
        size: Vec2::new(84.0, 100.0),
        axis: CorridorAxis::Vertical,
    },
    CorridorSpec {
        center: Vec2::new(900.0, 0.0),
        size: Vec2::new(84.0, 100.0),
        axis: CorridorAxis::Vertical,
    },
    CorridorSpec {
        center: Vec2::new(0.0, 290.0),
        size: Vec2::new(96.0, 320.0),
        axis: CorridorAxis::Vertical,
    },
    CorridorSpec {
        center: Vec2::new(0.0, -290.0),
        size: Vec2::new(96.0, 320.0),
        axis: CorridorAxis::Vertical,
    },
];

pub const NAV_NODES: [Vec2; 22] = [
    Vec2::new(0.0, 105.0),
    Vec2::new(0.0, -105.0),
    Vec2::new(-145.0, 0.0),
    Vec2::new(145.0, 0.0),
    ARCHIVES_CENTER,
    COMMS_CENTER,
    REACTOR_CENTER,
    MEDBAY_CENTER,
    ELECTRICAL_CENTER,
    STORAGE_CENTER,
    Vec2::new(-450.0, 65.0),
    Vec2::new(-450.0, -65.0),
    Vec2::new(450.0, 65.0),
    Vec2::new(450.0, -65.0),
    Vec2::new(0.0, 290.0),
    Vec2::new(0.0, -290.0),
    Vec2::new(-900.0, 0.0),
    Vec2::new(900.0, 0.0),
    Vec2::new(-120.0, 95.0),
    Vec2::new(120.0, 95.0),
    Vec2::new(-120.0, -80.0),
    Vec2::new(120.0, -80.0),
];

pub const NAV_EDGES: [(usize, usize); 24] = [
    (0, 18),
    (0, 19),
    (1, 20),
    (1, 21),
    (2, 18),
    (2, 20),
    (3, 19),
    (3, 21),
    (0, 14),
    (14, 8),
    (1, 15),
    (15, 9),
    (2, 10),
    (10, 4),
    (2, 11),
    (11, 6),
    (3, 12),
    (12, 5),
    (3, 13),
    (13, 7),
    (4, 16),
    (16, 6),
    (5, 17),
    (17, 7),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloorKind {
    Wood,
    Carpet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug)]
pub struct Doorway {
    pub side: Side,
    pub offset: f32,
    pub width: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct RoomSpec {
    pub name: &'static str,
    pub center: Vec2,
    pub size: Vec2,
    pub floor: FloorKind,
    pub tint: (f32, f32, f32),
    pub doors: &'static [Doorway],
}

pub const BRIEFING_DOORS: [Doorway; 6] = [
    Doorway {
        side: Side::Left,
        offset: 65.0,
        width: 84.0,
    },
    Doorway {
        side: Side::Left,
        offset: -65.0,
        width: 84.0,
    },
    Doorway {
        side: Side::Right,
        offset: 65.0,
        width: 84.0,
    },
    Doorway {
        side: Side::Right,
        offset: -65.0,
        width: 84.0,
    },
    Doorway {
        side: Side::Top,
        offset: 0.0,
        width: 96.0,
    },
    Doorway {
        side: Side::Bottom,
        offset: 0.0,
        width: 96.0,
    },
];

pub const ARCHIVES_DOORS: [Doorway; 2] = [
    Doorway {
        side: Side::Right,
        offset: -110.0,
        width: 84.0,
    },
    Doorway {
        side: Side::Bottom,
        offset: 0.0,
        width: 84.0,
    },
];

pub const COMMS_DOORS: [Doorway; 2] = [
    Doorway {
        side: Side::Left,
        offset: -110.0,
        width: 84.0,
    },
    Doorway {
        side: Side::Bottom,
        offset: 0.0,
        width: 84.0,
    },
];

pub const REACTOR_DOORS: [Doorway; 2] = [
    Doorway {
        side: Side::Right,
        offset: 110.0,
        width: 84.0,
    },
    Doorway {
        side: Side::Top,
        offset: 0.0,
        width: 84.0,
    },
];

pub const MEDBAY_DOORS: [Doorway; 2] = [
    Doorway {
        side: Side::Left,
        offset: 110.0,
        width: 84.0,
    },
    Doorway {
        side: Side::Top,
        offset: 0.0,
        width: 84.0,
    },
];

pub const ELECTRICAL_DOORS: [Doorway; 1] = [Doorway {
    side: Side::Bottom,
    offset: 0.0,
    width: 96.0,
}];

pub const STORAGE_DOORS: [Doorway; 1] = [Doorway {
    side: Side::Top,
    offset: 0.0,
    width: 96.0,
}];

pub const ROOMS: [RoomSpec; 7] = [
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_map_bounds_owner_exists_by_design() {
        assert_eq!(MAP_FLOOR_SIZE, MAP_BOUNDS * 2.0);
    }

    #[test]
    fn every_door_connects_to_a_corridor() {
        for room in ROOMS {
            let half = room.size * 0.5;
            for door in room.doors {
                let point = match door.side {
                    Side::Top => room.center + Vec2::new(door.offset, half.y),
                    Side::Bottom => room.center + Vec2::new(door.offset, -half.y),
                    Side::Left => room.center + Vec2::new(-half.x, door.offset),
                    Side::Right => room.center + Vec2::new(half.x, door.offset),
                };
                let connected = CORRIDORS.iter().any(|corridor| {
                    let half = corridor.size * 0.5 + Vec2::splat(1.0);
                    point.x >= corridor.center.x - half.x
                        && point.x <= corridor.center.x + half.x
                        && point.y >= corridor.center.y - half.y
                        && point.y <= corridor.center.y + half.y
                });
                assert!(
                    connected,
                    "{} has a door at {point:?} with no corridor",
                    room.name
                );
            }
        }
    }

    fn point_inside_room_or_corridor(p: Vec2) -> bool {
        for room in ROOMS {
            let half = room.size * 0.5;
            if p.x >= room.center.x - half.x
                && p.x <= room.center.x + half.x
                && p.y >= room.center.y - half.y
                && p.y <= room.center.y + half.y
            {
                return true;
            }
        }
        for corridor in CORRIDORS {
            let half = corridor.size * 0.5;
            if p.x >= corridor.center.x - half.x
                && p.x <= corridor.center.x + half.x
                && p.y >= corridor.center.y - half.y
                && p.y <= corridor.center.y + half.y
            {
                return true;
            }
        }
        false
    }

    #[test]
    fn every_task_fixes_spawn_inside_room_or_corridor() {
        for &(_, _, pos) in &TASK_STATIONS {
            assert!(
                point_inside_room_or_corridor(pos),
                "task at {pos:?} not inside"
            );
        }
        for &pos in &OXYGEN_STATIONS {
            assert!(
                point_inside_room_or_corridor(pos),
                "oxygen at {pos:?} not inside"
            );
        }
        for &pos in &REACTOR_STATIONS {
            assert!(
                point_inside_room_or_corridor(pos),
                "reactor at {pos:?} not inside"
            );
        }
        assert!(
            point_inside_room_or_corridor(LIGHTS_STATION),
            "lights not inside"
        );
        assert!(
            point_inside_room_or_corridor(EMERGENCY_BUTTON_POSITION),
            "button not inside"
        );
        for &pos in &PLAYER_SPAWNS {
            assert!(
                point_inside_room_or_corridor(pos),
                "spawn at {pos:?} not inside"
            );
        }
    }

    #[test]
    fn corridors_wider_than_player() {
        // Spec 12: wider than PLAYER_RADIUS*2 + 12
        const PLAYER_RADIUS: f32 = 14.0;
        for corridor in CORRIDORS {
            let width = match corridor.axis {
                CorridorAxis::Horizontal => corridor.size.y,
                CorridorAxis::Vertical => corridor.size.x,
            };
            assert!(
                width > PLAYER_RADIUS * 2.0 + 12.0,
                "corridor at {:?} width {width} too narrow",
                corridor.center
            );
        }
    }

    #[test]
    fn navigation_graph_is_connected() {
        // BFS from 0
        let n = NAV_NODES.len();
        let mut adj = vec![Vec::new(); n];
        for &(a, b) in &NAV_EDGES {
            adj[a].push(b);
            adj[b].push(a);
        }
        let mut visited = vec![false; n];
        let mut stack = vec![0];
        visited[0] = true;
        while let Some(cur) = stack.pop() {
            for &nb in &adj[cur] {
                if !visited[nb] {
                    visited[nb] = true;
                    stack.push(nb);
                }
            }
        }
        assert!(visited.iter().all(|&v| v), "nav graph not connected");
    }

    #[test]
    fn map_revision_is_two() {
        assert_eq!(crate::game::networking::protocol::MAP_REVISION, 2);
        assert_eq!(
            crate::game::networking::protocol::GAMEPLAY_PROTOCOL_VERSION,
            2
        );
    }
}
