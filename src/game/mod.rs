mod assets;
mod authority;
mod interaction;
mod kill_sabotage;
mod lobby;
mod map;
mod meeting_vote;
mod networking;
mod phases;
mod player;
mod roles;
mod sabotage;
mod tasks;
mod vision;

pub use assets::*;
pub use authority::*;
pub use interaction::*;
pub use kill_sabotage::*;
pub use lobby::*;
pub use map::*;
pub use meeting_vote::*;
pub use networking::*;
pub use phases::*;
pub use player::*;
pub use roles::*;
pub use sabotage::*;
pub use tasks::*;
pub use vision::*;

use bevy::ecs::schedule::ApplyDeferred;
use bevy::prelude::*;

use crate::app::{AppState, Paused};
use game_utils_bevy::screen_effects::{ScreenEffects, Trauma};
use game_utils_bevy::transitions::Transition;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RuntimeMode>()
            .init_resource::<GamePhase>()
            .init_resource::<MatchConfig>()
            .init_resource::<LobbyState>()
            .init_resource::<TaskBoard>()
            .init_resource::<LocalRole>()
            .init_resource::<LocalPlayerId>()
            .init_resource::<MeetingState>()
            .init_resource::<MatchStats>()
            .add_message::<StartMatchRequest>()
            .add_message::<MeetingCommand>()
            .add_message::<KillRequest>()
            .add_message::<ReportBody>()
            .add_message::<SabotageAction>()
            .configure_sets(
                Update,
                (
                    GameSimSet::Input,
                    GameSimSet::Resolve,
                    GameSimSet::Phase,
                    GameSimSet::Win,
                )
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            )
            .configure_sets(
                Update,
                (
                    ResolveStep::Ai,
                    ResolveStep::Combat,
                    ResolveStep::Interact,
                    ResolveStep::Sabotage,
                )
                    .chain()
                    .in_set(GameSimSet::Resolve),
            )
            .add_plugins((
                LobbyPlugin,
                PlayerPlugin,
                MapPlugin,
                TasksPlugin,
                InteractionPlugin,
                KillSabotagePlugin,
                MeetingVotePlugin,
                SabotagePlugin,
                VisionPlugin,
                NetworkingPlugin,
                GameAssetsPlugin,
            ))
            .add_systems(OnEnter(AppState::InGame), setup_match)
            .add_systems(OnExit(AppState::InGame), cleanup_match)
            .add_systems(
                Update,
                (
                    ApplyDeferred
                        .after(GameSimSet::Resolve)
                        .before(GameSimSet::Phase),
                    ApplyDeferred
                        .after(GameSimSet::Phase)
                        .before(GameSimSet::Win),
                )
                    .run_if(in_state(AppState::InGame)),
            )
            // Phase chain: bots vote BEFORE timers resolve voting.
            .add_systems(
                Update,
                (
                    cleanup_bodies_on_meeting,
                    ensure_bot_votes, // BEFORE tick — critical
                    tick_phase_timers,
                    apply_pending_eject, // same frame Results starts
                )
                    .chain()
                    .in_set(GameSimSet::Phase)
                    .run_if(in_state(AppState::InGame))
                    .run_if(|paused: Res<Paused>| !paused.0)
                    .run_if(|transition: Res<Transition<AppState>>| !transition.block_input)
                    .run_if(has_authority),
            )
            // Win ALWAYS last (after kills/tasks/sabotage resolve + phase/eject).
            .add_systems(
                Update,
                (check_win_conditions, cleanup_on_game_over_enter)
                    .chain()
                    .in_set(GameSimSet::Win)
                    .run_if(in_state(AppState::InGame))
                    .run_if(|paused: Res<Paused>| !paused.0)
                    .run_if(has_authority),
            );
    }
}

/// Authority world-mutating step ordering inside a running match.
///
/// `Input` collects every peer's intent (all modes, no authority gate).
/// `Resolve` applies intents/actions on the authority. `Phase` advances
/// timers and transitions. `Win` resolves whether the match is over.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameSimSet {
    Input,
    Resolve,
    Phase,
    Win,
}

