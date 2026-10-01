use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{ApplyDeferred, Schedule};
use repame_fx::TransitionFx;

use super::{
    GamePhase, RuntimeMode, ai_brain, ai_ghost_brain, apply_authority_chat, apply_intent_movement,
    apply_pending_eject, apply_sabotage, capture_chat_text, check_sabotage_loss,
    check_win_conditions, cleanup_bodies_on_meeting, cleanup_on_game_over_enter,
    clear_fixed_sabotage, do_kill, do_report, ensure_bot_votes, handle_meeting_commands,
    local_intent_and_move, play_body_spawn_cue, play_chat_cue, play_critical_alarm,
    play_phase_cues, play_sabotage_cues, play_task_complete_cue, play_vote_confirm_cue,
    process_interactions, read_local_action_edges, read_local_sabotage_edges,
    reset_cooldowns_after_meeting, tick_emergency_cooldowns, tick_kill_cds, tick_phase_timers,
    tick_sabotage, tick_trauma, update_local_prompt,
};
use crate::app::{AppState, Paused};

/// Authority world-mutating step ordering inside a running match.
///
/// `Input` collects every peer's intent (all modes, no authority gate).
/// `Resolve` applies intents/actions on the authority. `Phase` advances
/// timers and transitions. `Win` resolves whether the match is over.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameSimSet {
    Receive,
    Input,
    Resolve,
    Phase,
    Win,
    Send,
}

/// Ordered slices inside `GameSimSet::Resolve`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResolveStep {
    Ai,       // bot brains + non-local movement (authority)
    Combat,   // kills, reports, emergency meeting commands, kill CD
    Interact, // tasks + sabotage fixes
    Sabotage,
}

pub fn in_game(state: Res<AppState>) -> bool {
    *state == AppState::InGame
}

pub fn not_paused(paused: Res<Paused>) -> bool {
    !paused.0
}

pub fn in_playing(phase: Res<GamePhase>) -> bool {
    matches!(*phase, GamePhase::Playing)
}

pub fn has_authority(mode: Res<RuntimeMode>) -> bool {
    mode.has_authority()
}

pub fn chat_open(phase: Res<GamePhase>) -> bool {
    matches!(*phase, GamePhase::Meeting | GamePhase::Voting)
}

/// In a running match, not paused, not mid-transition.
pub fn gameplay_active(
    state: Res<AppState>,
    paused: Res<Paused>,
    transition: Option<Res<TransitionFx>>,
) -> bool {
    *state == AppState::InGame && !paused.0 && !transition.is_some_and(|t| t.blocking())
}

pub fn build_game_schedule() -> Schedule {
    let mut schedule = Schedule::default();

    schedule.configure_sets(
        (
            GameSimSet::Input,
            GameSimSet::Resolve,
            GameSimSet::Phase,
            GameSimSet::Win,
        )
            .chain()
            .run_if(in_game),
    );
    schedule.configure_sets(
        (
            ResolveStep::Ai,
            ResolveStep::Combat,
            ResolveStep::Interact,
            ResolveStep::Sabotage,
        )
            .chain()
            .in_set(GameSimSet::Resolve),
    );

    schedule.add_systems(
        (
            read_local_action_edges
                .in_set(GameSimSet::Input)
                .run_if(gameplay_active),
            (local_intent_and_move, update_local_prompt)
                .chain()
                .in_set(GameSimSet::Input)
                .run_if(gameplay_active)
                .run_if(in_playing),
            tick_emergency_cooldowns
                .in_set(GameSimSet::Input)
                .run_if(in_game)
                .run_if(not_paused)
                .run_if(in_playing)
                .run_if(has_authority),
            read_local_sabotage_edges
                .in_set(GameSimSet::Input)
                .run_if(gameplay_active),
            (
                capture_chat_text.run_if(chat_open),
                apply_authority_chat.run_if(has_authority),
            )
                .chain()
                .in_set(GameSimSet::Input)
                .run_if(gameplay_active),
        )
            .chain(),
    );

    schedule.add_systems(
        (
            (
                ai_brain,
                ai_ghost_brain,
                apply_intent_movement.run_if(in_playing),
            )
                .chain()
                .in_set(ResolveStep::Ai),
            tick_kill_cds.in_set(ResolveStep::Combat).run_if(in_playing),
            do_kill.in_set(ResolveStep::Combat).run_if(in_playing),
            do_report.in_set(ResolveStep::Combat).run_if(in_playing),
            handle_meeting_commands.in_set(ResolveStep::Combat),
            process_interactions
                .in_set(ResolveStep::Interact)
                .run_if(in_playing),
            (
                apply_sabotage,
                tick_sabotage,
                check_sabotage_loss,
                clear_fixed_sabotage,
            )
                .chain()
                .in_set(ResolveStep::Sabotage),
        )
            .chain()
            .run_if(gameplay_active)
            .run_if(has_authority),
    );

    // Phase chain: bots vote BEFORE timers resolve voting.
    schedule.add_systems(
        (
            cleanup_bodies_on_meeting,
            ensure_bot_votes, // BEFORE tick — critical
            tick_phase_timers,
            apply_pending_eject, // same frame Results starts
            reset_cooldowns_after_meeting,
            tick_trauma,
        )
            .chain()
            .in_set(GameSimSet::Phase)
            .run_if(gameplay_active)
            .run_if(has_authority),
    );

    // Win ALWAYS last (after kills/tasks/sabotage resolve + phase/eject).
    schedule.add_systems(
        (check_win_conditions, cleanup_on_game_over_enter)
            .chain()
            .in_set(GameSimSet::Win)
            .run_if(in_game)
            .run_if(not_paused)
            .run_if(has_authority),
    );

    schedule.add_systems(
        (
            play_phase_cues,
            play_body_spawn_cue,
            play_chat_cue,
            play_task_complete_cue,
            play_vote_confirm_cue,
            play_sabotage_cues,
            play_critical_alarm,
        )
            .run_if(in_game)
            .run_if(not_paused),
    );

    schedule.add_systems(
        (
            ApplyDeferred
                .after(GameSimSet::Resolve)
                .before(GameSimSet::Phase),
            ApplyDeferred
                .after(GameSimSet::Phase)
                .before(GameSimSet::Win),
        )
            .run_if(in_game),
    );

    super::networking::register_schedule(&mut schedule);

    schedule
}
