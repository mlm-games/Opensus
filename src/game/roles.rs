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
) {
    commands.entity(entity).remove::<Alive>().insert(Ghost);

    if let Ok(mut sprite) = sprites.get_mut(entity) {
        sprite.color = sprite.color.with_alpha(sprite.color.alpha().min(0.35));
    }

    if let Some(children) = children {
        for child in children.iter() {
            if let Ok(mut sprite) = sprites.get_mut(child) {
                sprite.color = sprite.color.with_alpha(0.35);
            }
        }
    }
}
