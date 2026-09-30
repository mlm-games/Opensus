use bevy::prelude::*;

#[derive(Resource)]
pub struct Theme {
    #[allow(dead_code, reason = "Reserved for future UI theming")]
    pub bg: Color,
    #[allow(dead_code, reason = "Reserved for future UI theming")]
    pub accent: Color,
    #[allow(dead_code, reason = "Reserved for future UI theming")]
    pub danger: Color,
    #[allow(dead_code, reason = "Reserved for future UI theming")]
    pub text: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::srgb(0.06, 0.09, 0.12),
            accent: Color::srgb(0.55, 0.18, 0.18), // spy red
            danger: Color::srgb(0.85, 0.2, 0.15),
            text: Color::WHITE,
        }
    }
}

pub struct ThemePlugin;
impl Plugin for ThemePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Theme>();
    }
}
