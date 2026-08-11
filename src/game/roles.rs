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
    #[expect(dead_code)]
    pub player_id: u64,
    #[expect(dead_code)]
    pub name: String,
}
