use bevy::prelude::*;

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Crewmate,
    Impostor,
}

#[derive(Component)]
pub struct Alive;

#[derive(Component)]
pub struct Ghost;

#[derive(Component)]
pub struct Body {
    pub player_id: u64,
    pub name: String,
}
