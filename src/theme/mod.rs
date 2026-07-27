use bevy::prelude::*;

#[derive(Resource)]
pub struct Theme {
    pub bg: Color,
    pub accent: Color,
    pub danger: Color,
    pub text: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::srgb(0.03, 0.03, 0.05),
            accent: Color::srgb(0.25, 0.5, 0.85),
            danger: Color::srgb(0.75, 0.25, 0.25),
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
