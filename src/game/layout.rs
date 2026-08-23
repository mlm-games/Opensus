use bevy::prelude::Vec2;

/// Maximum center position for ghosts.
///
/// Living players additionally reserve `PLAYER_RADIUS`, so their effective
/// movement bounds are smaller by that radius.
pub const MAP_BOUNDS: Vec2 = Vec2::new(520.0, 300.0);

pub const MAP_FLOOR_SIZE: Vec2 = Vec2::new(MAP_BOUNDS.x * 2.0, MAP_BOUNDS.y * 2.0);

pub const BORDER_THICKNESS: f32 = 12.0;

pub const BRIEFING_CENTER: Vec2 = Vec2::new(0.0, 0.0);
pub const BRIEFING_SIZE: Vec2 = Vec2::new(300.0, 210.0);

pub const ARCHIVES_CENTER: Vec2 = Vec2::new(-360.0, 120.0);
pub const ARCHIVES_SIZE: Vec2 = Vec2::new(280.0, 190.0);

pub const COMMS_CENTER: Vec2 = Vec2::new(360.0, 120.0);
pub const COMMS_SIZE: Vec2 = Vec2::new(280.0, 190.0);

pub const REACTOR_CENTER: Vec2 = Vec2::new(-360.0, -120.0);
pub const REACTOR_SIZE: Vec2 = Vec2::new(280.0, 190.0);

pub const MEDBAY_CENTER: Vec2 = Vec2::new(360.0, -120.0);
pub const MEDBAY_SIZE: Vec2 = Vec2::new(280.0, 190.0);

pub const ELECTRICAL_CENTER: Vec2 = Vec2::new(0.0, 235.0);
pub const ELECTRICAL_SIZE: Vec2 = Vec2::new(240.0, 90.0);

pub const STORAGE_CENTER: Vec2 = Vec2::new(0.0, -235.0);
pub const STORAGE_SIZE: Vec2 = Vec2::new(240.0, 90.0);

pub const TASK_STATIONS: [(u32, &str, Vec2); 5] = [
    (1, "Wire tap", Vec2::new(-445.0, 145.0)),
    (2, "Decode signal", Vec2::new(445.0, 145.0)),
    (3, "Stabilize core", Vec2::new(-360.0, -185.0)),
    (4, "Medical scan", Vec2::new(445.0, -145.0)),
    (5, "Upload dossier", Vec2::new(0.0, -235.0)),
];

pub const OXYGEN_STATIONS: [Vec2; 2] = [Vec2::new(275.0, 145.0), Vec2::new(275.0, -145.0)];

pub const REACTOR_STATIONS: [Vec2; 2] = [Vec2::new(-430.0, -120.0), Vec2::new(-290.0, -120.0)];

pub const LIGHTS_STATION: Vec2 = Vec2::new(0.0, 235.0);

pub const EMERGENCY_BUTTON_POSITION: Vec2 = Vec2::new(0.0, -25.0);

pub const PLAYER_SPAWNS: [Vec2; 10] = [
    Vec2::new(-95.0, 75.0),
    Vec2::new(-32.0, 75.0),
    Vec2::new(32.0, 75.0),
    Vec2::new(95.0, 75.0),
    Vec2::new(-95.0, -75.0),
    Vec2::new(-32.0, -75.0),
    Vec2::new(32.0, -75.0),
    Vec2::new(95.0, -75.0),
    Vec2::new(-115.0, 0.0),
    Vec2::new(115.0, 0.0),
];

pub const NAV_NODES: [Vec2; 18] = [
    // Central Briefing paths around the physical meeting table.
    Vec2::new(0.0, 75.0),     // 0: hub north
    Vec2::new(0.0, -75.0),    // 1: hub south
    Vec2::new(-105.0, 0.0),   // 2: hub west
    Vec2::new(105.0, 0.0),    // 3: hub east
    ARCHIVES_CENTER,          // 4
    COMMS_CENTER,             // 5
    REACTOR_CENTER,           // 6
    MEDBAY_CENTER,            // 7
    ELECTRICAL_CENTER,        // 8
    STORAGE_CENTER,           // 9
    Vec2::new(-185.0, 55.0),  // 10: west upper hall
    Vec2::new(-185.0, -55.0), // 11: west lower hall
    Vec2::new(185.0, 55.0),   // 12: east upper hall
    Vec2::new(185.0, -55.0),  // 13: east lower hall
    Vec2::new(0.0, 145.0),    // 14: north hall
    Vec2::new(0.0, -145.0),   // 15: south hall
    Vec2::new(-360.0, 0.0),   // 16: archives/reactor connector
    Vec2::new(360.0, 0.0),    // 17: comms/medbay connector
];

pub const NAV_EDGES: [(usize, usize); 20] = [
    // Route around the Briefing table.
    (0, 2),
    (0, 3),
    (1, 2),
    (1, 3),
    // North and south branches.
    (0, 14),
    (14, 8),
    (1, 15),
    (15, 9),
    // West rooms.
    (2, 10),
    (10, 4),
    (2, 11),
    (11, 6),
    // East rooms.
    (3, 12),
    (12, 5),
    (3, 13),
    (13, 7),
    // Vertical side loops.
    (4, 16),
    (16, 6),
    (5, 17),
    (17, 7),
];
