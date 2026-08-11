use bevy::prelude::*;
use rand::prelude::IndexedRandom;
use std::collections::HashMap;

use super::{Alive, GamePhase, Ghost, MatchConfig, Player, Role};
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
    pub votes: HashMap<u64, Option<u64>>, // voter -> Some(target) or None=skip
    pub local_voted: bool,
    pub result_text: String,
    pub emergencies_left: u8,
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
    }

    pub fn all_voted(&self) -> bool {
        let living = self.options.iter().filter(|o| !o.dead).count();
        living > 0 && self.votes.len() >= living
    }

    pub fn resolve_votes(&mut self, phase: &mut GamePhase) {
        let mut tallies: HashMap<Option<u64>, u32> = HashMap::new();
        for opt in self.votes.values() {
            *tallies.entry(*opt).or_default() += 1;
        }
        // find max non-skip
        let mut best: Option<(Option<u64>, u32)> = None;
        for (k, v) in &tallies {
            if best.map(|(_, bv)| *v > bv).unwrap_or(true) {
                best = Some((*k, *v));
            }
        }
        // tie → skip
        let mut winners = 0u32;
        if let Some((_, bv)) = best {
            winners = tallies.values().filter(|v| **v == bv).count() as u32;
        }
        if winners != 1 || best.map(|(id, _)| id.is_none()).unwrap_or(true) {
            self.result_text = "No one was ejected. (Skip/Tie)".into();
        } else if let Some((Some(id), _)) = best {
            if let Some(opt) = self.options.iter().find(|o| o.player_id == id) {
                self.result_text = format!("{} was ejected.", opt.name);
            } else {
                self.result_text = "Ejected.".into();
            }
            // actual eject applied in system below via flag
            self.votes.insert(0, Some(id)); // stash eject id under key 0 temporarily
        } else {
            self.result_text = "No one was ejected.".into();
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
                handle_meeting_commands,
                apply_eject_on_results,
                emergency_hotkey,
            )
                .run_if(in_state(AppState::InGame))
                .run_if(|p: Res<Paused>| !p.0)
                .run_if(|t: Res<Transition<AppState>>| !t.block_input),
        );
    }
}

fn emergency_hotkey(
    keys: Res<ButtonInput<KeyCode>>,
    mut ev: MessageWriter<MeetingCommand>,
    phase: Res<GamePhase>,
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
    local: Query<&Player, With<super::LocalPlayer>>,
    mut trauma: ResMut<Trauma>,
) {
    for cmd in ev.read() {
        match cmd {
            MeetingCommand::Emergency => {
                if !matches!(*phase, GamePhase::Playing) {
                    continue;
                }
                if meeting.emergencies_left == 0 {
                    continue;
                }
                // only living local
                let Ok(lp) = local.single() else { continue };
                let living_local = players.iter().any(|(p, a, _)| p.id == lp.id && a.is_some());
                if !living_local {
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
                let Ok(lp) = local.single() else { continue };
                meeting.votes.insert(lp.id, Some(*target));
                meeting.local_voted = true;
                // bots instant-vote randomly for sandbox
                bot_votes(&mut meeting, &players, lp.id);
            }
            MeetingCommand::Skip => {
                if !matches!(*phase, GamePhase::Voting) || meeting.local_voted {
                    continue;
                }
                let Ok(lp) = local.single() else { continue };
                meeting.votes.insert(lp.id, None);
                meeting.local_voted = true;
                bot_votes(&mut meeting, &players, lp.id);
            }
        }
    }
}

fn bot_votes(
    meeting: &mut MeetingState,
    players: &Query<(&Player, Option<&Alive>, Option<&Ghost>)>,
    local_id: u64,
) {
    let living_ids: Vec<u64> = meeting
        .options
        .iter()
        .filter(|o| !o.dead)
        .map(|o| o.player_id)
        .collect();
    let mut rng = rand::rng();
    for (p, alive, _) in players.iter() {
        if p.id == local_id || alive.is_none() {
            continue;
        }
        if meeting.votes.contains_key(&p.id) {
            continue;
        }
        // random skip or vote
        if rand::random::<f32>() < 0.25 {
            meeting.votes.insert(p.id, None);
        } else if let Some(&tid) = living_ids.choose(&mut rng) {
            meeting.votes.insert(p.id, Some(tid));
        }
    }
}

fn apply_eject_on_results(
    phase: Res<GamePhase>,
    mut meeting: ResMut<MeetingState>,
    mut commands: Commands,
    mut q: Query<(Entity, &Player, &mut Sprite, Option<&Alive>, Option<&Role>)>,
    mut trauma: ResMut<Trauma>,
) {
    if !matches!(*phase, GamePhase::Results) {
        return;
    }
    // eject id stashed under votes key 0 by resolve_votes
    let eject_id = meeting.votes.get(&0).copied().flatten();
    let Some(eid) = eject_id else {
        return;
    };
    // only once
    meeting.votes.remove(&0);
    for (e, p, mut sprite, alive, role) in &mut q {
        if p.id != eid || alive.is_none() {
            continue;
        }
        commands.entity(e).remove::<Alive>();
        commands.entity(e).insert(Ghost);
        sprite.color = Color::srgba(0.7, 0.7, 0.8, 0.35);
        ScreenEffects::add_trauma(&mut trauma, 0.5);
        if let Some(Role::Impostor) = role {
            meeting.result_text = format!("{} was an Impostor.", p.name);
        } else {
            meeting.result_text = format!("{} was not an Impostor.", p.name);
        }
    }
}
