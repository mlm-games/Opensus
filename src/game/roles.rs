use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Role {
    Crewmate,
    Impostor,
}

#[derive(Component, Default)]
pub struct Alive;

#[derive(Component, Default)]
pub struct Ghost;

/// Markers on child sprite layers of a player (body/clothes) so ghost fading
/// can fade every layer instead of just the root.
#[derive(Component)]
pub struct PlayerLayer;

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

pub fn make_ghost(
    commands: &mut Commands,
    entity: Entity,
    children: Option<&Children>,
    sprites: &mut Query<&mut Sprite>,
    texts: &mut Query<&mut TextColor>,
) {
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
        .insert(super::PlayerIntent::default());

    if let Ok(mut sprite) = sprites.get_mut(entity) {
        sprite.color = sprite.color.with_alpha(sprite.color.alpha().min(0.35));
    }

    if let Some(children) = children {
        for child in children.iter() {
            if let Ok(mut sprite) = sprites.get_mut(child) {
                sprite.color = sprite.color.with_alpha(0.35);
            }
            // Fade the name tag too, not just the body layers.
            if let Ok(mut tc) = texts.get_mut(child) {
                let c = tc.0;
                tc.0 = c.with_alpha(0.35);
            }
        }
    }
}
