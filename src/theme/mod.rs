use bevy::prelude::*;

#[derive(Resource)]
pub struct Theme {
    #[expect(dead_code)]
    pub bg: Color,
    #[expect(dead_code)]
    pub accent: Color,
    #[expect(dead_code)]
    pub danger: Color,
    #[expect(dead_code)]
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
