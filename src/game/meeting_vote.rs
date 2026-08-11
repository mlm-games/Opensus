use bevy::prelude::*;
use rand::prelude::IndexedRandom;
use std::collections::HashMap;

use super::{Alive, GamePhase, Ghost, LocalPlayer, MatchConfig, Player, Role};
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
    pub emergencies_left: u8,
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

    pub fn all_voted(&self) -> bool {
        let living = self.options.iter().filter(|o| !o.dead).count();
        living > 0 && self.votes.len() >= living
    }

    pub fn resolve_votes(&mut self, phase: &mut GamePhase) {
        let mut tallies: HashMap<Option<u64>, u32> = HashMap::new();
        for v in self.votes.values() {
            *tallies.entry(*v).or_default() += 1;
        }
        let max = tallies.values().copied().max().unwrap_or(0);
        let top: Vec<Option<u64>> = tallies
            .iter()
            .filter(|(_, c)| **c == max)
            .map(|(k, _)| *k)
            .collect();

        self.pending_eject = None;
        if max == 0 || top.len() != 1 || top[0].is_none() {
            self.result_text = "No one was ejected. (Skip / Tie)".into();
        } else if let Some(id) = top[0] {
            let name = self
                .options
                .iter()
                .find(|o| o.player_id == id)
                .map(|o| o.name.clone())
                .unwrap_or_default();
            self.result_text = format!("{name} was ejected.");
            self.pending_eject = Some(id);
        }
        self.timer = Timer::from_seconds(5.0, TimerMode::Once);
        *phase = GamePhase::Results;
    }
}

#[derive(Message, Clone)]
pub enum MeetingCommand {
    Emergency,
    Vote(u64),
    Skip,
}

pub struct MeetingVotePlugin;

impl Plugin for MeetingVotePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::InGame),
            |mut m: ResMut<MeetingState>, cfg: Res<MatchConfig>| {
                m.emergencies_left = cfg.emergency_meetings;
                m.clear_for_play();
            },
        )
        .add_systems(
            Update,
            (
                emergency_hotkey,
                handle_meeting_commands,
                apply_eject_on_results,
            )
                .run_if(in_state(AppState::InGame))
                .run_if(|p: Res<Paused>| !p.0)
                .run_if(|t: Res<Transition<AppState>>| !t.block_input),
        );
    }
}

fn emergency_hotkey(
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<GamePhase>,
    mut ev: MessageWriter<MeetingCommand>,
) {
    if matches!(*phase, GamePhase::Playing) && keys.just_pressed(KeyCode::KeyF) {
        ev.write(MeetingCommand::Emergency);
    }
}

fn handle_meeting_commands(
    mut ev: MessageReader<MeetingCommand>,
    mut phase: ResMut<GamePhase>,
    mut meeting: ResMut<MeetingState>,
    cfg: Res<MatchConfig>,
    players: Query<(&Player, Option<&Alive>, Option<&Ghost>)>,
    local: Query<(&Player, Option<&Alive>), With<LocalPlayer>>,
    mut trauma: ResMut<Trauma>,
) {
    for cmd in ev.read() {
        match cmd {
            MeetingCommand::Emergency => {
                if !matches!(*phase, GamePhase::Playing) || meeting.emergencies_left == 0 {
                    continue;
                }
                // Only a living local player can call a meeting.
                let Ok((_, alive)) = local.single() else {
                    continue;
                };
                if alive.is_none() {
                    continue;
                }
                meeting.emergencies_left -= 1;
                ScreenEffects::add_trauma(&mut trauma, 0.35);
                meeting.begin_meeting("Emergency Meeting!".into(), &players, cfg.discussion_time);
                *phase = GamePhase::Meeting;
            }
            MeetingCommand::Vote(target) => {
                if !matches!(*phase, GamePhase::Voting) || meeting.local_voted {
                    continue;
                }
                let Ok((lp, _)) = local.single() else {
                    continue;
                };
                let local_id = lp.id;
                meeting.votes.insert(local_id, Some(*target));
                meeting.local_voted = true;
                bot_votes(&mut meeting, &players, local_id);
            }
            MeetingCommand::Skip => {
                if !matches!(*phase, GamePhase::Voting) || meeting.local_voted {
                    continue;
                }
                let Ok((lp, _)) = local.single() else {
                    continue;
                };
                let local_id = lp.id;
                meeting.votes.insert(local_id, None);
                meeting.local_voted = true;
                bot_votes(&mut meeting, &players, local_id);
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

fn apply_eject_on_results(
    phase: Res<GamePhase>,
    mut meeting: ResMut<MeetingState>,
    mut commands: Commands,
    mut q: Query<(Entity, &Player, &mut Sprite, &Role), With<Alive>>,
    mut trauma: ResMut<Trauma>,
) {
    if !matches!(*phase, GamePhase::Results) {
        return;
    }
    // take() guarantees this fires exactly once per resolution.
    let Some(eid) = meeting.pending_eject.take() else {
        return;
    };
    for (e, p, mut sprite, role) in &mut q {
        if p.id != eid {
            continue;
        }
        commands.entity(e).remove::<Alive>();
        commands.entity(e).insert(Ghost);
        sprite.color = Color::srgba(0.7, 0.7, 0.8, 0.35);
        ScreenEffects::add_trauma(&mut trauma, 0.5);
        meeting.result_text = if matches!(role, Role::Impostor) {
            format!("{} was an Impostor.", p.name)
        } else {
            format!("{} was not an Impostor.", p.name)
        };
        break;
    }
}
