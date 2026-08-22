use std::time::Duration;

use bevy::audio::Volume;
use bevy::prelude::*;

use super::{ActiveSabotage, Body, ChatState, GamePhase, MeetingState, SabotageKind, TaskBoard};
use crate::app::{AppState, Paused};
use game_utils_bevy::audio::AudioChannels;

/// Procedural soundscape using Bevy's built-in Pitch audio source.
/// This avoids committing placeholder .ogg files while the art/audio direction is still fluid.
///
/// Later replacement path:
/// - keep the public cue systems;
/// - replace `Handle<Pitch>` with `Handle<AudioSource>`;
/// - load real files from `assets/audio/sfx/*.ogg`.
pub struct GameAudioPlugin;

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CriticalAlarmTimer>()
            .add_systems(Startup, setup_audio_handles)
            .add_systems(OnEnter(AppState::InGame), reset_audio_locals)
            .add_systems(
                Update,
                (
                    play_phase_cues,
                    play_body_spawn_cue,
                    play_task_complete_cue,
                    play_vote_confirm_cue,
                    play_chat_cue,
                    play_sabotage_cues,
                    play_critical_alarm,
                )
                    .run_if(in_state(AppState::InGame))
                    .run_if(|paused: Res<Paused>| !paused.0),
            );
    }
}

#[derive(Resource)]
pub struct GameAudioHandles {
    role_reveal: Handle<Pitch>,
    meeting: Handle<Pitch>,
    voting: Handle<Pitch>,
    results: Handle<Pitch>,
    crew_win: Handle<Pitch>,
    impostor_win: Handle<Pitch>,

    body: Handle<Pitch>,
    task_done: Handle<Pitch>,
    vote: Handle<Pitch>,
    chat: Handle<Pitch>,

    sabotage_start: Handle<Pitch>,
    alarm: Handle<Pitch>,
    lights: Handle<Pitch>,
}

#[derive(Resource)]
struct CriticalAlarmTimer(Timer);

impl Default for CriticalAlarmTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(0.72, TimerMode::Repeating))
    }
}

/// Reset-only resource used to prevent one-frame false positives when entering a match.
#[derive(Resource, Default)]
struct AudioFrameMemory {
    phase: Option<GamePhase>,
    task_completed: u32,
    votes_len: usize,
    local_voted: bool,
    chat_len: usize,
    sabotage_kind: Option<SabotageKind>,
}

fn setup_audio_handles(mut commands: Commands, mut pitches: ResMut<Assets<Pitch>>) {
    commands.insert_resource(GameAudioHandles {
        role_reveal: pitches.add(Pitch::new(440.0, Duration::from_millis(180))),
        meeting: pitches.add(Pitch::new(740.0, Duration::from_millis(420))),
        voting: pitches.add(Pitch::new(560.0, Duration::from_millis(220))),
        results: pitches.add(Pitch::new(350.0, Duration::from_millis(260))),
        crew_win: pitches.add(Pitch::new(660.0, Duration::from_millis(520))),
        impostor_win: pitches.add(Pitch::new(180.0, Duration::from_millis(700))),

        body: pitches.add(Pitch::new(115.0, Duration::from_millis(180))),
        task_done: pitches.add(Pitch::new(920.0, Duration::from_millis(170))),
        vote: pitches.add(Pitch::new(500.0, Duration::from_millis(90))),
        chat: pitches.add(Pitch::new(980.0, Duration::from_millis(55))),

        sabotage_start: pitches.add(Pitch::new(240.0, Duration::from_millis(420))),
        alarm: pitches.add(Pitch::new(460.0, Duration::from_millis(150))),
        lights: pitches.add(Pitch::new(300.0, Duration::from_millis(260))),
    });

    commands.insert_resource(AudioFrameMemory::default());
}

fn reset_audio_locals(
    mut memory: ResMut<AudioFrameMemory>,
    mut alarm: ResMut<CriticalAlarmTimer>,
    phase: Res<GamePhase>,
    tasks: Option<Res<TaskBoard>>,
    meeting: Option<Res<MeetingState>>,
    chat: Option<Res<ChatState>>,
    sabotage: Option<Res<ActiveSabotage>>,
) {
    memory.phase = Some(*phase);
    memory.task_completed = tasks.as_ref().map(|t| t.completed).unwrap_or(0);
    memory.votes_len = meeting.as_ref().map(|m| m.votes.len()).unwrap_or(0);
    memory.local_voted = meeting.as_ref().map(|m| m.local_voted).unwrap_or(false);
    memory.chat_len = chat.as_ref().map(|c| c.entries.len()).unwrap_or(0);
    memory.sabotage_kind = sabotage.as_ref().and_then(|s| s.kind);
    alarm.0.reset();
}

#[inline]
fn sfx_volume(channels: &AudioChannels, gain: f32) -> Volume {
    Volume::Linear((channels.master * channels.sfx * gain).clamp(0.0, 2.0))
}

