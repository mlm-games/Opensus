use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use game_utils_repame::{I18nStrings, SaveResource, register_i18n, register_save};
use rand::SeedableRng;
use rand::rngs::StdRng;
use repame_fx::TransitionFx;
use repame_shell::{
    SharedEdges, Staging, install_game_shortcuts, shared_edges, take_shortcut_edges,
};
use repame_sim::Sim;
use repose_core::input::{Key, KeyEvent, KeyEventType, PhysicalKey};
use repose_core::prelude::Modifier;
use repose_core::{FocusRequester, RenderContext, Scheduler, View, remember, request_frame};
use repose_ui::overlay::OverlayHandle;
use repose_ui::{ViewExt, ZStack};
use web_time::{Duration, Instant};

use crate::assets::GameImages;
use crate::audio::GameAudio;
use crate::game::{
    ActiveSabotage, AudioFrameMemory, CriticalAlarmTimer, GameOverSaved, GamePhase, LobbyState,
    LocalControls, MatchConfig, MatchRng, MatchSeed, MeetingCommand, MeetingCommands,
    PLAYER_COLORS, PendingCues, PendingNetworkStart, RuntimeMode, StateRequest, TaskBoard,
    build_game_schedule, enter_ingame, exit_ingame, handle_start_match, input_direction,
    networking, setup_lobby,
};
use crate::render::{RenderState, sync_world_render};
use crate::save::{SAVE_VERSION, SaveData};
use crate::ui::{SharedUi, UiAction, UiActions, compose_root, drain_actions, sync_shared_ui};

const TRANSLATION_KEYS: &[&str] = &[
    "app-title",
    "app-tagline",
    "start-game",
    "host-game",
    "join-game",
    "ready",
    "unready",
    "start-match",
    "leave-lobby",
    "settings",
    "credits",
    "quit",
    "paused",
    "pause",
    "resume",
    "quit-to-title",
    "save",
    "back",
    "master-volume",
    "sfx-volume",
    "music-volume",
    "language",
    "loading",
    "loading-subtitle",
    "crewmate",
    "impostor",
    "alive",
    "dead",
    "ghost",
    "tasks-remaining",
    "kill",
    "report",
    "sabotage",
    "emergency-meeting",
    "discussion",
    "voting",
    "vote",
    "skip",
    "ejected",
    "skip-result",
    "crewmates-win",
    "impostors-win",
    "play-again",
    "controls-hint",
    "you-are",
    "kill-cooldown",
    "lobby-waiting",
    "players",
    "color",
    "name",
];

const LOCALES: &[(&str, &str)] = &[
    ("en", include_str!("../../assets/locales/en/main.ftl")),
    ("es", include_str!("../../assets/locales/es/main.ftl")),
    ("fr", include_str!("../../assets/locales/fr/main.ftl")),
    ("de", include_str!("../../assets/locales/de/main.ftl")),
    ("ja", include_str!("../../assets/locales/ja/main.ftl")),
    ("zh", include_str!("../../assets/locales/zh/main.ftl")),
    ("pt", include_str!("../../assets/locales/pt/main.ftl")),
];

const SPLASH_SECS: f32 = 1.5;
pub(crate) const LOADING_SECS: f32 = 0.35;
const UNPAUSE_DELAY_SECS: f32 = 0.2;

const SHORTCUT_PAUSE: &str = "opensus.pause";
const SHORTCUT_RESTART: &str = "opensus.restart";
const SHORTCUT_CONFIRM: &str = "opensus.confirm";

const FREDOKA_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Fredoka-Regular.ttf");
const FREDOKA_BOLD: &[u8] = include_bytes!("../../assets/fonts/Fredoka-Bold.ttf");

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Splash,
    Loading,
    Title,
    Lobby,
    InGame,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlayMenu {
    #[default]
    None,
    Settings,
    Credits,
    Pause,
}

#[derive(Resource, Default)]
pub struct QuitRequested(pub bool);

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub struct Paused(pub bool);

#[derive(Resource, Default)]
pub struct PendingUnpause(pub Option<f32>);

#[derive(Resource, Default)]
pub struct InputEdges {
    pub pause: bool,
    pub kill: bool,
    pub report: bool,
    pub emergency: bool,
    pub sabotage_lights: bool,
    pub sabotage_oxygen: bool,
    pub sabotage_reactor: bool,
}

#[derive(Resource)]
pub struct SplashTimer(pub f32);

#[derive(Resource)]
pub struct LoadingTimer(pub f32);

fn ensure_font() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        repose_text::register_font_data(FREDOKA_REGULAR);
        repose_text::register_font_data(FREDOKA_BOLD);
    });
}

pub fn goto_state(world: &mut World, next: AppState) {
    let prev = world
        .get_resource::<AppState>()
        .copied()
        .unwrap_or_default();
    if prev == next {
        return;
    }
    if prev == AppState::InGame {
        exit_ingame(world);
    }
    world.insert_resource(next);
    world.insert_resource(Paused::default());
    world.insert_resource(OverlayMenu::None);
    world.insert_resource(PendingUnpause::default());
    match next {
        AppState::Splash => {
            world.insert_resource(SplashTimer(SPLASH_SECS));
        }
        AppState::Loading => {
            world.insert_resource(LoadingTimer(LOADING_SECS));
        }
        AppState::Lobby => {
            setup_lobby(world);
        }
        AppState::InGame => {
            enter_ingame(world);
        }
        AppState::Title => {
            networking::on_enter_title(world);
        }
    }
}

fn tick_splash(world: &mut World, dt_secs: f32) -> Option<AppState> {
    if *world.get_resource::<AppState>()? != AppState::Splash {
        return None;
    }
    let fired = {
        let mut timer = world.get_resource_mut::<SplashTimer>()?;
        timer.0 -= dt_secs;
        timer.0 <= 0.0
    };
    if !fired {
        return None;
    }
    world.remove_resource::<SplashTimer>();
    Some(AppState::Title)
}

fn tick_loading(world: &mut World, dt_secs: f32) -> Option<AppState> {
    if *world.get_resource::<AppState>()? != AppState::Loading {
        return None;
    }
    let fired = {
        let mut timer = world.get_resource_mut::<LoadingTimer>()?;
        timer.0 -= dt_secs;
        timer.0 <= 0.0
    };
    if !fired {
        return None;
    }
    world.remove_resource::<LoadingTimer>();
    Some(AppState::Lobby)
}

fn tick_pending_unpause(world: &mut World, dt_secs: f32) {
    let finished = match world.get_resource_mut::<PendingUnpause>() {
        Some(mut pending) => match pending.0.as_mut() {
            Some(secs) => {
                *secs -= dt_secs;
                *secs <= 0.0
            }
            None => false,
        },
        None => false,
    };
    if !finished {
        return;
    }
    if let Some(mut pending) = world.get_resource_mut::<PendingUnpause>() {
        pending.0 = None;
    }
    world.insert_resource(Paused(false));
}

fn handle_pause_input(world: &mut World) {
    let pause = match world.get_resource_mut::<InputEdges>() {
        Some(mut edges) => std::mem::take(&mut edges.pause),
        None => return,
    };
    if !pause || *world.resource::<AppState>() != AppState::InGame {
        return;
    }
    // GameOver UI owns the screen; Esc shouldn't open the pause menu.
    if matches!(*world.resource::<GamePhase>(), GamePhase::GameOver { .. }) {
        return;
    }
    if world.resource::<TransitionFx>().blocking() {
        return;
    }
    let overlay = *world.resource::<OverlayMenu>();
    let paused = world.resource::<Paused>().0;
    match overlay {
        OverlayMenu::None if !paused => {
            {
                let mut edges = world.resource_mut::<InputEdges>();
                edges.kill = false;
                edges.report = false;
                edges.emergency = false;
                edges.sabotage_lights = false;
                edges.sabotage_oxygen = false;
                edges.sabotage_reactor = false;
            }
            world.insert_resource(Paused(true));
            world.insert_resource(OverlayMenu::Pause);
            world.insert_resource(PendingUnpause(None));
        }
        OverlayMenu::Pause => {
            world.insert_resource(OverlayMenu::None);
            world.insert_resource(PendingUnpause(Some(UNPAUSE_DELAY_SECS)));
        }
        OverlayMenu::Settings | OverlayMenu::Credits => {
            if paused {
                world.insert_resource(OverlayMenu::Pause);
            } else {
                world.insert_resource(OverlayMenu::None);
            }
        }
        _ => {}
    }
}