/// Ordered slices inside `GameSimSet::Resolve`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResolveStep {
    Ai,       // bot brains + non-local movement (authority)
    Combat,   // kills, reports, emergency meeting commands, kill CD
    Interact, // tasks + sabotage fixes
    Sabotage, // start sabotage, tick, loss, clear-fixed
}

#[derive(Component)]
pub struct MatchCleanup;

/// Snapshot of how the match was seeded — used so win rules don't fire
/// spuriously (e.g. 0 impostors assigned).
#[derive(Resource, Default, Clone, Debug)]
pub struct MatchStats {
    pub impostors_spawned: u32,
    pub players_spawned: u32,
}

pub(crate) fn setup_match(
    mut phase: ResMut<GamePhase>,
    mut tasks: ResMut<TaskBoard>,
    mut meeting: ResMut<MeetingState>,
) {
    *phase = GamePhase::Playing;
    tasks.completed = 0;
    // `total` is owned by spawn_task_stations (runs after this system).
    // `MatchStats` is owned exclusively by spawn_players_from_lobby.
    *meeting = MeetingState::default();
}

fn cleanup_match(
    mut commands: Commands,
    q: Query<Entity, With<MatchCleanup>>,
    mut stats: ResMut<MatchStats>,
    mut meeting: ResMut<MeetingState>,
    mut sabotage: ResMut<ActiveSabotage>,
) {
    for e in &q {
        commands.entity(e).despawn();
    }
    // Leave GameOver visible until UI navigates away; only clear stats.
    // Phase is cleared by PlayAgain / QuitToTitle.
    *stats = MatchStats::default();
    meeting.clear_for_play();
    sabotage.clear();
}

fn cleanup_bodies_on_meeting(
    phase: Res<GamePhase>,
    mut previous: Local<Option<GamePhase>>,
    mut commands: Commands,
    bodies: Query<Entity, With<Body>>,
) {
    let entered_meeting =
        matches!(*phase, GamePhase::Meeting) && !matches!(*previous, Some(GamePhase::Meeting));

    *previous = Some(*phase);

    if !entered_meeting {
        return;
    }

    for entity in &bodies {
        commands.entity(entity).despawn();
    }
}

fn tick_phase_timers(
    time: Res<Time>,
    mut phase: ResMut<GamePhase>,
    mut meeting: ResMut<MeetingState>,
    cfg: Res<MatchConfig>,
    tasks: Res<TaskBoard>,
    players: Query<&Role, (With<Player>, With<Alive>)>,
    stats: Res<MatchStats>,
    mut save: ResMut<crate::save::SaveData>,
    manager: Res<game_utils_bevy::save::SaveManager>,
) {
    // Never advance meeting timers after the match is decided.
    if matches!(*phase, GamePhase::GameOver { .. } | GamePhase::None) {
        return;
    }

    match *phase {
        GamePhase::Meeting => {
            meeting.timer.tick(time.delta());
            if meeting.timer.just_finished() {
                *phase = GamePhase::Voting;
                meeting.timer = Timer::from_seconds(cfg.voting_time, TimerMode::Once);
                meeting.prompt = "Vote".into();
            }
        }
        GamePhase::Voting => {
            meeting.timer.tick(time.delta());
            // bots already filled this frame via ensure_bot_votes (runs before us)
            if meeting.timer.just_finished() || meeting.all_voted() {
                meeting.resolve_votes(&mut phase, cfg.results_time);
            }
        }
        GamePhase::Results => {
            meeting.timer.tick(time.delta());
            if meeting.timer.just_finished() {
                // Eject already applied on Results entry (prior frames + ApplyDeferred).
                // Decide here so we never flash Playing on a decided match.
                if let Some(reason) = compute_win(&tasks, &players, &stats) {
                    apply_game_over(&mut phase, reason, &mut save, &manager);
                } else {
                    *phase = GamePhase::Playing;
                }
                meeting.clear_for_play();
            }
        }
        _ => {}
    }
}
fn apply_pending_eject(
    phase: Res<GamePhase>,
    mut meeting: ResMut<MeetingState>,
    mut commands: Commands,
    mut q: Query<(Entity, &Player, Option<&Children>, &Role), With<Alive>>,
    mut sprites: Query<&mut Sprite>,
    mut trauma: ResMut<Trauma>,
) {
    // Only consume eject once we've entered Results.
    if !matches!(*phase, GamePhase::Results) {
        return;
    }
    let Some(eid) = meeting.pending_eject.take() else {
        return;
    };

    for (e, p, children, role) in &mut q {
        if p.id != eid {
            continue;
        }
        make_ghost(&mut commands, e, children, &mut sprites);
        ScreenEffects::add_trauma(&mut trauma, 0.5);
        meeting.result_text = if matches!(role, Role::Impostor) {
            format!("{} was an Impostor.", p.name)
        } else {
            format!("{} was not an Impostor.", p.name)
        };
        break;
    }
}

