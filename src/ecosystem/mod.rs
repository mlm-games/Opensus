pub mod audio;
pub mod center_pivot;
pub mod game_feel;
pub mod juice;
pub mod math_utils;
pub mod pooling;
pub mod save;
pub mod screen_effects;
pub mod transitions;
pub mod ui_effects;
pub mod vfx;

use bevy::prelude::*;

use self::ui_effects::UiEffectsPlugin;
use self::vfx::VfxPlugin;

pub struct EcosystemPlugin;
impl Plugin for EcosystemPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((VfxPlugin, UiEffectsPlugin));
    }
}
