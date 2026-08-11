use bevy::prelude::*;
use game_utils::save::Versioned;
use serde::{Deserialize, Serialize};

pub const SAVE_VERSION: u32 = 1;

#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct SaveData {
    #[serde(default)]
    pub version: u32,
    pub settings: SettingsData,
    pub player_name: String,
    pub preferred_color_index: u8,
    pub games_played: u32,
    pub crew_wins: u32,
    pub impostor_wins: u32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SettingsData {
    pub master_volume: f32,
    pub sfx_volume: f32,
    pub music_volume: f32,
    pub language: String,
}

impl Default for SettingsData {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            sfx_volume: 1.0,
            music_volume: 0.8,
            language: "en".to_string(),
        }
    }
}

impl Default for SaveData {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            settings: SettingsData::default(),
            player_name: "Agent".to_string(),
            preferred_color_index: 0,
            games_played: 0,
            crew_wins: 0,
            impostor_wins: 0,
        }
    }
}

impl Versioned for SaveData {
    fn version(&self) -> u32 {
        self.version
    }

    fn set_version(&mut self, version: u32) {
        self.version = version;
    }
}
