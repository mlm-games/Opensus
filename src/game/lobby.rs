use bevy::prelude::*;

use crate::app::AppState;
use crate::save::SaveData;

use super::{MatchConfig, RuntimeMode};

#[derive(Clone, Debug)]
pub struct LobbySlot {
    pub id: u64,
    pub name: String,
    pub color_index: u8,
    pub ready: bool,
    pub is_local: bool,
    pub is_host: bool,
}

#[derive(Resource, Default)]
pub struct LobbyState {
    pub slots: Vec<LobbySlot>,
    pub local_ready: bool,
    pub is_host: bool,
}

#[derive(Message)]
pub struct StartMatchRequest;

pub struct LobbyPlugin;
impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Lobby), setup_lobby)
            .add_systems(OnExit(AppState::Lobby), |_s: ResMut<LobbyState>| {})
            .add_systems(Update, handle_start_match.run_if(in_state(AppState::Lobby)));
    }
}

fn setup_lobby(
    mut lobby: ResMut<LobbyState>,
    save: Res<SaveData>,
    mode: Res<RuntimeMode>,
    cfg: Res<MatchConfig>,
) {
    lobby.slots.clear();
    lobby.local_ready = false;

    match *mode {
        RuntimeMode::Local => {
            lobby.is_host = true;
            lobby.slots.push(LobbySlot {
                id: 1,
                name: save.player_name.clone(),
                color_index: save.preferred_color_index,
                ready: false,
                is_local: true,
                is_host: true,
            });
            for i in 0..3u64 {
                if lobby.slots.len() >= cfg.max_players as usize {
                    break;
                }
                lobby.slots.push(LobbySlot {
                    id: 10 + i,
                    name: format!("Agent-{}", i + 2),
                    color_index: ((save.preferred_color_index as u64 + 1 + i) % 12) as u8,
                    ready: true,
                    is_local: false,
                    is_host: false,
                });
            }
        }
        RuntimeMode::Host => {
            lobby.is_host = true;
            lobby.slots.push(LobbySlot {
                id: 1,
                name: save.player_name.clone(),
                color_index: save.preferred_color_index,
                ready: false,
                is_local: true,
                is_host: true,
            });
        }
        RuntimeMode::Client => {
            lobby.is_host = false;
        }
    }
}

fn handle_start_match(
    mut ev: MessageReader<StartMatchRequest>,
    lobby: Res<LobbyState>,
    mut transition: ResMut<game_utils_bevy::transitions::Transition<AppState>>,
) {
    for _ in ev.read() {
        if !lobby.is_host {
            continue;
        }
        let ready_count = lobby.slots.iter().filter(|s| s.ready || s.is_local).count();
        // require local ready + at least 2
        if lobby.local_ready && ready_count >= 2 {
            transition.begin_to_state(AppState::InGame);
        }
    }
}