fn play_pitch(
    commands: &mut Commands,
    handle: &Handle<Pitch>,
    channels: &AudioChannels,
    gain: f32,
) {
    // Bevy's audio examples use `AudioPlayer(handle)` plus `PlaybackSettings`.
    // DESPAWN prevents old one-shot audio entities from accumulating.
    commands.spawn((
        AudioPlayer(handle.clone()),
        PlaybackSettings::DESPAWN.with_volume(sfx_volume(channels, gain)),
    ));
}

fn play_phase_cues(
    phase: Res<GamePhase>,
    mut memory: ResMut<AudioFrameMemory>,
    mut commands: Commands,
    handles: Res<GameAudioHandles>,
    channels: Res<AudioChannels>,
) {
    if memory.phase == Some(*phase) {
        return;
    }

    match *phase {
        GamePhase::RoleReveal => {
            play_pitch(&mut commands, &handles.role_reveal, &channels, 0.65);
        }
        GamePhase::Meeting => {
            play_pitch(&mut commands, &handles.meeting, &channels, 0.95);
        }
        GamePhase::Voting => {
            play_pitch(&mut commands, &handles.voting, &channels, 0.75);
        }
        GamePhase::Results => {
            play_pitch(&mut commands, &handles.results, &channels, 0.7);
        }
        GamePhase::GameOver { crew_win: true, .. } => {
            play_pitch(&mut commands, &handles.crew_win, &channels, 0.9);
        }
        GamePhase::GameOver {
            crew_win: false, ..
        } => {
            play_pitch(&mut commands, &handles.impostor_win, &channels, 0.95);
        }
        GamePhase::None | GamePhase::Playing => {}
    }

    memory.phase = Some(*phase);
}

fn play_body_spawn_cue(
    added_bodies: Query<&Body, Added<Body>>,
    mut commands: Commands,
    handles: Res<GameAudioHandles>,
    channels: Res<AudioChannels>,
) {
    if added_bodies.is_empty() {
        return;
    }

    // One cue per frame is enough even if a snapshot creates multiple existing bodies.
    play_pitch(&mut commands, &handles.body, &channels, 0.8);
}

fn play_task_complete_cue(
    tasks: Res<TaskBoard>,
    mut memory: ResMut<AudioFrameMemory>,
    mut commands: Commands,
    handles: Res<GameAudioHandles>,
    channels: Res<AudioChannels>,
) {
    if tasks.completed > memory.task_completed {
        play_pitch(&mut commands, &handles.task_done, &channels, 0.75);
    }

    memory.task_completed = tasks.completed;
}

fn play_vote_confirm_cue(
    meeting: Res<MeetingState>,
    mut memory: ResMut<AudioFrameMemory>,
    mut commands: Commands,
    handles: Res<GameAudioHandles>,
    channels: Res<AudioChannels>,
) {
    let votes_len = meeting.votes.len();
    let local_voted = meeting.local_voted;

    // Local confirmation is the most important feedback on clients.
    // Host/local also get a subtle tick when total votes increase.
    if (!memory.local_voted && local_voted) || votes_len > memory.votes_len {
        play_pitch(&mut commands, &handles.vote, &channels, 0.55);
    }

    memory.votes_len = votes_len;
    memory.local_voted = local_voted;
}

fn play_chat_cue(
    chat: Res<ChatState>,
    mut memory: ResMut<AudioFrameMemory>,
    mut commands: Commands,
    handles: Res<GameAudioHandles>,
    channels: Res<AudioChannels>,
) {
    let len = chat.entries.len();

    if len > memory.chat_len {
        play_pitch(&mut commands, &handles.chat, &channels, 0.45);
    }

    memory.chat_len = len;
}

fn play_sabotage_cues(
    sabotage: Res<ActiveSabotage>,
    mut memory: ResMut<AudioFrameMemory>,
    mut commands: Commands,
    handles: Res<GameAudioHandles>,
    channels: Res<AudioChannels>,
) {
    if sabotage.kind == memory.sabotage_kind {
        return;
    }

    if let Some(kind) = sabotage.kind {
        let handle = match kind {
            SabotageKind::Lights => &handles.lights,
            SabotageKind::Oxygen | SabotageKind::Reactor => &handles.sabotage_start,
        };
        play_pitch(&mut commands, handle, &channels, 0.9);
    }

    memory.sabotage_kind = sabotage.kind;
}

fn play_critical_alarm(
    time: Res<Time>,
    sabotage: Res<ActiveSabotage>,
    mut alarm: ResMut<CriticalAlarmTimer>,
    mut commands: Commands,
    handles: Res<GameAudioHandles>,
    channels: Res<AudioChannels>,
) {
    if !sabotage.is_critical() {
        alarm.0.reset();
        return;
    }

    if alarm.0.tick(time.delta()).just_finished() {
        let remaining = sabotage.critical_remaining();

        // Urgency ramp: louder as the timer approaches zero.
        let gain = if remaining <= 8.0 {
            1.0
        } else if remaining <= 15.0 {
            0.78
        } else {
            0.55
        };

        play_pitch(&mut commands, &handles.alarm, &channels, gain);
    }
}