fn ensure_bot_votes(
    phase: Res<GamePhase>,
    mut meeting: ResMut<MeetingState>,
    players: Query<(&Player, Option<&Alive>, Option<&Ghost>)>,
    local_id: Res<LocalPlayerId>,
) {
    if !matches!(*phase, GamePhase::Voting) {
        return;
    }
    let skip = local_id.0.unwrap_or(u64::MAX);
    crate::game::meeting_vote::bot_votes_public(&mut meeting, &players, skip);
}

/// Living crew / impostor counts (Alive only).
pub fn living_role_counts(players: &Query<&Role, (With<Player>, With<Alive>)>) -> (u32, u32) {
    let mut crew = 0u32;
    let mut imps = 0u32;
    for role in players.iter() {
        match role {
            Role::Crewmate => crew += 1,
            Role::Impostor => imps += 1,
        }
    }
    (crew, imps)
}

/// Pure win rule (kill/task/eject). Sabotage has its own authority path.
/// `Some(reason)` ends the match.
pub fn compute_win_from_counts(
    tasks: &TaskBoard,
    crew: u32,
    imps: u32,
    stats: &MatchStats,
) -> Option<WinReason> {
    let living = crew.saturating_add(imps);

    // 1) Shared task bar.
    if tasks.total > 0 && tasks.completed >= tasks.total {
        return Some(WinReason::Tasks);
    }

    // No living players: do not invent a winner.
    if living == 0 {
        return None;
    }

    // 2) All impostors gone — only if the match actually seeded impostors.
    if imps == 0 && stats.impostors_spawned > 0 {
        return Some(WinReason::ImpostorsEliminated);
    }

    // 3) Impostor majority (1v1 ⇒ impostors).
    if stats.impostors_spawned > 0 && imps >= crew {
        return Some(WinReason::ImpostorMajority);
    }

    None
}

/// Convenience over a live query.
pub fn compute_win(
    tasks: &TaskBoard,
    players: &Query<&Role, (With<Player>, With<Alive>)>,
    stats: &MatchStats,
) -> Option<WinReason> {
    let (crew, imps) = living_role_counts(players);
    compute_win_from_counts(tasks, crew, imps, stats)
}

fn check_win_conditions(
    mut phase: ResMut<GamePhase>,
    tasks: Res<TaskBoard>,
    players: Query<&Role, (With<Player>, With<Alive>)>,
    stats: Res<MatchStats>,
    mut save: ResMut<crate::save::SaveData>,
    manager: Res<game_utils_bevy::save::SaveManager>,
) {
    // Open play only. Results decides at timer end inside tick_phase_timers
    // so the eject/role-reveal UI always gets a full beat.
    if !matches!(*phase, GamePhase::Playing) {
        return;
    }
    if let Some(reason) = compute_win(&tasks, &players, &stats) {
        apply_game_over(&mut phase, reason, &mut save, &manager);
    }
}

