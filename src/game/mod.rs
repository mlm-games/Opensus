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

use bevy::prelude::*;

use crate::app::{AppState, Paused};
use game_utils_bevy::transitions::Transition;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GamePhase>()
            .init_resource::<MatchConfig>()
            .init_resource::<LobbyState>()
            .init_resource::<TaskBoard>()
            .init_resource::<KillCooldown>()
            .init_resource::<LocalRole>()
            .init_resource::<MeetingState>()
            .add_message::<StartMatchRequest>()
            .add_message::<MeetingCommand>()
            .add_message::<KillRequest>()
            .add_message::<ReportBody>()
            .add_message::<TaskInteract>()
            .add_plugins((
                LobbyPlugin,
                PlayerPlugin,
                MapPlugin,
                TasksPlugin,
                KillSabotagePlugin,
                MeetingVotePlugin,
                SabotagePlugin,
                NetworkingPlugin,
            ))
            .add_systems(OnEnter(AppState::InGame), setup_match)
            .add_systems(OnExit(AppState::InGame), cleanup_match)
            .add_systems(
                Update,
                (tick_phase_timers, check_win_conditions)
                    .run_if(in_state(AppState::InGame))
                    .run_if(|p: Res<Paused>| !p.0)
                    .run_if(|t: Res<Transition<AppState>>| !t.block_input),
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

fn tick_phase_timers(
    time: Res<Time>,
    mut phase: ResMut<GamePhase>,
    mut meeting: ResMut<MeetingState>,
    cfg: Res<MatchConfig>,
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
                meeting.resolve_votes(&mut phase);
            }
        }
        GamePhase::Results => {
            meeting.timer.tick(time.delta());
            if meeting.timer.just_finished() {
                if matches!(*phase, GamePhase::GameOver { .. }) {
                    return;
                }
                if !matches!(*phase, GamePhase::GameOver { .. }) {
                    *phase = GamePhase::Playing;
                    meeting.clear_for_play();
                }
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
    if matches!(*phase, GamePhase::GameOver { .. } | GamePhase::None) {
        return;
    }
    if matches!(
        *phase,
        GamePhase::Meeting | GamePhase::Voting | GamePhase::Results
    ) {
        return;
    }

    let mut crew = 0u32;
    let mut imps = 0u32;
    for role in &players {
        match role {
            Role::Crewmate => crew += 1,
            Role::Impostor => imps += 1,
        }
    }

    if tasks.completed >= tasks.total && tasks.total > 0 {
        *phase = GamePhase::GameOver { crew_win: true };
        save.games_played += 1;
        save.crew_wins += 1;
        let _ = manager.save(&*save);
        return;
    }
    if imps == 0 {
        *phase = GamePhase::GameOver { crew_win: true };
        save.games_played += 1;
        save.crew_wins += 1;
        let _ = manager.save(&*save);
        return;
    }
    if imps >= crew {
        *phase = GamePhase::GameOver { crew_win: false };
        save.games_played += 1;
        save.impostor_wins += 1;
        let _ = manager.save(&*save);
    }
}
