pub mod audio;
pub mod chat;
pub mod collision;
pub mod interaction;
pub mod kill;
pub mod layout;
pub mod map;
pub mod meeting;
pub mod navigation;
pub mod networking;
pub mod player;
pub mod roles;
pub mod sabotage;
pub mod schedule;
pub mod tasks;
pub mod timer;
pub mod vents;

pub use audio::*;
pub use chat::*;
pub use collision::*;
pub use interaction::*;
pub use kill::*;
pub use layout::*;
pub use map::*;
pub use meeting::*;
pub use navigation::*;
pub use player::*;
pub use roles::*;
pub use sabotage::*;
pub use schedule::*;
pub use tasks::*;
pub use timer::*;
pub use vents::*;

use bevy_ecs::prelude::*;
use glam::Vec2;
use rand::RngExt;
use rand::SeedableRng;
use rand::rngs::StdRng;
use repose_core::Color;
use serde::{Deserialize, Serialize};

use crate::save::SaveData;

pub const PLAYER_COLORS: [Color; 12] = [
    Color(230, 51, 51, 255),
    Color(51, 115, 242, 255),
    Color(51, 191, 89, 255),
    Color(242, 217, 51, 255),
    Color(217, 89, 217, 255),
    Color(242, 140, 38, 255),
    Color(77, 230, 230, 255),
    Color(140, 64, 191, 255),
    Color(115, 77, 51, 255),
    Color(242, 242, 242, 255),
    Color(38, 38, 46, 255),
    Color(128, 204, 77, 255),
];

#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeMode {
    #[default]
    Local,
    Host,
    Client,
}

impl RuntimeMode {
    pub const fn has_authority(self) -> bool {
        matches!(self, Self::Local | Self::Host)
    }

    #[allow(dead_code, reason = "Reserved for the network protocol")]
    pub const fn is_remote_client(self) -> bool {
        matches!(self, Self::Client)
    }
}

#[derive(Resource, Default, Clone, Debug)]
pub enum PendingNetworkStart {
    #[default]
    None,
    HostLocal {
        bind_addr: String,
    },
    JoinLocal {
        server_addr: String,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum WinReason {
    Tasks,
    ImpostorsEliminated,
    ImpostorMajority,
    Sabotage,
}

impl WinReason {
    #[inline]
    pub const fn crew_win(self) -> bool {
        matches!(self, Self::Tasks | Self::ImpostorsEliminated)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Tasks => "All tasks completed",
            Self::ImpostorsEliminated => "All impostors eliminated",
            Self::ImpostorMajority => "Impostors reached majority",
            Self::Sabotage => "Critical sabotage succeeded",
        }
    }
}

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum GamePhase {
    #[default]
    None,
    RoleReveal,
    Playing,
    Meeting,
    Voting,
    Results,
    GameOver {
        crew_win: bool,
        reason: WinReason,
    },
}

/// World-space position of a map/player entity (bevy_ecs has no Transform).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Position(pub Vec2);

/// Marker for everything spawned for a single match; despawned on exit.
#[derive(Component)]
pub struct MatchCleanup;

/// Snapshot of how the match was seeded — used so win rules don't fire
/// spuriously (e.g. 0 impostors assigned).
#[derive(Resource, Default, Clone, Debug)]
pub struct MatchStats {
    pub impostors_spawned: u32,
    pub players_spawned: u32,
}

#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct LocalRole(pub Option<Role>);

#[derive(Resource, Default)]
pub struct RoleRevealTimer(pub SimTimer);

/// Per-app random seed; each match derives a new seed from it so matches
/// differ per run while a test can pin a seed for determinism.
#[derive(Resource, Clone, Copy, Debug)]
pub struct MatchSeed(pub u64);

#[derive(Resource)]
pub struct MatchRng(pub StdRng);

#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Trauma {
    pub value: f32,
}

impl Trauma {
    pub fn add(&mut self, amount: f32) {
        self.value = (self.value + amount).min(1.0);
    }
}

pub fn tick_trauma(time: Res<repame_sim::SimTime>, mut trauma: ResMut<Trauma>) {
    trauma.value = (trauma.value - time.delta_secs * 1.5).max(0.0);
}

/// GameOver result written to `SaveData` in memory; the app layer persists
/// it to disk once after the schedule run.
#[derive(Resource, Default)]
pub struct GameOverSaved(pub bool);

