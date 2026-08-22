use serde::{Deserialize, Serialize};

use crate::game::{GamePhase, Role, SabotageKind};

pub const PROTOCOL_VERSION: u32 = 4;
pub const PROTOCOL_ID: u64 = 0x4F50_454E_5355_5301;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NetLobbyPlayer {
    pub player_id: u64,
    pub name: String,
    pub color_index: u8,
    pub ready: bool,
    pub is_host: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NetPlayerState {
    pub player_id: u64,
    pub name: String,
    pub color_index: u8,
    pub position: [f32; 2],
    pub alive: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NetBodyState {
    pub body_id: u64,
    pub player_id: u64,
    pub name: String,
    pub position: [f32; 2],
    pub reported: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NetSabotageState {
    pub kind: SabotageKind,
    pub remaining: f32,
    pub fixes_needed: u8,
    pub fixes_done: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PrivatePlayerState {
    pub kill_cooldown: f32,
    pub emergencies_left: u8,
    pub role: Role,
    pub voted: bool,
    pub vote_tallies: Vec<(String, u32)>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ClientPacket {
    Hello {
        protocol_version: u32,
        name: String,
        color_index: u8,
    },
    Ready {
        ready: bool,
    },
    Input {
        sequence: u32,
        movement: [f32; 2],
        interact: bool,
    },
    Kill,
    Report,
    Emergency,
    Vote {
        target: Option<u64>,
    },
    Sabotage {
        kind: SabotageKind,
    },
    Chat {
        text: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ServerPacket {
    Welcome {
        player_id: u64,
    },
    LobbySnapshot {
        players: Vec<NetLobbyPlayer>,
    },
    MatchStarted {
        your_role: Role,
    },
    WorldSnapshot {
        sequence: u32,
        players: Vec<NetPlayerState>,
        bodies: Vec<NetBodyState>,
        phase: GamePhase,
        sabotage: Option<NetSabotageState>,
        tasks_completed: u32,
        tasks_total: u32,
        meeting_prompt: String,
        meeting_timer: f32,
        vote_options: Vec<(u64, String, bool)>,
        result_text: String,
        private: Option<PrivatePlayerState>,
    },
    Chat {
        player_id: u64,
        name: String,
        text: String,
        ghost: bool,
    },
    Rejected {
        reason: String,
    },
}
