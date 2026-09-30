use std::collections::HashMap;

use bevy_ecs::prelude::*;
use rand::Rng;
use rand::RngExt;
use rand::seq::IndexedRandom;

use super::{
    ActiveSabotage, AiPlayer, Alive, EmergenciesLeft, EmergencyButton, EmergencyCooldownLeft,
    GamePhase, Ghost, LocalPlayerId, MatchConfig, MeetingCommand, MeetingCommands, Player,
    Position, SimTimer, TimerMode, clear_sabotage_world,
};

#[derive(Clone, Debug)]
pub struct VoteOption {
    pub player_id: u64,
    pub name: String,
    pub dead: bool,
}

#[derive(Resource, Default)]
pub struct MeetingState {
    pub timer: SimTimer,
    pub prompt: String,
    pub options: Vec<VoteOption>,
    pub votes: HashMap<u64, Option<u64>>, // voter -> Some(target) | None = skip
    pub local_voted: bool,
    pub result_text: String,

    pub tallies: Vec<(String, u32)>,

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
        self.timer = SimTimer::from_seconds(discussion, TimerMode::Once);
        self.options.clear();
        self.votes.clear();
        self.local_voted = false;
        self.result_text.clear();
        self.tallies.clear();
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
        self.tallies.clear();
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

        // Store the per-candidate tally for the Results screen.
        self.tallies.clear();
        self.tallies.reserve(tallies.len());
        for (target, count) in &tallies {
            let label = match target {
                None => "Skip".into(),
                Some(id) => self
                    .options
                    .iter()
                    .find(|o| o.player_id == *id)
                    .map(|o| o.name.clone())
                    .unwrap_or_else(|| "?".into()),
            };
            self.tallies.push((label, *count));
        }
        self.tallies.sort_by_key(|t| std::cmp::Reverse(t.1));

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

        self.timer = SimTimer::from_seconds(results_time.max(0.1), TimerMode::Once);

        *phase = GamePhase::Results;
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_meeting_commands(
    mut queue: ResMut<MeetingCommands>,
    mut phase: ResMut<GamePhase>,
    mut meeting: ResMut<MeetingState>,
    cfg: Res<MatchConfig>,
    mut sabotage: ResMut<ActiveSabotage>,
    mut fix_stations: Query<&mut super::SabotageFixStation>,
    players: Query<(&Player, Option<&Alive>, Option<&Ghost>)>,
    mut living: Query<(&Player, &mut EmergenciesLeft, &EmergencyCooldownLeft), With<Alive>>,
    positions: Query<(&Player, &Position), With<Alive>>,
    local_id: Res<LocalPlayerId>,
    emergency_buttons: Query<&Position, With<EmergencyButton>>,
) {
    let commands = std::mem::take(&mut queue.0);
    for cmd in commands {
        match cmd {
            MeetingCommand::Emergency { actor_id } => {
                if !matches!(*phase, GamePhase::Playing) || sabotage.is_active() {
                    continue;
                }
                let Some((_, mut left, cooldown)) =
                    living.iter_mut().find(|(p, _, _)| p.id == actor_id)
                else {
                    continue;
                };
                if left.0 == 0 || cooldown.0 > 0.0 {
                    continue;
                }
                // Map-gated: the caller must stand at the emergency button.
                let Some((_, position)) = positions.iter().find(|(p, _)| p.id == actor_id) else {
                    continue;
                };
                let near_button = emergency_buttons
                    .iter()
                    .any(|bt| position.0.distance(bt.0) <= cfg.interact_range);
                if !near_button {
                    continue;
                }
                left.0 -= 1;

                if !sabotage.is_critical() {
                    clear_sabotage_world(&mut sabotage, &mut fix_stations);
                }

                meeting.begin_meeting("Emergency Meeting!".into(), &players, cfg.discussion_time);
                *phase = GamePhase::Meeting;
            }
            MeetingCommand::Vote { voter_id, target } => {
                if !matches!(*phase, GamePhase::Voting) || meeting.votes.contains_key(&voter_id) {
                    continue;
                }
                let voter_alive = meeting
                    .options
                    .iter()
                    .any(|o| o.player_id == voter_id && !o.dead);
                let target_alive = meeting
                    .options
                    .iter()
                    .any(|o| o.player_id == target && !o.dead);
                if !voter_alive || !target_alive {
                    continue;
                }
                meeting.votes.insert(voter_id, Some(target));
                if local_id.0 == Some(voter_id) {
                    meeting.local_voted = true;
                }
            }
            MeetingCommand::Skip { voter_id } => {
                if !matches!(*phase, GamePhase::Voting) || meeting.votes.contains_key(&voter_id) {
                    continue;
                }
                let voter_alive = meeting
                    .options
                    .iter()
                    .any(|o| o.player_id == voter_id && !o.dead);
                if !voter_alive {
                    continue;
                }
                meeting.votes.insert(voter_id, None);
                if local_id.0 == Some(voter_id) {
                    meeting.local_voted = true;
                }
            }
        }
    }
}

pub fn cast_missing_bot_votes(
    meeting: &mut MeetingState,
    bots: &Query<&Player, (With<AiPlayer>, With<Alive>)>,
    rng: &mut impl Rng,
) {
    let living_ids: Vec<u64> = meeting
        .options
        .iter()
        .filter(|option| !option.dead)
        .map(|option| option.player_id)
        .collect();

    if living_ids.is_empty() {
        return;
    }

    for bot in bots.iter() {
        if meeting.votes.contains_key(&bot.id) {
            continue;
        }

        if rng.random::<f32>() < 0.25 {
            meeting.votes.insert(bot.id, None);
        } else if let Some(&target) = living_ids.choose(rng) {
            meeting.votes.insert(bot.id, Some(target));
        }
    }
}