#[derive(Resource, Clone, Debug)]
pub struct MatchConfig {
    pub max_players: u8,
    pub impostor_count: u8,
    pub emergency_meetings: u8,
    pub emergency_cooldown: f32,

    pub kill_cooldown: f32,
    pub initial_kill_cooldown: f32,
    pub kill_range: f32,

    pub discussion_time: f32,
    pub voting_time: f32,
    pub results_time: f32,
    pub confirm_ejects: bool,

    pub role_reveal_time: f32,

    pub tasks_per_crewmate: u32,
    pub tasks_to_win: u32,
    pub task_hold_time: f32,

    pub interact_range: f32,
    pub report_range: f32,

    pub sabotage_cooldown: f32,
    pub oxygen_time: f32,
    pub reactor_time: f32,
    pub sabotage_fix_time: f32,
    pub reactor_sync_window: f32,

    pub player_speed: f32,
    pub ghost_speed_mul: f32,
    pub camera_follow_sharpness: f32,

    pub bot_count: u8,
    pub bot_task_weight: f32,
    pub bot_report_range: f32,
    pub bot_kill_aggression: f32,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            max_players: 10,
            impostor_count: 1,
            emergency_meetings: 1,
            emergency_cooldown: 15.0,

            kill_cooldown: 22.5,
            initial_kill_cooldown: 10.0,
            kill_range: 42.0,

            discussion_time: 18.0,
            voting_time: 22.0,
            results_time: 4.0,
            confirm_ejects: false,

            role_reveal_time: 3.0,

            tasks_per_crewmate: 3,
            tasks_to_win: 3,
            task_hold_time: 1.65,

            interact_range: 42.0,
            report_range: 52.0,
            sabotage_cooldown: 20.0,
            oxygen_time: 30.0,
            reactor_time: 45.0,
            sabotage_fix_time: 2.5,
            reactor_sync_window: 0.75,

            player_speed: 220.0,
            ghost_speed_mul: 1.18,
            camera_follow_sharpness: 9.0,

