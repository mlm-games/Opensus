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

#[derive(Component)]
pub struct Body {
    #[allow(dead_code, reason = "Reserved for the network protocol")]
    pub player_id: u64,
    pub name: String,
    pub reported: bool,
}

pub fn make_ghost(commands: &mut Commands, entity: Entity, sprite: &mut Sprite) {
    commands.entity(entity).remove::<Alive>().insert(Ghost);

    sprite.color = sprite.color.with_alpha(0.35);
}
