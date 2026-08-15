use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::Role;

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
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

#[derive(Resource, Clone, Debug)]
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
    pub task_hold_time: f32,

    pub interact_range: f32,
    pub report_range: f32,

    pub sabotage_cooldown: f32,
    pub oxygen_time: f32,
    pub reactor_time: f32,
    pub sabotage_fix_time: f32,

    pub player_speed: f32,

    /// Camera exponential-follow sharpness, independent of frame rate.
    pub camera_follow_sharpness: f32,
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
            task_hold_time: 2.0,

            interact_range: 42.0,
            report_range: 52.0,

            sabotage_cooldown: 20.0,
            oxygen_time: 30.0,
            reactor_time: 45.0,
            sabotage_fix_time: 2.5,

            player_speed: 220.0,
            camera_follow_sharpness: 9.0,
        }
    }
}

#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct LocalRole(pub Option<Role>);
