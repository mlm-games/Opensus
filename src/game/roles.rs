use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use super::PlayerIntent;

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Role {
    Crewmate,
    Impostor,
}

#[derive(Component, Default)]
pub struct Alive;

#[derive(Component, Default)]
pub struct Ghost;

/// Per-impostor kill cooldown (seconds remaining).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct KillCooldownLeft(pub f32);

/// Personal emergency meetings remaining this match.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct EmergenciesLeft(pub u8);

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct EmergencyCooldownLeft(pub f32);

/// Which dual-fix station this living player held on the previous reactor
/// pulse. Tracks the Second Reactor rule (progress needs two simultaneous
/// consoles), and is stripped when the player becomes a ghost.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct SabotageFixContribution {
    #[allow(dead_code, reason = "Reserved for soft-window Reactor sync rule")]
    pub station_entity: Option<Entity>,
}

#[derive(Component)]
pub struct Body {
    #[allow(dead_code, reason = "Reserved for the network protocol")]
    pub player_id: u64,
    pub name: String,
    pub reported: bool,
}

pub fn make_ghost(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<Alive>()
        .remove::<KillCooldownLeft>()
        .remove::<EmergenciesLeft>()
        .remove::<EmergencyCooldownLeft>()
        .remove::<SabotageFixContribution>()
        .insert(Ghost)
        // Zero intent so a corpse can't "hold E" from its last living frame
        // and keeps a stale movement vector forever.
        .insert(PlayerIntent::default());
}
