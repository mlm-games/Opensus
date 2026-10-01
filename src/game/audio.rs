use bevy_ecs::prelude::*;

use super::{
    ActiveSabotage, Body, ChatState, GamePhase, MeetingState, SabotageKind, SimTimer, TaskBoard,
    TimerMode,
};

const CRITICAL_ALARM_SECS: f32 = 0.72;

#[derive(Resource, Default)]
pub struct PendingCues(pub Vec<&'static str>);

pub struct ToneCue {
    pub name: &'static str,
    pub freq: f32,
    pub millis: u64,
    pub gain: f32,
}

pub const TONE_CUES: &[ToneCue] = &[
    ToneCue {
        name: "role_reveal",
        freq: 440.0,
        millis: 180,
        gain: 0.65,
    },
    ToneCue {
        name: "meeting",
        freq: 740.0,
        millis: 420,
        gain: 0.95,
    },
    ToneCue {
        name: "voting",
        freq: 560.0,
        millis: 220,
        gain: 0.75,
    },
    ToneCue {
        name: "results",
        freq: 350.0,
        millis: 260,
        gain: 0.7,
    },
    ToneCue {
        name: "crew_win",
        freq: 660.0,
        millis: 520,
        gain: 0.9,
    },
    ToneCue {
        name: "impostor_win",
        freq: 180.0,
        millis: 700,
        gain: 0.95,
    },
    ToneCue {
        name: "body",
        freq: 115.0,
        millis: 180,
        gain: 0.8,
    },
    ToneCue {
        name: "task_done",
        freq: 920.0,
        millis: 170,
        gain: 0.75,
    },
    ToneCue {
        name: "vote",
        freq: 500.0,
        millis: 90,
        gain: 0.55,
    },
    ToneCue {
        name: "chat",
        freq: 980.0,
        millis: 55,
        gain: 0.45,
    },
    ToneCue {
        name: "sabotage_start",
        freq: 240.0,
        millis: 420,
        gain: 0.9,
    },
    ToneCue {
        name: "lights",
        freq: 300.0,
        millis: 260,
        gain: 0.9,
    },
    ToneCue {
        name: "alarm",
        freq: 460.0,
        millis: 150,
        gain: 0.55,
    },
    ToneCue {
        name: "alarm_warn",
        freq: 460.0,
        millis: 150,
        gain: 0.78,
    },
    ToneCue {
        name: "alarm_crit",
        freq: 460.0,
        millis: 150,
        gain: 1.0,
    },
];

#[derive(Resource)]
pub struct CriticalAlarmTimer(pub SimTimer);

impl Default for CriticalAlarmTimer {
    fn default() -> Self {
        Self(SimTimer::from_seconds(
            CRITICAL_ALARM_SECS,
            TimerMode::Repeating,
        ))
    }
}

#[derive(Resource, Default)]
pub struct AudioFrameMemory {
    phase: Option<GamePhase>,
    task_completed: u32,
    votes_len: usize,
    local_voted: bool,
    sabotage_kind: Option<SabotageKind>,
    chat_len: usize,
}

pub fn reset_audio_locals(world: &mut World) {
    let phase = *world.resource::<GamePhase>();
    let task_completed = world.resource::<TaskBoard>().completed;
    let (votes_len, local_voted) = {
        let meeting = world.resource::<MeetingState>();
        (meeting.votes.len(), meeting.local_voted)
    };
    let sabotage_kind = world.resource::<ActiveSabotage>().kind;
    let chat_len = world.resource::<ChatState>().entries.len();
    {
        let mut memory = world.resource_mut::<AudioFrameMemory>();
        memory.phase = Some(phase);
        memory.task_completed = task_completed;
        memory.votes_len = votes_len;
        memory.local_voted = local_voted;
        memory.sabotage_kind = sabotage_kind;
        memory.chat_len = chat_len;
    }
    world.resource_mut::<CriticalAlarmTimer>().0 =
        SimTimer::from_seconds(CRITICAL_ALARM_SECS, TimerMode::Repeating);
}

pub fn play_phase_cues(
    phase: Res<GamePhase>,
    mut memory: ResMut<AudioFrameMemory>,
    mut pending: ResMut<PendingCues>,
) {
    if memory.phase == Some(*phase) {
        return;
    }

    let cue = match *phase {
        GamePhase::RoleReveal => Some("role_reveal"),
        GamePhase::Meeting => Some("meeting"),
        GamePhase::Voting => Some("voting"),
        GamePhase::Results => Some("results"),
        GamePhase::GameOver { crew_win: true, .. } => Some("crew_win"),
        GamePhase::GameOver {
            crew_win: false, ..
        } => Some("impostor_win"),
        GamePhase::None | GamePhase::Playing => None,
    };
    if let Some(cue) = cue {
        pending.0.push(cue);
    }

    memory.phase = Some(*phase);
}

pub fn play_body_spawn_cue(
    added_bodies: Query<&Body, Added<Body>>,
    mut pending: ResMut<PendingCues>,
) {
    if added_bodies.is_empty() {
        return;
    }

    pending.0.push("body");
}

pub fn play_task_complete_cue(
    tasks: Res<TaskBoard>,
    mut memory: ResMut<AudioFrameMemory>,
    mut pending: ResMut<PendingCues>,
) {
    if tasks.completed > memory.task_completed {
        pending.0.push("task_done");
    }

    memory.task_completed = tasks.completed;
}

pub fn play_vote_confirm_cue(
    meeting: Res<MeetingState>,
    mut memory: ResMut<AudioFrameMemory>,
    mut pending: ResMut<PendingCues>,
) {
    let votes_len = meeting.votes.len();
    let local_voted = meeting.local_voted;

    if (!memory.local_voted && local_voted) || votes_len > memory.votes_len {
        pending.0.push("vote");
    }

    memory.votes_len = votes_len;
    memory.local_voted = local_voted;
}

pub fn play_chat_cue(
    chat: Res<ChatState>,
    mut memory: ResMut<AudioFrameMemory>,
    mut pending: ResMut<PendingCues>,
) {
    let len = chat.entries.len();
    if len > memory.chat_len {
        pending.0.push("chat");
    }
    memory.chat_len = len;
}

pub fn play_sabotage_cues(
    sabotage: Res<ActiveSabotage>,
    mut memory: ResMut<AudioFrameMemory>,
    mut pending: ResMut<PendingCues>,
) {
    if sabotage.kind == memory.sabotage_kind {
        return;
    }

    if let Some(kind) = sabotage.kind {
        let cue = match kind {
            SabotageKind::Lights => "lights",
            SabotageKind::Oxygen | SabotageKind::Reactor => "sabotage_start",
        };
        pending.0.push(cue);
    }

    memory.sabotage_kind = sabotage.kind;
}

pub fn play_critical_alarm(
    time: Res<repame_sim::SimTime>,
    sabotage: Res<ActiveSabotage>,
    mut alarm: ResMut<CriticalAlarmTimer>,
    mut pending: ResMut<PendingCues>,
) {
    if !sabotage.is_critical() {
        alarm.0 = SimTimer::from_seconds(CRITICAL_ALARM_SECS, TimerMode::Repeating);
        return;
    }

    alarm.0.tick(time.delta_secs);
    if alarm.0.just_finished() {
        let remaining = sabotage.critical_remaining();

        let cue = if remaining <= 8.0 {
            "alarm_crit"
        } else if remaining <= 15.0 {
            "alarm_warn"
        } else {
            "alarm"
        };

        pending.0.push(cue);
    }
}
