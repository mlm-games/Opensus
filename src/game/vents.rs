#[derive(Clone, Copy, Debug)]
pub struct Vent {
    pub id: u8,
    pub exits: &'static [u8],
}

#[derive(Clone, Copy, Debug)]
pub struct InVent {
    pub vent_id: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct VentRequest {
    pub actor_id: u64,
    pub vent_id: u8,
}

pub const VENTS: [Vent; 6] = [
    Vent { id: 0, exits: &[1] },
    Vent { id: 1, exits: &[0] },
    Vent { id: 2, exits: &[3] },
    Vent { id: 3, exits: &[2] },
    Vent { id: 4, exits: &[5] },
    Vent { id: 5, exits: &[4] },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vent_networks_are_paired() {
        assert_eq!(VENTS.len(), 6);
        for vent in &VENTS {
            for exit in vent.exits {
                assert!(VENTS[*exit as usize].exits.contains(&vent.id));
            }
        }
    }
}