pub fn apply_game_over(
    phase: &mut GamePhase,
    reason: WinReason,
    save: &mut crate::save::SaveData,
    manager: &game_utils_bevy::save::SaveManager,
) {
    if matches!(*phase, GamePhase::GameOver { .. }) {
        return;
    }
    let crew_win = reason.crew_win();
    *phase = GamePhase::GameOver { crew_win, reason };
    save.games_played = save.games_played.saturating_add(1);
    if crew_win {
        save.crew_wins = save.crew_wins.saturating_add(1);
    } else {
        save.impostor_wins = save.impostor_wins.saturating_add(1);
    }
    if let Err(e) = manager.save(&*save) {
        warn!("failed to save match result: {e}");
    }
}

/// Runs once when entering GameOver: stop sabotage HUD/timers so nothing
/// “expires” under the victory screen and rematches start clean.
fn cleanup_on_game_over_enter(
    phase: Res<GamePhase>,
    mut previous: Local<Option<GamePhase>>,
    mut sabotage: ResMut<ActiveSabotage>,
    mut stations: Query<(&mut SabotageFixStation, &mut Sprite)>,
) {
    let entered = matches!(*phase, GamePhase::GameOver { .. })
        && !matches!(*previous, Some(GamePhase::GameOver { .. }));
    *previous = Some(*phase);
    if !entered {
        return;
    }
    sabotage.clear();
    for (mut station, mut sprite) in &mut stations {
        station.progress = 0.0;
        sprite.color = Color::srgba(1.0, 1.0, 1.0, 0.0);
    }
}

#[cfg(test)]
mod win_tests {
    use super::*;

    fn stats(imps: u32, players: u32) -> MatchStats {
        MatchStats {
            impostors_spawned: imps,
            players_spawned: players,
        }
    }

    fn board(done: u32, total: u32) -> TaskBoard {
        TaskBoard {
            completed: done,
            total,
        }
    }

    #[test]
    fn tasks_complete_crew_win() {
        assert_eq!(
            compute_win_from_counts(&board(4, 4), 3, 1, &stats(1, 4)),
            Some(WinReason::Tasks)
        );
    }

    #[test]
    fn tasks_disabled_when_total_zero() {
        assert_eq!(
            compute_win_from_counts(&board(0, 0), 3, 1, &stats(1, 4)),
            None
        );
    }

    #[test]
    fn all_imps_gone_crew_win() {
        assert_eq!(
            compute_win_from_counts(&board(0, 4), 2, 0, &stats(1, 4)),
            Some(WinReason::ImpostorsEliminated)
        );
    }

    #[test]
    fn no_imps_seeded_no_death_win() {
        // Solo / debug: never auto-crew-win just because imps==0.
        assert_eq!(
            compute_win_from_counts(&board(0, 4), 1, 0, &stats(0, 1)),
            None
        );
    }

    #[test]
    fn parity_impostor_win() {
        assert_eq!(
            compute_win_from_counts(&board(0, 4), 1, 1, &stats(1, 4)),
            Some(WinReason::ImpostorMajority)
        );
        assert_eq!(
            compute_win_from_counts(&board(0, 4), 2, 2, &stats(2, 6)),
            Some(WinReason::ImpostorMajority)
        );
    }

    #[test]
    fn majority_impostor_win() {
        assert_eq!(
            compute_win_from_counts(&board(0, 4), 1, 2, &stats(2, 5)),
            Some(WinReason::ImpostorMajority)
        );
    }

    #[test]
    fn empty_world_no_winner() {
        assert_eq!(
            compute_win_from_counts(&board(0, 4), 0, 0, &stats(1, 4)),
            None
        );
    }

    #[test]
    fn tasks_beat_same_frame_majority() {
        // Filled task bar wins even when impostors hit parity that same frame.
        assert_eq!(
            compute_win_from_counts(&board(4, 4), 1, 1, &stats(1, 4)),
            Some(WinReason::Tasks)
        );
    }

    #[test]
    fn task_win_when_no_death_majority() {
        // Tasks finishing while imps still alive, no parity ⇒ crew.
        assert_eq!(
            compute_win_from_counts(&board(4, 4), 2, 1, &stats(1, 4)),
            Some(WinReason::Tasks)
        );
    }
}
