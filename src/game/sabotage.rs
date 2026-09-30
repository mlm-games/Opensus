use bevy_ecs::prelude::*;

use super::SimTimer;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SabotageKind {
    Lights,
    Oxygen,
    Reactor,
}

#[derive(Resource, Default)]
pub struct ActiveSabotage {
    pub kind: Option<SabotageKind>,
    pub timer: Option<SimTimer>,
    pub fixes_needed: u8,
    pub fixes_done: u8,
}

impl ActiveSabotage {
    pub fn is_active(&self) -> bool {
        self.kind.is_some()
    }

    pub fn is_critical(&self) -> bool {
        matches!(
            self.kind,
            Some(SabotageKind::Oxygen | SabotageKind::Reactor)
        )
    }

    pub fn critical_remaining(&self) -> f32 {
        self.timer
            .as_ref()
            .map(SimTimer::remaining_secs)
            .unwrap_or(0.0)
    }

    pub fn is_fixed(&self) -> bool {
        self.fixes_needed > 0 && self.fixes_done >= self.fixes_needed
    }

    pub fn clear(&mut self) {
        self.kind = None;
        self.timer = None;
        self.fixes_needed = 0;
        self.fixes_done = 0;
    }
}

#[derive(Component)]
pub struct SabotageFixStation {
    #[allow(dead_code, reason = "Only read by the networking replication code")]
    pub id: u8,
    pub kind: SabotageKind,
    pub progress: f32,
}

pub fn clear_sabotage_world(
    sabotage: &mut ActiveSabotage,
    stations: &mut Query<&mut SabotageFixStation>,
) {
    sabotage.clear();
    for mut station in stations.iter_mut() {
        station.progress = 0.0;
    }
}
