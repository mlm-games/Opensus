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
    pub is_bot: bool,
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

    let push_local_host = |lobby: &mut LobbyState| {
        lobby.is_host = true;
        lobby.slots.push(LobbySlot {
            id: 1,
            name: save.player_name.clone(),
            color_index: save.preferred_color_index,
            ready: false,
            is_local: true,
            is_host: true,
            is_bot: false,
        });
    };

    let fill_bots = |lobby: &mut LobbyState| {
        for i in 0..cfg.bot_count as u64 {
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
                is_bot: true,
            });
        }
    };

    match *mode {
        RuntimeMode::Local => {
            push_local_host(&mut lobby);
            fill_bots(&mut lobby);
        }
        RuntimeMode::Host => {
            // Online host starts with just themselves; real clients join over
            // the network and the host start rule requires everyone ready.
            push_local_host(&mut lobby);
        }
        RuntimeMode::Client => {
            lobby.is_host = false;
        }
    }
}

fn handle_start_match(
    mut ev: MessageReader<StartMatchRequest>,
    lobby: Res<LobbyState>,
    mode: Res<RuntimeMode>,
    cfg: Res<MatchConfig>,
    mut transition: ResMut<game_utils_bevy::transitions::Transition<AppState>>,
) {
    for _ in ev.read() {
        if !lobby.is_host || !lobby.local_ready {
            continue;
        }
        let minimum_players = (cfg.impostor_count as usize)
            .saturating_mul(2)
            .saturating_add(1)
            .max(3);
        if lobby.slots.len() < minimum_players {
            continue;
        }
        // Offline bots are pre-ready; online Host requires every remote ready.
        let everyone_ready = lobby.slots.iter().all(|s| s.is_local || s.ready);
        if matches!(*mode, RuntimeMode::Local) || everyone_ready {
            transition.begin_to_state(AppState::InGame);
        }
    }
}
