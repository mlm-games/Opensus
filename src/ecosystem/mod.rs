pub mod audio;
pub mod center_pivot;
pub mod game_feel;
pub mod i18n;
pub mod juice;
pub mod math_utils;
pub mod pooling;
pub mod save;
pub mod screen_effects;
pub mod transitions;
pub mod ui_effects;
pub mod vfx;

use bevy::prelude::*;

use audio::AudioPlugin;
use center_pivot::CenterPivotPlugin;
use game_feel::GameFeelPlugin;
use i18n::I18nPlugin;
use juice::JuicePlugin;
use save::SavePlugin;
use screen_effects::ScreenEffectsPlugin;
use transitions::TransitionsPlugin;
use ui_effects::UiEffectsPlugin;
use vfx::VfxPlugin;

pub struct EcosystemPlugin;
impl Plugin for EcosystemPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            AudioPlugin,
            CenterPivotPlugin,
            GameFeelPlugin,
            I18nPlugin,
            JuicePlugin,
            SavePlugin,
            ScreenEffectsPlugin,
            TransitionsPlugin,
            UiEffectsPlugin,
            VfxPlugin,
        ));
    }
}
