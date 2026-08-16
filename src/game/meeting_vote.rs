use bevy::prelude::*;
use rand::prelude::IndexedRandom;
use std::collections::HashMap;

use super::{
    ActiveSabotage, Alive, EmergenciesLeft, EmergencyButton, GamePhase, Ghost, LocalPlayerId,
    MatchConfig, Player,
};
use crate::app::{AppState, Paused};
use game_utils_bevy::screen_effects::{ScreenEffects, Trauma};
use game_utils_bevy::transitions::Transition;

#[derive(Clone, Debug)]
pub struct VoteOption {
    pub player_id: u64,
    pub name: String,
    pub dead: bool,
}

#[derive(Resource, Default)]
pub struct MeetingState {
    pub timer: Timer,
    pub prompt: String,
    pub options: Vec<VoteOption>,
    pub votes: HashMap<u64, Option<u64>>, // voter -> Some(target) | None = skip
    pub local_voted: bool,
    pub result_text: String,
    /// FIXED: dedicated field instead of stashing the eject id under
    /// votes key 0 (which collided with real ids and double-applied).
    pub pending_eject: Option<u64>,
}

impl MeetingState {
    pub fn begin_meeting(
        &mut self,
        prompt: String,
        players: &Query<(&Player, Option<&Alive>, Option<&Ghost>)>,
        discussion: f32,
    ) {
        self.prompt = prompt;
        self.timer = Timer::from_seconds(discussion, TimerMode::Once);
        self.options.clear();
        self.votes.clear();
        self.local_voted = false;
        self.result_text.clear();
        self.pending_eject = None;
        for (p, alive, _g) in players.iter() {
            self.options.push(VoteOption {
                player_id: p.id,
                name: p.name.clone(),
                dead: alive.is_none(),
            });
        }
    }

    pub fn clear_for_play(&mut self) {
        self.prompt.clear();
        self.options.clear();
        self.votes.clear();
        self.local_voted = false;
        self.result_text.clear();
        self.pending_eject = None;
    }

    /// True once every living option has voted. An empty living list resolves
    /// immediately so a meeting can never deadlock.
    pub fn all_voted(&self) -> bool {
        let living_ids: Vec<u64> = self
            .options
            .iter()
            .filter(|o| !o.dead)
            .map(|o| o.player_id)
            .collect();
        if living_ids.is_empty() {
            return true;
        }
        living_ids.iter().all(|id| self.votes.contains_key(id))
    }

    pub fn resolve_votes(&mut self, phase: &mut GamePhase, results_time: f32) {
        let mut tallies: HashMap<Option<u64>, u32> = HashMap::new();

        for vote in self.votes.values() {
            *tallies.entry(*vote).or_default() += 1;
        }

        let maximum = tallies.values().copied().max().unwrap_or(0);

        let leaders: Vec<Option<u64>> = tallies
            .iter()
            .filter(|(_, count)| **count == maximum)
            .map(|(target, _)| *target)
            .collect();

        self.pending_eject = None;

        if maximum == 0 || leaders.len() != 1 || leaders[0].is_none() {
            self.result_text = "No one was ejected. (Skip / Tie)".into();
        } else if let Some(player_id) = leaders[0] {
            self.pending_eject = Some(player_id);

            let name = self
                .options
                .iter()
                .find(|option| option.player_id == player_id)
                .map(|option| option.name.as_str())
                .unwrap_or("Unknown");

            self.result_text = format!("{name} was ejected.");
        }

        self.timer = Timer::from_seconds(results_time.max(0.1), TimerMode::Once);

        *phase = GamePhase::Results;
    }
}

#[derive(Message, Clone)]
pub enum MeetingCommand {
    Emergency { actor_id: u64 },
    Vote { voter_id: u64, target: u64 },
    Skip { voter_id: u64 },
}

pub struct MeetingVotePlugin;

impl Plugin for MeetingVotePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), |mut m: ResMut<MeetingState>| {
            m.clear_for_play();
        })
        // Local input only (all modes).
        .add_systems(
            Update,
            emergency_hotkey
                .in_set(super::GameSimSet::Input)
                .run_if(in_state(AppState::InGame))
                .run_if(|p: Res<Paused>| !p.0)
                .run_if(|t: Res<Transition<AppState>>| !t.block_input),
        )
        // Authority resolves commands (eject lives in the GamePlugin chain).
        .add_systems(
            Update,
            handle_meeting_commands
                .in_set(super::GameSimSet::Resolve)
                .run_if(in_state(AppState::InGame))
                .run_if(|p: Res<Paused>| !p.0)
                .run_if(|t: Res<Transition<AppState>>| !t.block_input)
                .run_if(super::has_authority),
        );
    }
}

