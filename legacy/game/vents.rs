use bevy::prelude::*;

#[derive(Component, Clone, Debug)]
pub struct Vent {
    pub id: u8,
    pub exits: &'static [u8],
}

#[derive(Component, Clone, Copy, Debug)]
pub struct InVent {
    pub vent_id: u8,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct VentRequest {
    pub actor_id: u64,
    pub vent_id: u8,
}

pub struct VentsPlugin;

impl Plugin for VentsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<VentRequest>();
    }
}

// Start with three networks:
// Reactor ↔ Electrical
// Archives ↔ Storage
// Comms ↔ Medbay
pub const VENTS: [Vent; 6] = [
    Vent { id: 0, exits: &[1] },
    Vent { id: 1, exits: &[0] },
    Vent { id: 2, exits: &[3] },
    Vent { id: 3, exits: &[2] },
    Vent { id: 4, exits: &[5] },
    Vent { id: 5, exits: &[4] },
];