pub struct App {
    pub sim: Sim,
    schedule: Schedule,
    pending_state: Option<AppState>,
    staging: Rc<RefCell<Staging>>,
    chat_keys: Rc<RefCell<Vec<KeyEvent>>>,
    shortcut_edges: SharedEdges,
    ui: SharedUi,
    render: RenderState,
    images: GameImages,
    audio: GameAudio,
}

impl App {
    pub fn new() -> Self {
        ensure_font();
        let mut sim = Sim::with_default_step();
        repame_fx::init_resources(&mut sim.world);
        game_utils_repame::init_time_resources(&mut sim.world);
        register_i18n(&mut sim.world, TRANSLATION_KEYS, LOCALES);
        register_save::<SaveData, _>(
            &mut sim.world,
            SaveResource::new("com", "mlm-games", "opensus", "save.ron", SAVE_VERSION),
        );
        sim.world.init_resource::<UiActions>();
        sim.world.init_resource::<RuntimeMode>();
        sim.world.init_resource::<PendingNetworkStart>();
        sim.world.init_resource::<StateRequest>();
        networking::init_resources(&mut sim.world);
        sim.world.init_resource::<MatchConfig>();
        sim.world.init_resource::<LobbyState>();
        sim.world.init_resource::<GamePhase>();
        sim.world.init_resource::<MeetingCommands>();
        sim.world.init_resource::<TaskBoard>();
        sim.world.init_resource::<crate::game::LocalRole>();
        sim.world.init_resource::<crate::game::LocalPlayerId>();
        sim.world.init_resource::<crate::game::MeetingState>();
        sim.world.init_resource::<crate::game::MatchStats>();
        sim.world.init_resource::<crate::game::RoleRevealTimer>();
        sim.world.init_resource::<ActiveSabotage>();
        sim.world.init_resource::<crate::game::SabotageCooldown>();
        sim.world.init_resource::<crate::game::SabotageRequests>();
        sim.world.init_resource::<crate::game::Trauma>();
        sim.world.init_resource::<crate::game::KillRequests>();
        sim.world.init_resource::<crate::game::ReportBodies>();
        sim.world.init_resource::<GameOverSaved>();
        sim.world.init_resource::<crate::game::LocalPrompt>();
        sim.world.init_resource::<crate::game::ChatState>();
        sim.world.init_resource::<crate::game::ChatInputBuffer>();
        sim.world.init_resource::<crate::game::OutgoingChat>();
        sim.world.init_resource::<crate::game::ChatKeys>();
        sim.world.init_resource::<LocalControls>();
        sim.world.init_resource::<PendingCues>();
        sim.world.init_resource::<AudioFrameMemory>();
        sim.world.init_resource::<CriticalAlarmTimer>();
        let seed = rand::random::<u64>();
        sim.world.insert_resource(MatchSeed(seed));
        sim.world
            .insert_resource(MatchRng(StdRng::seed_from_u64(seed)));
        sim.world.insert_resource(QuitRequested::default());
        sim.world.insert_resource(AppState::Splash);
        sim.world.insert_resource(SplashTimer(SPLASH_SECS));
        sim.world.insert_resource(Paused::default());
        sim.world.insert_resource(OverlayMenu::None);
        sim.world.insert_resource(PendingUnpause::default());
        sim.world.insert_resource(InputEdges::default());
        Self {
            sim,
            schedule: build_game_schedule(),
            pending_state: None,
            staging: Staging::shared(),
            chat_keys: Rc::new(RefCell::new(Vec::new())),
            shortcut_edges: shared_edges(),
            ui: SharedUi::default(),
            render: RenderState::default(),
            images: GameImages::default(),
            audio: GameAudio::new(),
        }
    }

    pub fn begin_to_state(&mut self, next: AppState) {
        self.pending_state = Some(next);
        self.sim.world.resource_mut::<TransitionFx>().begin();
    }

    pub fn advance(&mut self, dt: Duration) -> u32 {
        let dt_secs = dt.as_secs_f32();
        self.sim
            .world
            .resource_mut::<TransitionFx>()
            .step_secs(dt_secs);
        let requested = {
            let mut request = self.sim.world.resource_mut::<StateRequest>();
            request.0.take()
        };
        if let Some(next) = requested
            && self.pending_state.is_none()
        {
            self.begin_to_state(next);
        }
        if self.sim.world.resource::<TransitionFx>().alpha() >= 1.0
            && let Some(next) = self.pending_state.take()
        {
            goto_state(&mut self.sim.world, next);
        }
        if let Some(next) = tick_splash(&mut self.sim.world, dt_secs)
            .or_else(|| tick_loading(&mut self.sim.world, dt_secs))
        {
            self.begin_to_state(next);
        }
        handle_pause_input(&mut self.sim.world);
        tick_pending_unpause(&mut self.sim.world, dt_secs);

        let state = *self.sim.world.resource::<AppState>();
        if matches!(state, AppState::Splash | AppState::Loading)
            || (self.sim.world.resource::<Paused>().0
                && *self.sim.world.resource::<RuntimeMode>() == RuntimeMode::Local)
        {
            return 0;
        }
        let schedule = &mut self.schedule;
        let ran = self.sim.step_with(dt, move |world| schedule.run(world));
        if matches!(
            *self.sim.world.resource::<GamePhase>(),
            GamePhase::GameOver { .. }
        ) && !self.sim.world.resource::<GameOverSaved>().0
        {
            self.sim.world.insert_resource(GameOverSaved(true));
            let world = &self.sim.world;
            let _ = world
                .resource::<SaveResource>()
                .save_now(world.resource::<SaveData>());
        }
        ran
    }

    fn feed_polled(&mut self, sched: &Scheduler) {
        self.staging.borrow_mut().feed_polled(sched);
    }

    fn feed_input(&mut self) {
        let key_edges = self.staging.borrow_mut().take_edges();
        let (pause, _, _) = take_shortcut_edges(&self.shortcut_edges);
        let blocked = self.sim.world.resource::<TransitionFx>().blocking();
        let state = *self.sim.world.resource::<AppState>();
        let paused = self.sim.world.resource::<Paused>().0;
        let phase = *self.sim.world.resource::<GamePhase>();
        let playing =
            state == AppState::InGame && !paused && !blocked && matches!(phase, GamePhase::Playing);
        let (direction, interact) = {
            let staging = self.staging.borrow();
            let held = |key| staging.held.contains(&key);
            (
                input_direction(
                    held(PhysicalKey::KeyW) || held(PhysicalKey::ArrowUp),
                    held(PhysicalKey::KeyS) || held(PhysicalKey::ArrowDown),
                    held(PhysicalKey::KeyA) || held(PhysicalKey::ArrowLeft),
                    held(PhysicalKey::KeyD) || held(PhysicalKey::ArrowRight),
                ),
                held(PhysicalKey::KeyE),
            )
        };
        self.sim.world.insert_resource(LocalControls {
            direction,
            interact,
        });
        let chat_events = std::mem::take(&mut *self.chat_keys.borrow_mut());
        if state == AppState::InGame
            && matches!(phase, GamePhase::Meeting | GamePhase::Voting)
            && !chat_events.is_empty()
        {
            self.sim
                .world
                .resource_mut::<crate::game::ChatKeys>()
                .0
                .extend(chat_events);
        }
        let mut edges = self.sim.world.resource_mut::<InputEdges>();
        edges.pause = pause && !blocked;
        if playing {
            edges.kill |= key_edges.contains(&PhysicalKey::KeyQ);
            edges.report |= key_edges.contains(&PhysicalKey::KeyR);
            edges.emergency |= key_edges.contains(&PhysicalKey::KeyF);
            edges.sabotage_lights |= key_edges.contains(&PhysicalKey::Digit1);
            edges.sabotage_oxygen |= key_edges.contains(&PhysicalKey::Digit2);
            edges.sabotage_reactor |= key_edges.contains(&PhysicalKey::Digit3);
        }
    }

