use bevy_ecs::prelude::*;
use repose_core::Color;

use crate::save::SaveData;

pub const PLAYER_COLORS: [Color; 12] = [
    Color(230, 51, 51, 255),
    Color(51, 115, 242, 255),
    Color(51, 191, 89, 255),
    Color(242, 217, 51, 255),
    Color(217, 89, 217, 255),
    Color(242, 140, 38, 255),
    Color(77, 230, 230, 255),
    Color(140, 64, 191, 255),
    Color(115, 77, 51, 255),
    Color(242, 242, 242, 255),
    Color(38, 38, 46, 255),
    Color(128, 204, 77, 255),
];

#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeMode {
    #[default]
    Local,
    Host,
    Client,
}

#[derive(Resource, Default, Clone, Debug)]
pub enum PendingNetworkStart {
    #[default]
    None,
    HostLocal {
        bind_addr: String,
    },
    JoinLocal {
        server_addr: String,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Crewmate,
    Impostor,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WinReason {
    Tasks,
    ImpostorsEliminated,
    ImpostorMajority,
    Sabotage,
}

impl WinReason {
    #[inline]
    pub const fn crew_win(self) -> bool {
        matches!(self, Self::Tasks | Self::ImpostorsEliminated)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Tasks => "All tasks completed",
            Self::ImpostorsEliminated => "All impostors eliminated",
            Self::ImpostorMajority => "Impostors reached majority",
            Self::Sabotage => "Critical sabotage succeeded",
        }
    }
}

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GamePhase {
    #[default]
    None,
    RoleReveal,
    Playing,
    Meeting,
    Voting,
    Results,
    GameOver {
        crew_win: bool,
        reason: WinReason,
    },
}

#[derive(Resource, Clone, Debug)]
pub struct MatchConfig {
    pub max_players: u8,
    pub impostor_count: u8,
    pub emergency_meetings: u8,
    pub emergency_cooldown: f32,

    pub kill_cooldown: f32,
    pub initial_kill_cooldown: f32,
    pub kill_range: f32,

    pub discussion_time: f32,
    pub voting_time: f32,
    pub results_time: f32,
    pub confirm_ejects: bool,

    pub role_reveal_time: f32,

    pub tasks_per_crewmate: u32,
    pub tasks_to_win: u32,
    pub task_hold_time: f32,

    pub interact_range: f32,
    pub report_range: f32,

    pub sabotage_cooldown: f32,
    pub oxygen_time: f32,
    pub reactor_time: f32,
    pub sabotage_fix_time: f32,
    pub reactor_sync_window: f32,

    pub player_speed: f32,
    pub ghost_speed_mul: f32,
    pub camera_follow_sharpness: f32,

    pub bot_count: u8,
    pub bot_task_weight: f32,
    pub bot_report_range: f32,
    pub bot_kill_aggression: f32,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            max_players: 10,
            impostor_count: 1,
            emergency_meetings: 1,
            emergency_cooldown: 15.0,

            kill_cooldown: 22.5,
            initial_kill_cooldown: 10.0,
            kill_range: 42.0,

            discussion_time: 18.0,
            voting_time: 22.0,
            results_time: 4.0,
            confirm_ejects: false,

            role_reveal_time: 3.0,

            tasks_per_crewmate: 3,
            tasks_to_win: 3,
            task_hold_time: 1.65,

            interact_range: 42.0,
            report_range: 52.0,
            sabotage_cooldown: 20.0,
            oxygen_time: 30.0,
            reactor_time: 45.0,
            sabotage_fix_time: 2.5,
            reactor_sync_window: 0.75,

            player_speed: 220.0,
            ghost_speed_mul: 1.18,
            camera_follow_sharpness: 9.0,

            bot_count: 3,
            bot_task_weight: 0.65,
            bot_report_range: 70.0,
            bot_kill_aggression: 0.55,
        }
    }
}

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

pub fn setup_lobby(world: &mut World) {
    let save = world.resource::<SaveData>().clone();
    let mode = *world.resource::<RuntimeMode>();
    let bot_count = world.resource::<MatchConfig>().bot_count;
    let max_players = world.resource::<MatchConfig>().max_players as usize;
    let mut lobby = world.resource_mut::<LobbyState>();

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
        for i in 0..bot_count as u64 {
            if lobby.slots.len() >= max_players {
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

    match mode {
        RuntimeMode::Local => {
            push_local_host(&mut lobby);
            fill_bots(&mut lobby);
        }
        RuntimeMode::Host => {
            push_local_host(&mut lobby);
        }
        RuntimeMode::Client => {
            lobby.is_host = false;
        }
    }
}

pub fn handle_start_match(world: &World) -> bool {
    let lobby = world.resource::<LobbyState>();
    let mode = *world.resource::<RuntimeMode>();
    let cfg = world.resource::<MatchConfig>();
    if !lobby.is_host || !lobby.local_ready {
        return false;
    }
    let minimum_players = match cfg.impostor_count {
        0 => 1,
        1 => 4,
        2 => 7,
        _ => 9,
    };
    if lobby.slots.len() < minimum_players {
        return false;
    }
    let everyone_ready = lobby.slots.iter().all(|s| s.is_local || s.ready);
    mode == RuntimeMode::Local || everyone_ready
}

#[derive(Clone, Debug)]
pub enum MeetingCommand {
    Emergency { actor_id: u64 },
    Vote { voter_id: u64, target: u64 },
    Skip { voter_id: u64 },
}

#[derive(Resource, Default)]
pub struct MeetingCommands(pub Vec<MeetingCommand>);
