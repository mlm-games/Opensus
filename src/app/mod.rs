use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use game_utils_repame::{I18nStrings, SaveResource, register_i18n, register_save};
use repame_fx::TransitionFx;
use repame_shell::{
    SharedEdges, Staging, install_game_shortcuts, shared_edges, take_shortcut_edges,
};
use repame_sim::Sim;
use repose_core::prelude::Modifier;
use repose_core::{FocusRequester, RenderContext, Scheduler, View, remember, request_frame};
use repose_ui::overlay::OverlayHandle;
use repose_ui::{ViewExt, ZStack};
use web_time::{Duration, Instant};

use crate::game::{
    GamePhase, LobbyState, MatchConfig, MeetingCommand, MeetingCommands, PLAYER_COLORS,
    PendingNetworkStart, RuntimeMode, handle_start_match, setup_lobby,
};
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
        AppState::Title | AppState::InGame => {}
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
    shortcut_edges: SharedEdges,
    ui: SharedUi,
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
        sim.world.init_resource::<MatchConfig>();
        sim.world.init_resource::<LobbyState>();
        sim.world.init_resource::<GamePhase>();
        sim.world.init_resource::<MeetingCommands>();
        sim.world.insert_resource(QuitRequested::default());
        sim.world.insert_resource(AppState::Splash);
        sim.world.insert_resource(SplashTimer(SPLASH_SECS));
        sim.world.insert_resource(Paused::default());
        sim.world.insert_resource(OverlayMenu::None);
        sim.world.insert_resource(PendingUnpause::default());
        sim.world.insert_resource(InputEdges::default());
        Self {
            sim,
            schedule: Schedule::default(),
            pending_state: None,
            staging: Staging::shared(),
            shortcut_edges: shared_edges(),
            ui: SharedUi::default(),
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
        self.sim.step_with(dt, move |world| schedule.run(world))
    }

    fn feed_polled(&mut self, sched: &Scheduler) {
        self.staging.borrow_mut().feed_polled(sched);
    }

    fn feed_input(&mut self) {
        self.staging.borrow_mut().take_edges();
        let (pause, _, _) = take_shortcut_edges(&self.shortcut_edges);
        let blocked = self.sim.world.resource::<TransitionFx>().blocking();
        self.sim.world.insert_resource(InputEdges {
            pause: pause && !blocked,
        });
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

    pub fn view(&mut self, sched: &mut Scheduler, _ctx: &RenderContext, dt: Duration) -> View {
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
        self.advance(dt);
        if self
            .sim
            .world
            .get_resource::<QuitRequested>()
            .is_some_and(|quit| quit.0)
        {
            std::process::exit(0);
        }
        sync_shared_ui(&self.sim.world, &mut self.ui);
        let overlay_rc = remember(OverlayHandle::new);
        let overlay = (*overlay_rc).clone();
        let focus = remember(FocusRequester::new);
        let focus_positioned = (*focus).clone();
        let focus_staging = self.staging.clone();
        let key_staging = self.staging.clone();
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
        app.sim.world.insert_resource(InputEdges { pause: true });
        app.advance(Duration::from_millis(16));
        assert_eq!(*app.sim.world.resource::<OverlayMenu>(), OverlayMenu::Pause);
        assert!(app.sim.world.resource::<Paused>().0);

        app.sim.world.insert_resource(InputEdges { pause: true });
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
        app.sim.world.insert_resource(InputEdges { pause: true });
        app.advance(Duration::from_millis(16));
        assert!(!app.sim.world.resource::<Paused>().0);
        assert!(!app.sim.world.resource::<InputEdges>().pause);

        goto_state(&mut app.sim.world, AppState::InGame);
        app.begin_to_state(AppState::Title);
        app.shortcut_edges.borrow_mut().pause = true;
        app.feed_input();
        assert!(!app.sim.world.resource::<InputEdges>().pause);
        app.sim.world.insert_resource(InputEdges { pause: true });
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

        app.sim.world.insert_resource(InputEdges { pause: true });
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

        app.sim.world.insert_resource(InputEdges { pause: true });
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
}
