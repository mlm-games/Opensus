use bevy::prelude::*;

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GamePhase {
    #[default]
    None,
    Playing,
    Meeting,
    Voting,
    Results,
    GameOver {
        crew_win: bool,
    },
}

#[derive(Resource, Clone)]
pub struct MatchConfig {
    pub max_players: u8,
    pub impostor_count: u8,
    pub emergency_meetings: u8,
    pub kill_cooldown: f32,
    pub kill_range: f32,
    pub discussion_time: f32,
    pub voting_time: f32,
    pub results_time: f32,
    pub tasks_to_win: u32,
    pub player_speed: f32,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            max_players: 10,
            impostor_count: 1,
            emergency_meetings: 1,
            kill_cooldown: 15.0,
            kill_range: 48.0,
            discussion_time: 25.0,
            voting_time: 25.0,
            results_time: 5.0,
            tasks_to_win: 4,
            player_speed: 220.0,
        }
    }
}

#[derive(Resource, Default)]
pub struct LocalRole(pub Option<Role>);

use super::roles::Role;