fn emergency_hotkey(
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<GamePhase>,
    local: Query<&Player, (With<super::LocalPlayer>, With<Alive>)>,
    mut ev: MessageWriter<MeetingCommand>,
) {
    if matches!(*phase, GamePhase::Playing) && keys.just_pressed(KeyCode::KeyF) {
        if let Ok(p) = local.single() {
            ev.write(MeetingCommand::Emergency { actor_id: p.id });
        }
    }
}

fn handle_meeting_commands(
    mut ev: MessageReader<MeetingCommand>,
    mut phase: ResMut<GamePhase>,
    mut meeting: ResMut<MeetingState>,
    cfg: Res<MatchConfig>,
    sabotage: Res<ActiveSabotage>,
    players: Query<(&Player, Option<&Alive>, Option<&Ghost>)>,
    mut living: Query<(&Player, &mut EmergenciesLeft), With<Alive>>,
    positions: Query<(&Player, &Transform), With<Alive>>,
    local_id: Res<LocalPlayerId>,
    emergency_buttons: Query<&Transform, With<EmergencyButton>>,
    mut trauma: ResMut<Trauma>,
) {
    for cmd in ev.read() {
        match cmd {
            MeetingCommand::Emergency { actor_id } => {
                if !matches!(*phase, GamePhase::Playing) || sabotage.is_critical() {
                    continue;
                }
                let Some((_, mut left)) = living.iter_mut().find(|(p, _)| p.id == *actor_id) else {
                    continue;
                };
                if left.0 == 0 {
                    continue;
                }
                // Map-gated: the caller must stand at the emergency button.
                let Some((_, position)) = positions.iter().find(|(p, _)| p.id == *actor_id) else {
                    continue;
                };
                let near_button = emergency_buttons.iter().any(|bt| {
                    position
                        .translation
                        .truncate()
                        .distance(bt.translation.truncate())
                        <= cfg.interact_range
                });
                if !near_button {
                    continue;
                }
                left.0 -= 1;
                ScreenEffects::add_trauma(&mut trauma, 0.35);
                meeting.begin_meeting("Emergency Meeting!".into(), &players, cfg.discussion_time);
                *phase = GamePhase::Meeting;
            }
            MeetingCommand::Vote { voter_id, target } => {
                if !matches!(*phase, GamePhase::Voting) || meeting.votes.contains_key(voter_id) {
                    continue;
                }
                let voter_alive = meeting
                    .options
                    .iter()
                    .any(|o| o.player_id == *voter_id && !o.dead);
                let target_alive = meeting
                    .options
                    .iter()
                    .any(|o| o.player_id == *target && !o.dead);
                if !voter_alive || !target_alive {
                    continue;
                }
                meeting.votes.insert(*voter_id, Some(*target));
                if local_id.0 == Some(*voter_id) {
                    meeting.local_voted = true;
                }
                bot_votes(&mut meeting, &players, *voter_id);
            }
            MeetingCommand::Skip { voter_id } => {
                if !matches!(*phase, GamePhase::Voting) || meeting.votes.contains_key(voter_id) {
                    continue;
                }
                let voter_alive = meeting
                    .options
                    .iter()
                    .any(|o| o.player_id == *voter_id && !o.dead);
                if !voter_alive {
                    continue;
                }
                meeting.votes.insert(*voter_id, None);
                if local_id.0 == Some(*voter_id) {
                    meeting.local_voted = true;
                }
                bot_votes(&mut meeting, &players, *voter_id);
            }
        }
    }
}

fn bot_votes(
    meeting: &mut MeetingState,
    players: &Query<(&Player, Option<&Alive>, Option<&Ghost>)>,
    local_id: u64,
) {
    let mut rng = rand::rng();
    let living_ids: Vec<u64> = meeting
        .options
        .iter()
        .filter(|o| !o.dead)
        .map(|o| o.player_id)
        .collect();
    for (p, alive, _) in players.iter() {
        if p.id == local_id || alive.is_none() || meeting.votes.contains_key(&p.id) {
            continue;
        }
        if rand::random::<f32>() < 0.25 || living_ids.is_empty() {
            meeting.votes.insert(p.id, None);
        } else if let Some(&pick) = living_ids.choose(&mut rng) {
            meeting.votes.insert(p.id, Some(pick));
        }
    }
}

/// Fill votes for every living non-local player that hasn't voted yet.
/// Safe to call every frame during Voting (idempotent via the votes map).
pub fn bot_votes_public(
    meeting: &mut MeetingState,
    players: &Query<(&Player, Option<&Alive>, Option<&Ghost>)>,
    local_id: u64,
) {
    bot_votes(meeting, players, local_id);
}
