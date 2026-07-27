pub mod audio;
pub mod game_feel;
pub mod juice;
pub mod pooling;
pub mod save;
pub mod screen_effects;
pub mod transitions;

use bevy::prelude::*;

pub struct EcosystemPlugin;
impl Plugin for EcosystemPlugin {
    fn build(&self, _app: &mut App) {}
}