            bot_count: 3,
            bot_task_weight: 0.65,
            bot_report_range: 70.0,
            bot_kill_aggression: 0.55,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LobbySlot {
    pub id: u64,
    pub name: String,
    pub color_index: u8,
    pub ready: bool,
    pub is_local: bool,
    pub is_host: bool,
    pub is_bot: bool,
}

#[derive(Resource, Default)]
pub struct LobbyState {
    pub slots: Vec<LobbySlot>,
    pub local_ready: bool,
    pub is_host: bool,
}

pub fn setup_lobby(world: &mut World) {
    let save = world.resource::<SaveData>().clone();
    let mode = *world.resource::<RuntimeMode>();
    let bot_count = world.resource::<MatchConfig>().bot_count;
    let max_players = world.resource::<MatchConfig>().max_players as usize;
    let mut lobby = world.resource_mut::<LobbyState>();

    lobby.slots.clear();
    lobby.local_ready = false;

    let push_local_host = |lobby: &mut LobbyState| {
        lobby.is_host = true;
        lobby.slots.push(LobbySlot {
            id: 1,
            name: save.player_name.clone(),
            color_index: save.preferred_color_index,
            ready: false,
            is_local: true,
            is_host: true,
            is_bot: false,
        });
    };

    let fill_bots = |lobby: &mut LobbyState| {
        for i in 0..bot_count as u64 {
            if lobby.slots.len() >= max_players {
                break;
            }
            lobby.slots.push(LobbySlot {
                id: 10 + i,
                name: format!("Agent-{}", i + 2),
                color_index: ((save.preferred_color_index as u64 + 1 + i) % 12) as u8,
                ready: true,
                is_local: false,
                is_host: false,
                is_bot: true,
            });
        }
    };

    match mode {
        RuntimeMode::Local => {
            push_local_host(&mut lobby);
            fill_bots(&mut lobby);
        }
        RuntimeMode::Host => {
            push_local_host(&mut lobby);
        }
        RuntimeMode::Client => {
            lobby.is_host = false;
        }
    }
}

pub fn handle_start_match(world: &World) -> bool {
    let lobby = world.resource::<LobbyState>();
    let mode = *world.resource::<RuntimeMode>();
    let cfg = world.resource::<MatchConfig>();
    if !lobby.is_host || !lobby.local_ready {
        return false;
    }
    let minimum_players = match cfg.impostor_count {
        0 => 1,
        1 => 4,
        2 => 7,
        _ => 9,
    };
    if lobby.slots.len() < minimum_players {
        return false;
    }
    let everyone_ready = lobby.slots.iter().all(|s| s.is_local || s.ready);
    mode == RuntimeMode::Local || everyone_ready
}

#[derive(Clone, Debug)]
pub enum MeetingCommand {
    Emergency { actor_id: u64 },
    Vote { voter_id: u64, target: u64 },
    Skip { voter_id: u64 },
}

#[derive(Resource, Default)]
pub struct MeetingCommands(pub Vec<MeetingCommand>);

fn setup_match(world: &mut World) {
    let seed = world.resource::<MatchSeed>().0;
    let mut rng = StdRng::seed_from_u64(seed);
    world.resource_mut::<MatchSeed>().0 = rng.random::<u64>();
    world.insert_resource(MatchRng(rng));

    // Role reveal beat first; intents are frozen until it expires.
    let cfg = world.resource::<MatchConfig>().clone();
    *world.resource_mut::<GamePhase>() = GamePhase::RoleReveal;
    world.insert_resource(RoleRevealTimer(SimTimer::from_seconds(
        cfg.role_reveal_time,
        TimerMode::Once,
    )));
    world.resource_mut::<TaskBoard>().completed = 0;
    // `total` is owned by spawn_players_from_lobby.
    *world.resource_mut::<MeetingState>() = MeetingState::default();
    world.resource_mut::<ActiveSabotage>().clear();
    world.insert_resource(SabotageCooldown::default());
    world.insert_resource(SabotageRequests::default());
    world.insert_resource(Trauma::default());
    world.insert_resource(KillRequests::default());
    world.insert_resource(ReportBodies::default());
    world.insert_resource(GameOverSaved(false));
}

pub fn enter_ingame(world: &mut World) {
    networking::reset_match_state(world);
    reset_chat(world);
    reset_audio_locals(world);
    setup_match(world);
    spawn_map(world);
    spawn_task_stations(world);
    spawn_fix_stations(world);
    spawn_players_from_lobby(world);
}

pub fn exit_ingame(world: &mut World) {
    networking::reset_match_state(world);
    let entities: Vec<Entity> = {
        let mut query = world.query_filtered::<Entity, With<MatchCleanup>>();
        query.iter(world).collect()
    };
    for entity in entities {
        let _ = world.despawn(entity);
    }
    // Leave GameOver visible until UI navigates away; only clear stats.
    // Phase is cleared by PlayAgain / QuitToTitle.
    *world.resource_mut::<MatchStats>() = MatchStats::default();
    world.resource_mut::<MeetingState>().clear_for_play();
    world.resource_mut::<ActiveSabotage>().clear();
    if let Some(mut cooldown) = world.get_resource_mut::<SabotageCooldown>() {
        cooldown.remaining = 0.0;
    }
    reset_chat(world);
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

#[allow(clippy::too_many_arguments)]
fn tick_phase_timers(
    time: Res<repame_sim::SimTime>,
    mut phase: ResMut<GamePhase>,
    mut meeting: ResMut<MeetingState>,
    mut role_reveal: ResMut<RoleRevealTimer>,
    cfg: Res<MatchConfig>,
    tasks: Res<TaskBoard>,
    players: Query<&Role, (With<Player>, With<Alive>)>,
    stats: Res<MatchStats>,
    mut save: ResMut<SaveData>,
    mut kill_cooldowns: Query<&mut KillCooldownLeft>,
) {
    // Never advance meeting timers after the match is decided.
    if matches!(*phase, GamePhase::GameOver { .. } | GamePhase::None) {
        return;
    }

    match *phase {
        GamePhase::RoleReveal => {
            role_reveal.0.tick(time.delta_secs);
            if role_reveal.0.just_finished() {
                *phase = GamePhase::Playing;
            }
        }
        GamePhase::Meeting => {
            meeting.timer.tick(time.delta_secs);
            if meeting.timer.just_finished() {
                *phase = GamePhase::Voting;
                meeting.timer = SimTimer::from_seconds(cfg.voting_time, TimerMode::Once);
                meeting.prompt = "Vote".into();
            }
        }
        GamePhase::Voting => {
            meeting.timer.tick(time.delta_secs);
            // bots already filled this frame via ensure_bot_votes (runs before us)
            if meeting.timer.just_finished() || meeting.all_voted() {
                meeting.resolve_votes(&mut phase, cfg.results_time);
            }
        }
        GamePhase::Results => {
            meeting.timer.tick(time.delta_secs);
            if meeting.timer.just_finished() {
                // Eject already applied on Results entry (prior frames + ApplyDeferred).
                // Decide here so we never flash Playing on a decided match.
                if let Some(reason) = compute_win(&tasks, &players, &stats) {
                    apply_game_over(&mut phase, reason, &mut save);
                } else {
                    for mut cd in &mut kill_cooldowns {
                        cd.0 = cfg.kill_cooldown;
                    }
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
    mut q: Query<(Entity, &Player, &Role), With<Alive>>,
    cfg: Res<MatchConfig>,
    mut trauma: ResMut<Trauma>,
) {
    // Only consume eject once we've entered Results.
    if !matches!(*phase, GamePhase::Results) {
        return;
    }
    let Some(eid) = meeting.pending_eject.take() else {
        return;
    };

    for (e, p, role) in &mut q {
        if p.id != eid {
            continue;
        }
        make_ghost(&mut commands, e);
        trauma.add(0.5);
        meeting.result_text = if cfg.confirm_ejects {
            if matches!(role, Role::Impostor) {
                format!("{} was an Impostor.", p.name)
            } else {
                format!("{} was not an Impostor.", p.name)
            }
        } else {
            format!("{} was ejected.", p.name)
        };
        break;
    }
}

fn ensure_bot_votes(
    phase: Res<GamePhase>,
    mut meeting: ResMut<MeetingState>,
    bots: Query<&Player, (With<AiPlayer>, With<Alive>)>,
    mut rng: ResMut<MatchRng>,
) {
    if !matches!(*phase, GamePhase::Voting) {
        return;
    }
    cast_missing_bot_votes(&mut meeting, &bots, &mut rng.0);
}

fn tick_emergency_cooldowns(
    time: Res<repame_sim::SimTime>,
    mut players: Query<&mut EmergencyCooldownLeft, With<Alive>>,
) {
    for mut cooldown in &mut players {
        cooldown.0 = (cooldown.0 - time.delta_secs).max(0.0);
    }
}

fn reset_cooldowns_after_meeting(
    phase: Res<GamePhase>,
    cfg: Res<MatchConfig>,
    mut previous: Local<Option<GamePhase>>,
    mut players: Query<(Option<&mut KillCooldownLeft>, &mut EmergencyCooldownLeft), With<Alive>>,
) {
    let returned_to_play =
        matches!(*phase, GamePhase::Playing) && matches!(*previous, Some(GamePhase::Results));

    if returned_to_play {
        for (kill, mut emergency) in &mut players {
            if let Some(mut kill) = kill {
                kill.0 = cfg.kill_cooldown;
            }
            emergency.0 = cfg.emergency_cooldown;
        }
    }

    *previous = Some(*phase);
}

/// Runs once when entering GameOver: stop sabotage HUD/timers so nothing
/// "expires" under the victory screen and rematches start clean.
fn cleanup_on_game_over_enter(
    phase: Res<GamePhase>,
    mut previous: Local<Option<GamePhase>>,
    mut sabotage: ResMut<ActiveSabotage>,
    mut stations: Query<&mut SabotageFixStation>,
) {
    let entered = matches!(*phase, GamePhase::GameOver { .. })
        && !matches!(*previous, Some(GamePhase::GameOver { .. }));
    *previous = Some(*phase);
    if !entered {
        return;
    }
    sabotage.clear();
    for mut station in &mut stations {
        station.progress = 0.0;
    }
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
    mut save: ResMut<SaveData>,
) {
    // Open play only. Results decides at timer end inside tick_phase_timers
    // so the eject/role-reveal UI always gets a full beat.
    if !matches!(*phase, GamePhase::Playing) {
        return;
    }
    if let Some(reason) = compute_win(&tasks, &players, &stats) {
        apply_game_over(&mut phase, reason, &mut save);
    }
}

pub fn apply_game_over(phase: &mut GamePhase, reason: WinReason, save: &mut SaveData) {
    if matches!(*phase, GamePhase::GameOver { .. }) {
        return;
    }
    let crew_win = reason.crew_win();
    *phase = GamePhase::GameOver { crew_win, reason };
    log::info!("GameOver: crew_win={crew_win} reason={reason:?}");
    save.games_played = save.games_played.saturating_add(1);
    if crew_win {
        save.crew_wins = save.crew_wins.saturating_add(1);
    } else {
        save.impostor_wins = save.impostor_wins.saturating_add(1);
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
