mod app;
mod asset_tracking;
mod demo;
mod dev_tools;
mod ecosystem;
mod menus;
mod screens;
mod theme;

use app::AppPlugin;
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "My Ecosystem Bevy".into(),
                        resolution: (1280, 720).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(AppPlugin)
        .run();
}
