pub mod app;
pub mod assets;
pub mod audio;
pub mod game;
pub mod render;
pub mod save;
pub mod ui;

pub use app::App;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub fn run() {
    if let Err(error) = app::run_desktop() {
        eprintln!("opensus failed to start: {error:?}");
    }
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn run() {
    if let Err(error) = app::run_web() {
        eprintln!("opensus failed to start: {error:?}");
    }
}
