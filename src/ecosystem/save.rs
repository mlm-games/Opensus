use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct SaveData {
    pub high_score: u32, // Temp
    pub settings: SettingsData,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SettingsData {
    pub master_volume: f32,
    pub sfx_volume: f32,
    pub music_volume: f32,
}

impl Default for SettingsData {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            sfx_volume: 1.0,
            music_volume: 0.8,
        }
    }
}

impl Default for SaveData {
    fn default() -> Self {
        Self {
            high_score: 0,
            settings: SettingsData::default(),
        }
    }
}

pub struct SaveManager;

impl SaveManager {
    fn path() -> PathBuf {
        if let Some(proj) = directories::ProjectDirs::from("com", "mlm-games", "my-ecosystem-bevy")
        {
            let dir = proj.data_dir();
            let _ = fs::create_dir_all(dir);
            dir.join("save.ron")
        } else {
            PathBuf::from("saves/save.ron")
        }
    }

    pub fn save(data: &SaveData) -> Result<(), String> {
        let path = Self::path();
        if path.exists() {
            let _ = fs::copy(&path, path.with_extension("ron.bak"));
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let s = ron::ser::to_string_pretty(data, Default::default()).map_err(|e| e.to_string())?;
        fs::write(path, s).map_err(|e| e.to_string())
    }

    pub fn load() -> SaveData {
        let path = Self::path();
        fs::read_to_string(path)
            .ok()
            .and_then(|s| ron::from_str(&s).ok())
            .unwrap_or_default()
    }
}

pub struct SavePlugin;
impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        let data = SaveManager::load();
        app.insert_resource(data).add_systems(Update, hotkeys);
    }
}

fn hotkeys(keys: Res<ButtonInput<KeyCode>>, save: Res<SaveData>, mut commands: Commands) {
    if keys.just_pressed(KeyCode::F5) {
        if let Err(e) = SaveManager::save(&save) {
            bevy::log::warn!("Save failed: {e}");
        } else {
            bevy::log::info!("Game saved");
        }
    }
    if keys.just_pressed(KeyCode::F9) {
        let loaded = SaveManager::load();
        commands.insert_resource(loaded);
        bevy::log::info!("Game loaded");
    }
}
