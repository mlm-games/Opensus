mod app;
mod asset_tracking;
mod dev_tools;
mod game;
mod menus;
mod save;
mod screens;
mod theme;

use app::AppPlugin;
use bevy::prelude::*;
use bevy::window::WindowResolution;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn run() {
    let primary_window = Window {
        title: "Opensus".into(),
        resolution: WindowResolution::new(1280, 720),
        #[cfg(target_arch = "wasm32")]
        fit_canvas_to_parent: true,
        #[cfg(target_arch = "wasm32")]
        prevent_default_event_handling: true,
        ..default()
    };

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(primary_window),
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
                .set(AssetPlugin {
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").to_string(),
                    ..default()
                }),
        )
        .add_plugins(AppPlugin)
        .run();
}