    fn process_ui_actions(&mut self) {
        let local_id = self.ui.local_player_id.or_else(|| {
            self.sim
                .world
                .resource::<LobbyState>()
                .slots
                .iter()
                .find(|slot| slot.is_local)
                .map(|slot| slot.id)
        });
        for action in drain_actions(&mut self.sim.world) {
            self.apply_ui_action(action, local_id);
        }
    }

    fn apply_ui_action(&mut self, action: UiAction, local_id: Option<u64>) {
        match action {
            UiAction::PlayOffline => {
                self.sim.world.insert_resource(RuntimeMode::Local);
                self.sim.world.insert_resource(PendingNetworkStart::None);
                self.begin_to_state(AppState::Loading);
            }
            UiAction::HostLobby => {
                #[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
                {
                    self.sim.world.insert_resource(RuntimeMode::Host);
                    self.sim
                        .world
                        .insert_resource(PendingNetworkStart::HostLocal {
                            bind_addr: "127.0.0.1:5000".to_string(),
                        });
                }
                #[cfg(not(all(feature = "networking-native", not(target_arch = "wasm32"))))]
                {
                    self.sim.world.insert_resource(RuntimeMode::Local);
                    self.sim.world.insert_resource(PendingNetworkStart::None);
                }
                self.begin_to_state(AppState::Loading);
            }
            UiAction::JoinLobby => {
                self.sim.world.insert_resource(RuntimeMode::Client);
                self.sim
                    .world
                    .insert_resource(PendingNetworkStart::JoinLocal {
                        server_addr: "127.0.0.1:5000".to_string(),
                    });
                self.begin_to_state(AppState::Loading);
            }
            UiAction::ToggleReady => {
                let mut lobby = self.sim.world.resource_mut::<LobbyState>();
                lobby.local_ready = !lobby.local_ready;
                let ready = lobby.local_ready;
                if let Some(slot) = lobby.slots.iter_mut().find(|slot| slot.is_local) {
                    slot.ready = ready;
                }
            }
            UiAction::StartMatch => {
                if handle_start_match(&self.sim.world) {
                    self.begin_to_state(AppState::InGame);
                }
            }
            UiAction::LeaveLobby => {
                self.sim.world.insert_resource(PendingNetworkStart::None);
                self.sim.world.insert_resource(RuntimeMode::Local);
                self.begin_to_state(AppState::Title);
            }
            UiAction::CallEmergency => {
                if let Some(id) = local_id {
                    self.sim
                        .world
                        .resource_mut::<MeetingCommands>()
                        .0
                        .push(MeetingCommand::Emergency { actor_id: id });
                }
            }
            UiAction::CastVote(id) => {
                if let Some(voter_id) = local_id {
                    self.sim
                        .world
                        .resource_mut::<MeetingCommands>()
                        .0
                        .push(MeetingCommand::Vote {
                            voter_id,
                            target: id,
                        });
                }
            }
            UiAction::SkipVote => {
                if let Some(voter_id) = local_id {
                    self.sim
                        .world
                        .resource_mut::<MeetingCommands>()
                        .0
                        .push(MeetingCommand::Skip { voter_id });
                }
            }
            UiAction::PlayAgain => {
                self.sim.world.insert_resource(PendingNetworkStart::None);
                if !matches!(
                    *self.sim.world.resource::<RuntimeMode>(),
                    RuntimeMode::Host | RuntimeMode::Client
                ) {
                    self.sim.world.insert_resource(RuntimeMode::Local);
                    self.begin_to_state(AppState::Lobby);
                } else {
                    self.begin_to_state(AppState::Title);
                }
                self.sim.world.insert_resource(GamePhase::None);
                self.sim.world.insert_resource(Paused(false));
                self.sim.world.insert_resource(OverlayMenu::None);
                self.sim.world.insert_resource(PendingUnpause::default());
            }
            UiAction::CycleColor => {
                let next = {
                    let mut save = self.sim.world.resource_mut::<SaveData>();
                    save.preferred_color_index =
                        (save.preferred_color_index + 1) % PLAYER_COLORS.len() as u8;
                    save.preferred_color_index
                };
                let mut lobby = self.sim.world.resource_mut::<LobbyState>();
                if let Some(slot) = lobby.slots.iter_mut().find(|slot| slot.is_local) {
                    slot.color_index = next;
                }
            }
            UiAction::OpenSettings => {
                self.ui.saved_language = self.sim.world.resource::<I18nStrings>().0.current.clone();
                self.sim.world.insert_resource(OverlayMenu::Settings);
            }
            UiAction::TogglePause => {
                if !self.sim.world.resource::<Paused>().0 {
                    self.sim.world.insert_resource(Paused(true));
                    self.sim.world.insert_resource(OverlayMenu::Pause);
                    self.sim.world.insert_resource(PendingUnpause::default());
                } else {
                    self.sim.world.insert_resource(OverlayMenu::None);
                    self.sim
                        .world
                        .insert_resource(PendingUnpause(Some(UNPAUSE_DELAY_SECS)));
                }
            }
            UiAction::OpenCredits => {
                self.sim.world.insert_resource(OverlayMenu::Credits);
            }
            UiAction::CloseOverlay => {
                let overlay = *self.sim.world.resource::<OverlayMenu>();
                if overlay == OverlayMenu::Settings {
                    let revert = self.ui.saved_language.clone();
                    let locale = &mut self.sim.world.resource_mut::<I18nStrings>().0;
                    let _ = locale.set_locale(&revert);
                }
                match overlay {
                    OverlayMenu::Settings | OverlayMenu::Credits
                        if self.sim.world.resource::<Paused>().0 =>
                    {
                        self.sim.world.insert_resource(OverlayMenu::Pause);
                    }
                    OverlayMenu::Pause if self.sim.world.resource::<Paused>().0 => {
                        self.sim.world.insert_resource(OverlayMenu::None);
                        self.sim
                            .world
                            .insert_resource(PendingUnpause(Some(UNPAUSE_DELAY_SECS)));
                    }
                    _ => {
                        self.sim.world.insert_resource(OverlayMenu::None);
                    }
                }
            }
            UiAction::Resume => {
                self.sim.world.insert_resource(OverlayMenu::None);
                self.sim
                    .world
                    .insert_resource(PendingUnpause(Some(UNPAUSE_DELAY_SECS)));
            }
            UiAction::QuitToTitle => {
                self.sim.world.insert_resource(PendingNetworkStart::None);
                self.sim.world.insert_resource(RuntimeMode::Local);
                self.sim.world.insert_resource(GamePhase::None);
                self.sim.world.insert_resource(Paused(false));
                self.sim.world.insert_resource(OverlayMenu::None);
                self.sim.world.insert_resource(PendingUnpause::default());
                self.begin_to_state(AppState::Title);
            }
            UiAction::QuitApp => {
                self.sim.world.insert_resource(QuitRequested(true));
            }
            UiAction::SetMasterVol(value) => self.ui.master_vol = value.clamp(0.0, 1.0),
            UiAction::SetSfxVol(value) => self.ui.sfx_vol = value.clamp(0.0, 1.0),
            UiAction::SetMusicVol(value) => self.ui.music_vol = value.clamp(0.0, 1.0),
            UiAction::SaveSettings => {
                let language = self.sim.world.resource::<I18nStrings>().0.current.clone();
                {
                    let mut save = self.sim.world.resource_mut::<SaveData>();
                    save.settings.master_volume = self.ui.master_vol;
                    save.settings.sfx_volume = self.ui.sfx_vol;
                    save.settings.music_volume = self.ui.music_vol;
                    save.settings.language = language.clone();
                }
                let world = &self.sim.world;
                let _ = world
                    .resource::<SaveResource>()
                    .save_now(world.resource::<SaveData>());
                self.ui.saved_language = language;
                let paused = self.sim.world.resource::<Paused>().0;
                if paused {
                    self.sim.world.insert_resource(OverlayMenu::Pause);
                } else {
                    self.sim.world.insert_resource(OverlayMenu::None);
                }
            }
            UiAction::SetLanguage(ref lang) => {
                let locale = &mut self.sim.world.resource_mut::<I18nStrings>().0;
                if locale.available.contains(lang) {
                    locale.set_locale(lang);
                }
            }
        }
    }

