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

use bevy::prelude::*;

use crate::app::{AppState, Paused};
use game_utils_bevy::transitions::Transition;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RuntimeMode>()
            .init_resource::<GamePhase>()
            .init_resource::<MatchConfig>()
            .init_resource::<LobbyState>()
            .init_resource::<TaskBoard>()
            .init_resource::<KillCooldown>()
            .init_resource::<LocalRole>()
            .init_resource::<LocalPlayerId>()
            .init_resource::<MeetingState>()
            .add_message::<StartMatchRequest>()
            .add_message::<MeetingCommand>()
            .add_message::<KillRequest>()
            .add_message::<ReportBody>()
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
                    cleanup_bodies_on_meeting,
                    tick_phase_timers,
                    check_win_conditions,
                )
                    .chain()
                    .run_if(in_state(AppState::InGame))
                    .run_if(|paused: Res<Paused>| !paused.0)
                    .run_if(|transition: Res<Transition<AppState>>| !transition.block_input)
                    .run_if(has_authority),
            );
    }
}

#[derive(Component)]
pub struct MatchCleanup;

fn setup_match(
    mut commands: Commands,
    mut phase: ResMut<GamePhase>,
    cfg: Res<MatchConfig>,
    mut tasks: ResMut<TaskBoard>,
    mut kill_cd: ResMut<KillCooldown>,
    mut meeting: ResMut<MeetingState>,
) {
    *phase = GamePhase::Playing;
    tasks.completed = 0;
    tasks.total = cfg.tasks_to_win;
    kill_cd.remaining = 0.0;
    *meeting = MeetingState::default();
    // Map + players spawned by MapPlugin / PlayerPlugin OnEnter
    let _ = &mut commands;
}

fn cleanup_match(mut commands: Commands, q: Query<Entity, With<MatchCleanup>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
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
    players: Query<&Role, (With<Player>, With<Alive>)>,
    tasks: Res<TaskBoard>,
    mut save: ResMut<crate::save::SaveData>,
    manager: Res<game_utils_bevy::save::SaveManager>,
) {
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
            if meeting.timer.just_finished() || meeting.all_voted() {
                meeting.resolve_votes(&mut phase, cfg.results_time);
            }
        }
        GamePhase::Results => {
            meeting.timer.tick(time.delta());
            if meeting.timer.just_finished() {
                // An eject during Results may already have decided the match.
                if let Some(crew_win) = meeting.pending_game_over.take() {
                    apply_game_over(&mut phase, crew_win, &mut save, &manager);
                    meeting.clear_for_play();
                    return;
                }
                // Final safety check BEFORE returning to play.
                if let Some(crew_win) = compute_win(&tasks, &players) {
                    apply_game_over(&mut phase, crew_win, &mut save, &manager);
                    meeting.clear_for_play();
                    return;
                }
                *phase = GamePhase::Playing;
                meeting.clear_for_play();
            }
        }
        _ => {}
    }
}

fn check_win_conditions(
    mut phase: ResMut<GamePhase>,
    tasks: Res<TaskBoard>,
    players: Query<&Role, (With<Player>, With<Alive>)>,
    mut save: ResMut<crate::save::SaveData>,
    manager: Res<game_utils_bevy::save::SaveManager>,
) {
    if !matches!(*phase, GamePhase::Playing) {
        return;
    }
    if let Some(crew_win) = compute_win(&tasks, &players) {
        apply_game_over(&mut phase, crew_win, &mut save, &manager);
    }
}

pub fn compute_win(
    tasks: &TaskBoard,
    players: &Query<&Role, (With<Player>, With<Alive>)>,
) -> Option<bool> {
    let mut crew = 0u32;
    let mut imps = 0u32;
    for role in players.iter() {
        match role {
            Role::Crewmate => crew += 1,
            Role::Impostor => imps += 1,
        }
    }

    if tasks.total > 0 && tasks.completed >= tasks.total {
        return Some(true);
    }
    if imps == 0 && (crew + imps) > 0 {
        return Some(true);
    }
    // Only impostor-majority when someone is still alive.
    if (crew + imps) > 0 && imps >= crew {
        return Some(false);
    }
    None
}

pub fn apply_game_over(
    phase: &mut GamePhase,
    crew_win: bool,
    save: &mut crate::save::SaveData,
    manager: &game_utils_bevy::save::SaveManager,
) {
    if matches!(*phase, GamePhase::GameOver { .. }) {
        return;
    }
    *phase = GamePhase::GameOver { crew_win };
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