    pub fn view(&mut self, sched: &mut Scheduler, ctx: &RenderContext, dt: Duration) -> View {
        request_frame();
        install_game_shortcuts(
            &self.shortcut_edges,
            SHORTCUT_PAUSE,
            SHORTCUT_RESTART,
            SHORTCUT_CONFIRM,
        );
        self.feed_polled(sched);
        self.feed_input();
        self.process_ui_actions();
        networking::pre_frame(&mut self.sim.world, dt);
        self.advance(dt);
        networking::post_frame(&mut self.sim.world);
        if self
            .sim
            .world
            .get_resource::<QuitRequested>()
            .is_some_and(|quit| quit.0)
        {
            std::process::exit(0);
        }
        let settings = &self.sim.world.resource::<SaveData>().settings;
        self.audio.apply_volumes(
            settings.master_volume,
            settings.sfx_volume,
            settings.music_volume,
        );
        self.audio.drain(&mut self.sim.world);
        self.audio.update(dt.as_secs_f32());
        sync_shared_ui(&mut self.sim.world, &mut self.ui);
        if !self.images.is_loaded() {
            self.images = GameImages::load(ctx);
        }
        self.ui.world = sync_world_render(&mut self.sim.world, &mut self.render, &self.images, dt);
        let overlay_rc = remember(OverlayHandle::new);
        let overlay = (*overlay_rc).clone();
        let focus = remember(FocusRequester::new);
        let focus_positioned = (*focus).clone();
        let focus_staging = self.staging.clone();
        let key_staging = self.staging.clone();
        let chat_keys = self.chat_keys.clone();
        let chat_open = *self.sim.world.resource::<AppState>() == AppState::InGame
            && matches!(
                *self.sim.world.resource::<GamePhase>(),
                GamePhase::Meeting | GamePhase::Voting
            );
        let actions = self.sim.world.resource::<UiActions>().0.clone();
        let content = compose_root(self.ui.clone(), actions);
        let root = ZStack(
            Modifier::new()
                .fill_max_size()
                .focusable(true)
                .focus_requester((*focus).clone())
                .on_globally_positioned(move |_| focus_positioned.request_focus())
                .on_focus_changed(move |focused| {
                    focus_staging.borrow_mut().set_window_focused(focused);
                })
                .on_key_event(move |ke| {
                    key_staging.borrow_mut().handle_key(&ke);
                    if chat_open
                        && matches!(ke.event_type, KeyEventType::Down)
                        && matches!(
                            ke.key,
                            Key::Enter | Key::Backspace | Key::Space | Key::Character(_)
                        )
                    {
                        chat_keys.borrow_mut().push(ke);
                        return true;
                    }
                    false
                }),
        )
        .child(content);
        overlay.host(Modifier::new().fill_max_size(), root)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

struct AppRuntime {
    app: App,
    previous: Instant,
}

fn runtime() -> AppRuntime {
    AppRuntime {
        app: App::new(),
        previous: Instant::now(),
    }
}

impl AppRuntime {
    fn view(&mut self, sched: &mut Scheduler, ctx: &RenderContext) -> View {
        let now = Instant::now();
        let dt = now
            .duration_since(self.previous)
            .min(Duration::from_millis(250));
        self.previous = now;
        self.app.view(sched, ctx, dt)
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub fn run_desktop() -> anyhow::Result<()> {
    let mut runtime = runtime();
    repame_shell::run_desktop("Opensus", (1280, 720), move |sched, ctx| {
        runtime.view(sched, ctx)
    })
}

#[cfg(target_arch = "wasm32")]
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
    let mut runtime = runtime();
    repame_shell::run_web(move |sched, ctx| runtime.view(sched, ctx))
}

#[cfg(target_os = "android")]
pub fn run_android(app: winit::platform::android::activity::AndroidApp) -> anyhow::Result<()> {
    let mut runtime = runtime();
    repame_shell::run_android(app, move |sched, ctx| runtime.view(sched, ctx))
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: winit::platform::android::activity::AndroidApp) {
    if let Some(dir) = app.internal_data_path() {
        game_utils::storage::set_android_data_dir(dir);
    }
    if let Err(error) = run_android(app) {
        log::error!("opensus failed to start: {error:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_of(app: &App) -> AppState {
        *app.sim.world.resource::<AppState>()
    }

    #[test]
    fn fixed_step_advances_after_boot() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::Title);
        let ran = app.advance(Duration::from_millis(51));
        let elapsed = app.sim.world.resource::<repame_sim::SimTime>().elapsed_secs;
        let expected = 3.0 * app.sim.step.as_secs_f64();
        assert_eq!(ran, 3);
        assert!((elapsed - expected).abs() < 1e-12);
    }

    #[test]
    fn splash_fades_into_title() {
        let mut app = App::new();
        for _ in 0..10 {
            app.advance(Duration::from_millis(33));
        }
        assert_eq!(state_of(&app), AppState::Splash);
        for _ in 0..90 {
            app.advance(Duration::from_millis(33));
        }
        assert_eq!(state_of(&app), AppState::Title);
        assert!(!app.sim.world.resource::<TransitionFx>().blocking());
    }

    #[test]
    fn loading_hands_off_to_lobby() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::Loading);
        for _ in 0..40 {
            app.advance(Duration::from_millis(33));
        }
        assert_eq!(state_of(&app), AppState::Lobby);
        assert!(!app.sim.world.resource::<TransitionFx>().blocking());
    }

    #[test]
    fn boot_registers_i18n_and_save() {
        let app = App::new();
        assert_eq!(
            app.sim.world.resource::<I18nStrings>().get_str("app-title"),
            "Opensus"
        );
        assert!(
            app.sim
                .world
                .get_resource::<SaveData>()
                .is_some_and(|save| save.version <= SAVE_VERSION)
        );
    }

    #[test]
    fn escape_pauses_and_unpauses_in_game() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);
        app.sim.world.insert_resource(InputEdges {
            pause: true,
            ..Default::default()
        });
        app.advance(Duration::from_millis(16));
        assert_eq!(*app.sim.world.resource::<OverlayMenu>(), OverlayMenu::Pause);
        assert!(app.sim.world.resource::<Paused>().0);

        app.sim.world.insert_resource(InputEdges {
            pause: true,
            ..Default::default()
        });
        app.advance(Duration::from_millis(16));
        assert_eq!(*app.sim.world.resource::<OverlayMenu>(), OverlayMenu::None);
        assert!(app.sim.world.resource::<Paused>().0);
        assert!(app.sim.world.resource::<PendingUnpause>().0.is_some());

        app.advance(Duration::from_millis(250));
        assert!(!app.sim.world.resource::<Paused>().0);
    }

    #[test]
    fn pause_edges_gated_by_state_and_transition() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::Title);
        app.sim.world.insert_resource(InputEdges {
            pause: true,
            ..Default::default()
        });
        app.advance(Duration::from_millis(16));
        assert!(!app.sim.world.resource::<Paused>().0);
        assert!(!app.sim.world.resource::<InputEdges>().pause);

        goto_state(&mut app.sim.world, AppState::InGame);
        app.begin_to_state(AppState::Title);
        app.shortcut_edges.borrow_mut().pause = true;
        app.feed_input();
        assert!(!app.sim.world.resource::<InputEdges>().pause);
        app.sim.world.insert_resource(InputEdges {
            pause: true,
            ..Default::default()
        });
        app.advance(Duration::from_millis(16));
        assert!(!app.sim.world.resource::<Paused>().0);
    }

    #[test]
    fn shortcut_edge_reaches_pause_input() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);
        app.shortcut_edges.borrow_mut().pause = true;
        app.feed_input();
        assert!(app.sim.world.resource::<InputEdges>().pause);
        app.advance(Duration::from_millis(16));
        assert!(app.sim.world.resource::<Paused>().0);
        assert!(!app.shortcut_edges.borrow().pause);
    }

    fn push_action(app: &mut App, action: UiAction) {
        app.sim
            .world
            .resource::<UiActions>()
            .0
            .lock()
            .unwrap()
            .push(action);
    }

    fn settle(app: &mut App) {
        for _ in 0..150 {
            app.advance(Duration::from_millis(33));
        }
    }

    fn advance_for(app: &mut App, total_ms: u64) {
        let mut left = total_ms;
        while left > 0 {
            let chunk = left.min(100);
            app.advance(Duration::from_millis(chunk));
            left -= chunk;
        }
    }

    #[test]
    fn menu_actions_drive_title_lobby_ingame() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::Title);

        push_action(&mut app, UiAction::PlayOffline);
        app.process_ui_actions();
        assert_eq!(app.pending_state, Some(AppState::Loading));
        assert_eq!(
            *app.sim.world.resource::<crate::game::RuntimeMode>(),
            crate::game::RuntimeMode::Local
        );

        settle(&mut app);
        assert_eq!(state_of(&app), AppState::Lobby);
        let lobby = app.sim.world.resource::<crate::game::LobbyState>();
        assert_eq!(lobby.slots.len(), 4);
        assert!(lobby.is_host);
        assert!(!lobby.local_ready);
        assert!(lobby.slots[0].is_local);
        assert!(lobby.slots[0].is_host);
        assert!(
            lobby.slots[1..]
                .iter()
                .all(|slot| slot.is_bot && slot.ready)
        );
        assert_eq!(lobby.slots[0].name, "Agent");

        push_action(&mut app, UiAction::StartMatch);
        app.process_ui_actions();
        assert_eq!(app.pending_state, None);

        push_action(&mut app, UiAction::ToggleReady);
        app.process_ui_actions();
        assert!(
            app.sim
                .world
                .resource::<crate::game::LobbyState>()
                .local_ready
        );
        assert!(app.sim.world.resource::<crate::game::LobbyState>().slots[0].ready);

        push_action(&mut app, UiAction::StartMatch);
        app.process_ui_actions();
        assert_eq!(app.pending_state, Some(AppState::InGame));

        settle(&mut app);
        assert_eq!(state_of(&app), AppState::InGame);
        assert!(!app.sim.world.resource::<Paused>().0);
    }

    #[test]
    fn pause_menu_buttons_drive_frozen_sim() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);

        push_action(&mut app, UiAction::TogglePause);
        app.process_ui_actions();
        assert_eq!(*app.sim.world.resource::<OverlayMenu>(), OverlayMenu::Pause);
        assert!(app.sim.world.resource::<Paused>().0);
        assert_eq!(app.advance(Duration::from_millis(16)), 0);

        push_action(&mut app, UiAction::Resume);
        app.process_ui_actions();
        assert_eq!(*app.sim.world.resource::<OverlayMenu>(), OverlayMenu::None);
        assert!(app.sim.world.resource::<PendingUnpause>().0.is_some());
        app.advance(Duration::from_millis(250));
        assert!(!app.sim.world.resource::<Paused>().0);

        push_action(&mut app, UiAction::TogglePause);
        app.process_ui_actions();
        push_action(&mut app, UiAction::QuitToTitle);
        app.process_ui_actions();
        assert_eq!(app.pending_state, Some(AppState::Title));
        assert!(!app.sim.world.resource::<Paused>().0);
        assert_eq!(*app.sim.world.resource::<OverlayMenu>(), OverlayMenu::None);
        settle(&mut app);
        assert_eq!(state_of(&app), AppState::Title);
    }

    #[test]
    fn settings_overlay_lifecycle_with_escape_and_pause() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);

        push_action(&mut app, UiAction::OpenSettings);
        app.process_ui_actions();
        assert_eq!(
            *app.sim.world.resource::<OverlayMenu>(),
            OverlayMenu::Settings
        );
        assert_eq!(app.ui.saved_language, "en");

        push_action(&mut app, UiAction::SetMasterVol(0.4));
        app.process_ui_actions();
        assert!((app.ui.master_vol - 0.4).abs() < 1e-6);

        app.sim.world.insert_resource(InputEdges {
            pause: true,
            ..Default::default()
        });
        app.advance(Duration::from_millis(16));
        assert_eq!(*app.sim.world.resource::<OverlayMenu>(), OverlayMenu::None);
        assert!(!app.sim.world.resource::<Paused>().0);

        push_action(&mut app, UiAction::TogglePause);
        app.process_ui_actions();
        push_action(&mut app, UiAction::OpenSettings);
        app.process_ui_actions();
        assert_eq!(
            *app.sim.world.resource::<OverlayMenu>(),
            OverlayMenu::Settings
        );

        app.sim.world.insert_resource(InputEdges {
            pause: true,
            ..Default::default()
        });
        app.advance(Duration::from_millis(16));
        assert_eq!(*app.sim.world.resource::<OverlayMenu>(), OverlayMenu::Pause);
        assert!(app.sim.world.resource::<Paused>().0);

        push_action(&mut app, UiAction::CloseOverlay);
        app.process_ui_actions();
        assert_eq!(*app.sim.world.resource::<OverlayMenu>(), OverlayMenu::None);
        assert!(app.sim.world.resource::<PendingUnpause>().0.is_some());
    }

    #[test]
    fn language_action_switches_locale() {
        let mut app = App::new();
        push_action(&mut app, UiAction::SetLanguage("de".to_string()));
        app.process_ui_actions();
        assert_eq!(app.sim.world.resource::<I18nStrings>().0.current, "de");

        push_action(&mut app, UiAction::SetLanguage("xx".to_string()));
        app.process_ui_actions();
        assert_eq!(app.sim.world.resource::<I18nStrings>().0.current, "de");
    }

    #[test]
    fn meeting_actions_queue_with_local_slot_id() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::Lobby);

        push_action(&mut app, UiAction::CallEmergency);
        push_action(&mut app, UiAction::CastVote(10));
        push_action(&mut app, UiAction::SkipVote);
        app.process_ui_actions();

        let queue = &app.sim.world.resource::<crate::game::MeetingCommands>().0;
        assert_eq!(queue.len(), 3);
        assert!(
            matches!(
                queue[0],
                crate::game::MeetingCommand::Emergency { actor_id: 1 }
            ),
            "{:?}",
            queue[0]
        );
        assert!(
            matches!(
                queue[1],
                crate::game::MeetingCommand::Vote {
                    voter_id: 1,
                    target: 10
                }
            ),
            "{:?}",
            queue[1]
        );
        assert!(
            matches!(queue[2], crate::game::MeetingCommand::Skip { voter_id: 1 }),
            "{:?}",
            queue[2]
        );
    }

    #[test]
    fn color_cycle_updates_save_and_local_slot() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::Lobby);
        let before = app.sim.world.resource::<SaveData>().preferred_color_index;
        push_action(&mut app, UiAction::CycleColor);
        app.process_ui_actions();
        let after = app.sim.world.resource::<SaveData>().preferred_color_index;
        assert_eq!(after, (before + 1) % 12);
        assert_eq!(
            app.sim.world.resource::<crate::game::LobbyState>().slots[0].color_index,
            after
        );
    }

    fn local_entity(app: &mut App) -> Entity {
        let world = &mut app.sim.world;
        let mut q = world.query_filtered::<Entity, With<crate::game::LocalPlayer>>();
        q.single(world).unwrap()
    }

    fn set_local_position(app: &mut App, position: glam::Vec2) {
        let world = &mut app.sim.world;
        let mut q =
            world.query_filtered::<&mut crate::game::Position, With<crate::game::LocalPlayer>>();
        q.single_mut(world).unwrap().0 = position;
    }

    #[test]
    fn match_setup_spawns_fallback_crew() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);

        let world = &mut app.sim.world;
        let mut q = world.query::<&crate::game::Role>();
        let roles: Vec<crate::game::Role> = q.iter(world).copied().collect();
        assert_eq!(roles.len(), 4);
        assert_eq!(
            roles
                .iter()
                .filter(|role| matches!(role, crate::game::Role::Impostor))
                .count(),
            1
        );
        let stats = world.resource::<crate::game::MatchStats>();
        assert_eq!(stats.players_spawned, 4);
        assert_eq!(stats.impostors_spawned, 1);
        let board = world.resource::<TaskBoard>();
        assert_eq!(board.total, 9);
        assert_eq!(board.completed, 0);
        assert!(matches!(
            *world.resource::<GamePhase>(),
            GamePhase::RoleReveal
        ));
    }

    #[test]
    fn role_reveal_opens_play() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);
        advance_for(&mut app, 3100);
        assert!(matches!(
            *app.sim.world.resource::<GamePhase>(),
            GamePhase::Playing
        ));
    }

    #[test]
    fn emergency_meeting_is_gated_by_button_range() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);
        advance_for(&mut app, 3100);

        let local = local_entity(&mut app);
        app.sim
            .world
            .entity_mut(local)
            .insert(crate::game::EmergencyCooldownLeft(0.0));
        set_local_position(&mut app, glam::Vec2::new(500.0, 500.0));

        app.sim.world.insert_resource(InputEdges {
            emergency: true,
            ..Default::default()
        });
        app.advance(Duration::from_millis(50));
        assert!(matches!(
            *app.sim.world.resource::<GamePhase>(),
            GamePhase::Playing
        ));

        set_local_position(&mut app, crate::game::EMERGENCY_BUTTON_POSITION);
        app.sim.world.insert_resource(InputEdges {
            emergency: true,
            ..Default::default()
        });
        app.advance(Duration::from_millis(50));

        assert!(matches!(
            *app.sim.world.resource::<GamePhase>(),
            GamePhase::Meeting
        ));
        assert_eq!(
            app.sim.world.resource::<crate::game::MeetingState>().prompt,
            "Emergency Meeting!"
        );
        let world = &mut app.sim.world;
        let mut q =
            world.query_filtered::<&crate::game::EmergenciesLeft, With<crate::game::LocalPlayer>>();
        assert_eq!(q.single(world).unwrap().0, 0);
    }

    #[test]
    fn meeting_resolves_after_local_skip_vote() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);
        advance_for(&mut app, 3100);

        let local = local_entity(&mut app);
        app.sim
            .world
            .entity_mut(local)
            .insert(crate::game::EmergencyCooldownLeft(0.0));
        set_local_position(&mut app, crate::game::EMERGENCY_BUTTON_POSITION);
        app.sim.world.insert_resource(InputEdges {
            emergency: true,
            ..Default::default()
        });
        app.advance(Duration::from_millis(50));
        assert!(matches!(
            *app.sim.world.resource::<GamePhase>(),
            GamePhase::Meeting
        ));

        let local_id = app
            .sim
            .world
            .resource::<crate::game::LocalPlayerId>()
            .0
            .unwrap();
        {
            let mut meeting = app.sim.world.resource_mut::<crate::game::MeetingState>();
            assert_eq!(meeting.options.len(), 4);
            let bot_ids: Vec<u64> = meeting
                .options
                .iter()
                .map(|option| option.player_id)
                .filter(|&id| id != local_id)
                .collect();
            assert_eq!(bot_ids.len(), 3);
            for id in bot_ids {
                meeting.votes.insert(id, None);
            }
        }

        advance_for(&mut app, 18100);
        assert!(matches!(
            *app.sim.world.resource::<GamePhase>(),
            GamePhase::Voting
        ));
        assert_eq!(
            app.sim
                .world
                .resource::<crate::game::MeetingState>()
                .votes
                .len(),
            3
        );

        app.sim
            .world
            .resource_mut::<crate::game::MeetingCommands>()
            .0
            .push(crate::game::MeetingCommand::Skip { voter_id: local_id });
        app.advance(Duration::from_millis(50));
        assert!(matches!(
            *app.sim.world.resource::<GamePhase>(),
            GamePhase::Results
        ));
        {
            let meeting = app.sim.world.resource::<crate::game::MeetingState>();
            assert_eq!(meeting.tallies, vec![("Skip".to_string(), 4)]);
            assert_eq!(meeting.result_text, "No one was ejected. (Skip / Tie)");
        }

        advance_for(&mut app, 4500);
        assert!(matches!(
            *app.sim.world.resource::<GamePhase>(),
            GamePhase::Playing
        ));
        assert!(
            app.sim
                .world
                .resource::<crate::game::MeetingState>()
                .prompt
                .is_empty()
        );
    }

    #[test]
    fn kill_and_report_open_a_meeting() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);
        {
            let mut cfg = app.sim.world.resource::<MatchConfig>().clone();
            cfg.bot_report_range = 0.0;
            app.sim.world.insert_resource(cfg);
        }
        advance_for(&mut app, 3100);

        let local = local_entity(&mut app);
        let victim_position = {
            let world = &mut app.sim.world;
            let mut q = world.query_filtered::<&crate::game::Position, (
                With<crate::game::Player>,
                Without<crate::game::LocalPlayer>,
            )>();
            q.iter(world).next().unwrap().0
        };
        {
            let world = &mut app.sim.world;
            let mut q = world.query_filtered::<&mut crate::game::Role, With<crate::game::Alive>>();
            for mut role in q.iter_mut(world) {
                *role = crate::game::Role::Crewmate;
            }
        }
        app.sim.world.entity_mut(local).insert((
            crate::game::Role::Impostor,
            crate::game::KillCooldownLeft(0.0),
            crate::game::Position(victim_position + glam::Vec2::new(18.0, 0.0)),
        ));

        app.sim.world.insert_resource(InputEdges {
            kill: true,
            ..Default::default()
        });
        app.advance(Duration::from_millis(50));

        assert!(matches!(
            *app.sim.world.resource::<GamePhase>(),
            GamePhase::Playing
        ));
        let world = &mut app.sim.world;
        let mut q = world.query_filtered::<Entity, With<crate::game::Ghost>>();
        assert_eq!(q.iter(world).count(), 1);
        let mut q = world.query_filtered::<Entity, With<crate::game::Body>>();
        assert_eq!(q.iter(world).count(), 1);
        let mut q = world
            .query_filtered::<&crate::game::KillCooldownLeft, With<crate::game::LocalPlayer>>();
        assert!(q.single(world).unwrap().0 > 20.0);

        app.sim.world.insert_resource(InputEdges {
            report: true,
            ..Default::default()
        });
        app.advance(Duration::from_millis(50));

        assert!(matches!(
            *app.sim.world.resource::<GamePhase>(),
            GamePhase::Meeting
        ));
        assert!(
            app.sim
                .world
                .resource::<crate::game::MeetingState>()
                .prompt
                .ends_with("body was reported!")
        );
    }

    #[test]
    fn task_bar_completion_ends_match() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);
        app.sim.world.insert_resource(GameOverSaved(true));
        advance_for(&mut app, 3100);
        assert!(matches!(
            *app.sim.world.resource::<GamePhase>(),
            GamePhase::Playing
        ));

        let before = app.sim.world.resource::<SaveData>().games_played;
        {
            let mut board = app.sim.world.resource_mut::<TaskBoard>();
            board.completed = board.total;
        }
        app.advance(Duration::from_millis(50));

        assert!(matches!(
            *app.sim.world.resource::<GamePhase>(),
            GamePhase::GameOver {
                crew_win: true,
                reason: crate::game::WinReason::Tasks
            }
        ));
        assert_eq!(
            app.sim.world.resource::<SaveData>().games_played,
            before + 1
        );
        assert!(app.sim.world.resource::<GameOverSaved>().0);
    }

    #[test]
    fn bot_paths_toward_assigned_task() {
        let mut app = App::new();
        app.sim.world.insert_resource(MatchSeed(7));
        goto_state(&mut app.sim.world, AppState::InGame);
        app.sim.world.insert_resource(GameOverSaved(true));
        app.sim
            .world
            .insert_resource(crate::game::SabotageCooldown {
                remaining: f32::MAX,
            });
        advance_for(&mut app, 3100);
        assert!(matches!(
            *app.sim.world.resource::<GamePhase>(),
            GamePhase::Playing
        ));

        let station = crate::game::TASK_STATIONS[0];
        let bot = {
            let world = &mut app.sim.world;
            let mut query = world.query_filtered::<(
                Entity,
                &crate::game::Position,
                &mut crate::game::TaskAssignments,
                &crate::game::Role,
            ), (With<crate::game::AiPlayer>, With<crate::game::Alive>)>(
            );
            let mut found = None;
            for (entity, position, mut assignments, role) in query.iter_mut(world) {
                if !matches!(role, crate::game::Role::Crewmate) {
                    continue;
                }
                assignments.assigned = vec![station.0];
                assignments.completed.clear();
                found = Some((entity, position.0.distance(station.2)));
                break;
            }
            found.unwrap()
        };

        advance_for(&mut app, 4000);

        let world = &mut app.sim.world;
        let mut query =
            world.query_filtered::<&crate::game::Position, With<crate::game::AiPlayer>>();
        let after = query.get(world, bot.0).unwrap().0.distance(station.2);

        assert!(after < bot.1 * 0.6, "before {} after {}", bot.1, after);
    }

    #[test]
    fn bot_completes_assigned_task() {
        let mut app = App::new();
        app.sim.world.insert_resource(MatchSeed(9));
        goto_state(&mut app.sim.world, AppState::InGame);
        app.sim.world.insert_resource(GameOverSaved(true));
        advance_for(&mut app, 3100);
        assert!(matches!(
            *app.sim.world.resource::<GamePhase>(),
            GamePhase::Playing
        ));

        let task_id = {
            let world = &mut app.sim.world;
            let mut stations = world.query::<(&crate::game::TaskStation, &crate::game::Position)>();
            let station_positions: Vec<(u32, glam::Vec2)> = stations
                .iter(world)
                .map(|(station, position)| (station.id, position.0))
                .collect();

            let mut bots = world.query_filtered::<(
                &mut crate::game::TaskAssignments,
                &mut crate::game::AiPlayer,
                &mut crate::game::Position,
                &crate::game::Role,
            ), (With<crate::game::AiPlayer>, With<crate::game::Alive>)>(
            );
            let mut found = None;
            for (mut assignments, mut ai, mut position, role) in bots.iter_mut(world) {
                if !matches!(role, crate::game::Role::Crewmate) {
                    continue;
                }
                let id = assignments.assigned[0];
                let (_, station_position) = *station_positions
                    .iter()
                    .find(|(station_id, _)| *station_id == id)
                    .unwrap();
                assignments.assigned = vec![id];
                assignments.completed.clear();
                ai.target_task = None;
                position.0 = station_position + glam::Vec2::new(30.0, 0.0);
                found = Some(id);
                break;
            }
            found.unwrap()
        };

        advance_for(&mut app, 3000);

        let world = &mut app.sim.world;
        let mut query = world.query_filtered::<&crate::game::TaskAssignments, (
            With<crate::game::AiPlayer>,
            With<crate::game::Alive>,
        )>();
        assert!(
            query
                .iter(world)
                .any(|assignments| assignments.completed.contains(&task_id))
        );
        assert!(world.resource::<TaskBoard>().completed >= 1);
    }

    #[test]
    fn ai_leaves_local_intent_untouched() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);
        advance_for(&mut app, 3100);
        assert!(matches!(
            *app.sim.world.resource::<GamePhase>(),
            GamePhase::Playing
        ));

        let start = {
            let world = &mut app.sim.world;
            let mut query =
                world.query_filtered::<&crate::game::Position, With<crate::game::LocalPlayer>>();
            query.single(world).unwrap().0
        };

        app.sim.world.insert_resource(LocalControls {
            direction: glam::Vec2::X,
            interact: true,
        });
        app.advance(Duration::from_millis(50));

        let world = &mut app.sim.world;
        let mut local = world.query_filtered::<(&crate::game::PlayerIntent, &crate::game::Position), (
            With<crate::game::LocalPlayer>,
            With<crate::game::Alive>,
        )>();
        let (intent, position) = local.single(world).unwrap();
        assert_eq!(intent.movement, glam::Vec2::X);
        assert!(intent.interact);
        assert!(position.0.x > start.x);

        let mut bots =
            world.query_filtered::<&crate::game::PlayerIntent, With<crate::game::AiPlayer>>();
        assert!(
            bots.iter(world)
                .any(|intent| intent.movement.length_squared() > 0.0)
        );
    }

    #[test]
    fn entering_match_pushes_role_reveal_cue_once() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);
        app.advance(Duration::from_millis(50));

        let pending = app.sim.world.resource::<crate::game::PendingCues>();
        assert_eq!(
            pending
                .0
                .iter()
                .filter(|cue| **cue == "role_reveal")
                .count(),
            1
        );
        assert!(!pending.0.contains(&"body"));
        assert!(!pending.0.contains(&"meeting"));
        assert!(!pending.0.contains(&"task_done"));
    }

    #[test]
    fn added_body_pushes_body_cue_once() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);
        app.sim.world.spawn((
            crate::game::Body {
                player_id: 9,
                name: "Victim".to_string(),
                reported: false,
            },
            crate::game::Position(glam::Vec2::ZERO),
        ));
        app.advance(Duration::from_millis(50));

        {
            let pending = app.sim.world.resource::<crate::game::PendingCues>();
            assert_eq!(pending.0.iter().filter(|cue| **cue == "body").count(), 1);
        }

        app.advance(Duration::from_millis(50));
        let pending = app.sim.world.resource::<crate::game::PendingCues>();
        assert_eq!(pending.0.iter().filter(|cue| **cue == "body").count(), 1);
    }

    fn force_all_crew(app: &mut App) {
        let world = &mut app.sim.world;
        let mut q = world.query_filtered::<&mut crate::game::Role, With<crate::game::Alive>>();
        for mut role in q.iter_mut(world) {
            *role = crate::game::Role::Crewmate;
        }
    }

    fn make_local_impostor(app: &mut App) {
        let local = local_entity(app);
        force_all_crew(app);
        app.sim.world.entity_mut(local).insert((
            crate::game::Role::Impostor,
            crate::game::KillCooldownLeft(0.0),
        ));
    }

    #[test]
    fn local_impostor_sabotage_activates_with_key_edge() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);
        advance_for(&mut app, 3100);
        make_local_impostor(&mut app);

        app.sim.world.insert_resource(InputEdges {
            sabotage_oxygen: true,
            ..Default::default()
        });
        app.advance(Duration::from_millis(50));

        {
            let sabotage = app.sim.world.resource::<crate::game::ActiveSabotage>();
            assert_eq!(sabotage.kind, Some(crate::game::SabotageKind::Oxygen));
            assert_eq!(sabotage.fixes_needed, 2);
            assert_eq!(sabotage.fixes_done, 0);
        }
        let cooldown = app
            .sim
            .world
            .resource::<crate::game::SabotageCooldown>()
            .remaining;
        assert!(cooldown > 19.0, "cooldown {cooldown}");
        let trauma = app.sim.world.resource::<crate::game::Trauma>().value;
        assert!(trauma > 0.4, "trauma {trauma}");
        {
            let pending = app.sim.world.resource::<crate::game::PendingCues>();
            assert!(pending.0.contains(&"sabotage_start"));
        }

        app.sim.world.insert_resource(InputEdges {
            sabotage_reactor: true,
            ..Default::default()
        });
        app.advance(Duration::from_millis(50));
        assert_eq!(
            app.sim.world.resource::<crate::game::ActiveSabotage>().kind,
            Some(crate::game::SabotageKind::Oxygen)
        );
        let trauma_after = app.sim.world.resource::<crate::game::Trauma>().value;
        assert!(trauma_after < trauma, "{trauma_after} vs {trauma}");
    }

    #[test]
    fn crew_cannot_activate_sabotage() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);
        advance_for(&mut app, 3100);
        force_all_crew(&mut app);

        app.sim.world.insert_resource(InputEdges {
            sabotage_lights: true,
            ..Default::default()
        });
        app.advance(Duration::from_millis(50));

        assert!(
            app.sim
                .world
                .resource::<crate::game::ActiveSabotage>()
                .kind
                .is_none()
        );
        assert_eq!(
            app.sim
                .world
                .resource::<crate::game::SabotageCooldown>()
                .remaining,
            0.0
        );
        assert_eq!(app.sim.world.resource::<crate::game::Trauma>().value, 0.0);
    }

    #[test]
    fn critical_sabotage_timeout_wins_for_impostors() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);
        advance_for(&mut app, 3100);
        make_local_impostor(&mut app);
        app.sim
            .world
            .resource_mut::<crate::game::MatchConfig>()
            .oxygen_time = 0.2;

        app.sim.world.insert_resource(InputEdges {
            sabotage_oxygen: true,
            ..Default::default()
        });
        app.advance(Duration::from_millis(50));
        assert!(
            app.sim
                .world
                .resource::<crate::game::ActiveSabotage>()
                .is_active()
        );

        let before = app.sim.world.resource::<SaveData>().games_played;
        let impostor_wins_before = app.sim.world.resource::<SaveData>().impostor_wins;
        advance_for(&mut app, 500);

        match *app.sim.world.resource::<GamePhase>() {
            GamePhase::GameOver { crew_win, reason } => {
                assert!(!crew_win);
                assert!(matches!(reason, crate::game::WinReason::Sabotage));
            }
            other => panic!("expected game over, got {other:?}"),
        }
        let save = app.sim.world.resource::<SaveData>();
        assert_eq!(save.games_played, before + 1);
        assert_eq!(save.impostor_wins, impostor_wins_before + 1);
    }

    #[test]
    fn crew_bots_fix_sabotage_and_prevent_loss() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);
        advance_for(&mut app, 3100);
        make_local_impostor(&mut app);
        {
            let cfg = &mut *app.sim.world.resource_mut::<crate::game::MatchConfig>();
            cfg.bot_kill_aggression = 0.0;
        }
        app.sim.world.resource_mut::<TaskBoard>().total = 9999;

        app.sim.world.insert_resource(InputEdges {
            sabotage_oxygen: true,
            ..Default::default()
        });
        app.advance(Duration::from_millis(50));
        assert!(
            app.sim
                .world
                .resource::<crate::game::ActiveSabotage>()
                .is_active()
        );

        let stations = crate::game::OXYGEN_STATIONS;
        let bots: Vec<Entity> = {
            let world = &mut app.sim.world;
            let mut q = world.query_filtered::<(Entity, &crate::game::Role), (
                With<crate::game::AiPlayer>,
                With<crate::game::Alive>,
            )>();
            q.iter(world)
                .filter(|(_, role)| matches!(role, crate::game::Role::Crewmate))
                .map(|(entity, _)| entity)
                .collect()
        };
        assert!(bots.len() >= 2);
        let world = &mut app.sim.world;
        let mut q =
            world.query_filtered::<&mut crate::game::Position, With<crate::game::AiPlayer>>();
        for (index, entity) in bots.iter().take(2).enumerate() {
            q.get_mut(world, *entity).unwrap().0 = stations[index];
        }

        advance_for(&mut app, 4000);
        assert!(
            !app.sim
                .world
                .resource::<crate::game::ActiveSabotage>()
                .is_active()
        );

        advance_for(&mut app, 30_000);
        assert!(matches!(
            *app.sim.world.resource::<GamePhase>(),
            GamePhase::Playing
        ));
    }

    #[test]
    fn bot_impostor_activates_sabotage() {
        let mut app = App::new();
        app.sim.world.insert_resource(MatchSeed(11));
        goto_state(&mut app.sim.world, AppState::InGame);
        advance_for(&mut app, 3100);
        force_all_crew(&mut app);
        {
            let world = &mut app.sim.world;
            let mut q = world
                .query_filtered::<Entity, (With<crate::game::AiPlayer>, With<crate::game::Alive>)>(
                );
            let bot = q.iter(world).next().expect("alive bot impostor candidate");
            world.entity_mut(bot).insert(crate::game::Role::Impostor);
        }
        {
            let cfg = &mut *app.sim.world.resource_mut::<crate::game::MatchConfig>();
            cfg.bot_kill_aggression = 0.0;
            cfg.bot_report_range = 0.0;
        }
        app.sim.world.resource_mut::<TaskBoard>().total = 9999;

        let mut activated = false;
        for _ in 0..300 {
            app.advance(Duration::from_millis(100));
            if app
                .sim
                .world
                .resource::<crate::game::SabotageCooldown>()
                .remaining
                > 0.0
            {
                activated = true;
                break;
            }
        }
        assert!(activated, "bot impostor never activated a sabotage");
    }

    #[test]
    fn trauma_decays_to_zero() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);
        app.sim
            .world
            .insert_resource(crate::game::Trauma { value: 0.5 });
        advance_for(&mut app, 1000);
        assert_eq!(app.sim.world.resource::<crate::game::Trauma>().value, 0.0);
    }

    fn chat_key(key: Key) -> KeyEvent {
        KeyEvent {
            key,
            modifiers: repose_core::input::Modifiers::default(),
            is_repeat: false,
            event_type: KeyEventType::Down,
            utf16_code_point: 0,
            physical: None,
        }
    }

    #[test]
    fn chat_capture_apply_cue_and_ghost_channel() {
        let mut app = App::new();
        goto_state(&mut app.sim.world, AppState::InGame);
        advance_for(&mut app, 3100);
        assert!(matches!(
            *app.sim.world.resource::<GamePhase>(),
            GamePhase::Playing
        ));

        app.chat_keys
            .borrow_mut()
            .push(chat_key(Key::Character('x')));
        app.feed_input();
        assert!(
            app.sim
                .world
                .resource::<crate::game::ChatKeys>()
                .0
                .is_empty()
        );

        let local = local_entity(&mut app);
        app.sim
            .world
            .entity_mut(local)
            .insert(crate::game::EmergencyCooldownLeft(0.0));
        set_local_position(&mut app, crate::game::EMERGENCY_BUTTON_POSITION);
        app.sim.world.insert_resource(InputEdges {
            emergency: true,
            ..Default::default()
        });
        app.advance(Duration::from_millis(50));
        assert!(matches!(
            *app.sim.world.resource::<GamePhase>(),
            GamePhase::Meeting
        ));

        app.chat_keys
            .borrow_mut()
            .extend([chat_key(Key::Character('h')), chat_key(Key::Character('i'))]);
        app.feed_input();
        advance_for(&mut app, 300);
        assert!(
            app.sim
                .world
                .resource::<crate::game::ChatState>()
                .entries
                .is_empty()
        );
        assert_eq!(
            app.sim.world.resource::<crate::game::ChatInputBuffer>().0,
            "hi"
        );
        sync_shared_ui(&mut app.sim.world, &mut app.ui);
        assert_eq!(app.ui.chat_buffer, "hi");
        assert!(app.ui.chat_entries.is_empty());

        app.chat_keys.borrow_mut().push(chat_key(Key::Enter));
        app.feed_input();
        advance_for(&mut app, 300);
        {
            let chat = app.sim.world.resource::<crate::game::ChatState>();
            assert_eq!(chat.entries.len(), 1);
            assert_eq!(chat.entries[0].text, "hi");
            assert!(!chat.entries[0].ghost);
        }
        assert!(
            app.sim
                .world
                .resource::<crate::game::ChatInputBuffer>()
                .0
                .is_empty()
        );
        assert!(app.sim.world.resource::<PendingCues>().0.contains(&"chat"));
        sync_shared_ui(&mut app.sim.world, &mut app.ui);
        assert_eq!(app.ui.chat_entries.len(), 1);
        assert_eq!(app.ui.chat_entries[0].1, "hi");
        assert!(!app.ui.chat_is_ghost_channel);

        app.sim
            .world
            .entity_mut(local)
            .remove::<crate::game::Alive>();
        app.sim.world.entity_mut(local).insert(crate::game::Ghost);
        app.chat_keys.borrow_mut().extend([
            chat_key(Key::Character('b')),
            chat_key(Key::Character('y')),
            chat_key(Key::Enter),
        ]);
        app.feed_input();
        advance_for(&mut app, 300);
        {
            let chat = app.sim.world.resource::<crate::game::ChatState>();
            assert_eq!(chat.entries.len(), 2);
            assert!(chat.entries[1].ghost);
        }
        sync_shared_ui(&mut app.sim.world, &mut app.ui);
        assert!(app.ui.chat_is_ghost_channel);
        assert_eq!(app.ui.chat_entries.len(), 2);
    }
}
