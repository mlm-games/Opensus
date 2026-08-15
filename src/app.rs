use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use repose_bevy::{ReposePlugin, ReposePluginSettings};
use repose_core::{prelude::Modifier, remember};
use repose_ui::overlay::OverlayHandle;

use crate::asset_tracking::AssetsLoading;
use crate::dev_tools::DevToolsPlugin;
use crate::game::{GamePhase, GamePlugin, LobbySlot, MatchConfig, Role};
use crate::menus::{self, UiAction, UiBridge};
use crate::save::SaveData;
use crate::screens::ScreensPlugin;
use crate::theme::ThemePlugin;
use game_utils_bevy::{
    EcosystemPlugin,
    audio::AudioChannels,
    i18n::{self, I18nPlugin, LocaleResources},
    post_process::{ScreenEffectSettings, sync_post_process_settings},
    save::{SaveManager, SavePlugin},
    screen_effects::CameraBase,
    transitions::Transition,
};

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
    ("en", include_str!("../assets/locales/en/main.ftl")),
    ("es", include_str!("../assets/locales/es/main.ftl")),
    ("fr", include_str!("../assets/locales/fr/main.ftl")),
    ("de", include_str!("../assets/locales/de/main.ftl")),
    ("ja", include_str!("../assets/locales/ja/main.ftl")),
    ("zh", include_str!("../assets/locales/zh/main.ftl")),
    ("pt", include_str!("../assets/locales/pt/main.ftl")),
];

#[derive(States, Default, Clone, Eq, PartialEq, Debug, Hash)]
pub enum AppState {
    #[default]
    Splash,
    Loading,
    Title,
    Lobby,
    InGame,
}

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub struct Paused(pub bool);

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum OverlayMenu {
    #[default]
    None,
    Settings,
    Credits,
    Pause,
}

#[derive(Resource, Default)]
pub struct PendingUnpause(pub Option<Timer>);

/// Snapshot for Repose (clone every frame).
#[derive(Resource, Clone)]
pub struct SharedUi {
    pub phase: AppState,
    pub paused: bool,
    pub loading_progress: f32,
    pub overlay: OverlayMenu,
    pub master_vol: f32,
    pub sfx_vol: f32,
    pub music_vol: f32,
    pub transition_alpha: f32,
    pub flash_alpha: f32,
    pub language: String,
    pub saved_language: String,
    pub available_languages: Vec<String>,
    pub translations: HashMap<String, String>,
    // Opensus
    pub game_phase: GamePhase,
    pub lobby_slots: Vec<LobbySlot>,
    pub local_ready: bool,
    pub is_host: bool,
    pub my_role: Option<Role>,
    pub tasks_done: u32,
    pub tasks_total: u32,
    pub kill_cd: f32,
    pub phase_timer: f32,
    pub meeting_prompt: String,
    pub vote_options: Vec<(u64, String, bool)>, // id, name, dead
    pub my_voted: bool,
    pub result_text: String,
    pub player_name: String,
    pub color_index: u8,
    pub sabotage_kind: Option<String>,
    pub sabotage_remaining: f32,
    pub lights_out: bool,
}

impl Default for SharedUi {
    fn default() -> Self {
        Self {
            phase: AppState::Splash,
            paused: false,
            loading_progress: 0.0,
            overlay: OverlayMenu::None,
            master_vol: 1.0,
            sfx_vol: 1.0,
            music_vol: 0.8,
            transition_alpha: 0.0,
            flash_alpha: 0.0,
            language: "en".to_string(),
            saved_language: "en".to_string(),
            available_languages: vec!["en".to_string()],
            translations: HashMap::new(),
            game_phase: GamePhase::None,
            lobby_slots: Vec::new(),
            local_ready: false,
            is_host: true,
            my_role: None,
            tasks_done: 0,
            tasks_total: 0,
            kill_cd: 0.0,
            phase_timer: 0.0,
            meeting_prompt: String::new(),
            vote_options: Vec::new(),
            my_voted: false,
            result_text: String::new(),
            player_name: "Agent".to_string(),
            color_index: 0,
            sabotage_kind: None,
            sabotage_remaining: 0.0,
            lights_out: false,
        }
    }
}

pub struct AppPlugin;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        let shared = Arc::new(Mutex::new(SharedUi::default()));
        let actions = Arc::new(Mutex::new(Vec::<UiAction>::new()));
        let shared_ui = shared.clone();
        let actions_ui = actions.clone();

        app.init_state::<AppState>()
            .insert_resource(Paused(false))
            .insert_resource(OverlayMenu::None)
            .insert_resource(PendingUnpause(None))
            .insert_resource(UiBridge {
                shared: shared.clone(),
                actions: actions.clone(),
            })
            .add_plugins(ReposePlugin::with_settings(
                ReposePluginSettings {
                    clear_alpha: 0.0,
                    compose_every_frame: true,
                    msaa_samples: 1,
                    overlay: true,
                },
                move |_s, _c| {
                    let st = shared_ui.lock().unwrap().clone();
                    let acts = actions_ui.clone();
                    let overlay_rc = remember(OverlayHandle::new);
                    let overlay = (*overlay_rc).clone();
                    let root = menus::compose_root(overlay.clone(), st, acts);
                    overlay.host(Modifier::new().fill_max_size(), root)
                },
            ))
            .add_plugins((
                ThemePlugin,
                EcosystemPlugin::<AppState>::new(I18nPlugin::new(TRANSLATION_KEYS, LOCALES)),
                SavePlugin::<SaveData>::new(SaveManager::new(
                    "com",
                    "mlm-games",
                    "opensus",
                    "save.ron",
                    1,
                )),
                ScreensPlugin,
                GamePlugin,
                DevToolsPlugin,
            ))
            .add_systems(Startup, setup_camera)
            .add_systems(
                Update,
                (
                    apply_saved_settings,
                    sync_shared_ui,
                    sync_shared_game,
                    sync_post_process_settings::<AppState>,
                    process_ui_actions,
                    handle_pause_input,
                    tick_pending_unpause,
                    sync_virtual_time_with_pause,
                )
                    .chain(),
            );
    }
}

fn apply_saved_settings(save: Res<SaveData>, mut locale: ResMut<LocaleResources>) {
    if !save.is_added() && !save.is_changed() {
        return;
    }
    if locale
        .available
        .iter()
        .any(|l| l == &save.settings.language)
    {
        locale.set_locale(&save.settings.language);
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Transform::from_xyz(0.0, 0.0, 1000.0),
        CameraBase {
            translation: Vec3::new(0.0, 0.0, 1000.0),
            rotation: 0.0,
        },
        ScreenEffectSettings::default(),
    ));
}

fn sync_shared_ui(
    state: Res<State<AppState>>,
    paused: Res<Paused>,
    overlay: Res<OverlayMenu>,
    bridge: Res<UiBridge>,
    save: Res<SaveData>,
    transition: Res<Transition<AppState>>,
    flash: Res<game_utils_bevy::screen_effects::FlashWhite>,
    locale: Res<LocaleResources>,
    mut channels: ResMut<AudioChannels>,
    loading: Option<Res<AssetsLoading>>,
    asset_server: Res<AssetServer>,
) {
    let Ok(mut ui) = bridge.shared.lock() else {
        return;
    };
    ui.phase = state.get().clone();
    ui.paused = paused.0;
    ui.overlay = *overlay;
    ui.player_name = save.player_name.clone();
    ui.color_index = save.preferred_color_index;
    if *overlay != OverlayMenu::Settings {
        ui.master_vol = save.settings.master_volume;
        ui.sfx_vol = save.settings.sfx_volume;
        ui.music_vol = save.settings.music_volume;
    }
    ui.transition_alpha = transition.overlay_alpha;
    ui.flash_alpha = flash.amount;
    ui.language = locale.current.clone();
    ui.available_languages = locale.available.clone();
    ui.translations = i18n::get_current_translations(&locale);
    ui.loading_progress = match loading {
        Some(l) if !l.0.is_empty() => {
            l.0.iter()
                .filter(|h| asset_server.is_loaded_with_dependencies(h.id()))
                .count() as f32
                / l.0.len() as f32
        }
        _ => 1.0,
    };
    channels.master = save.settings.master_volume;
    channels.sfx = save.settings.sfx_volume;
    channels.music = save.settings.music_volume;
}

fn sync_shared_game(
    bridge: Res<UiBridge>,
    game_phase: Option<Res<GamePhase>>,
    lobby: Option<Res<crate::game::LobbyState>>,
    match_cfg: Option<Res<MatchConfig>>,
    tasks: Option<Res<crate::game::TaskBoard>>,
    kill_cd: Option<Res<crate::game::KillCooldown>>,
    meeting: Option<Res<crate::game::MeetingState>>,
    local_role: Option<Res<crate::game::LocalRole>>,
    sabotage: Option<Res<crate::game::ActiveSabotage>>,
) {
    let Ok(mut ui) = bridge.shared.lock() else {
        return;
    };
    ui.game_phase = game_phase.map(|g| *g).unwrap_or(GamePhase::None);
    if let Some(lobby) = lobby {
        ui.lobby_slots = lobby.slots.clone();
        ui.local_ready = lobby.local_ready;
        ui.is_host = lobby.is_host;
    }
    if let Some(tb) = tasks {
        ui.tasks_done = tb.completed;
        ui.tasks_total = tb.total;
    } else if let Some(cfg) = match_cfg {
        ui.tasks_total = cfg.tasks_to_win;
    }
    ui.kill_cd = kill_cd.map(|k| k.remaining).unwrap_or(0.0);
    ui.my_role = local_role.and_then(|r| r.0);
    if let Some(m) = meeting {
        ui.phase_timer = m.timer.remaining_secs().max(0.0);
        ui.meeting_prompt = m.prompt.clone();
        ui.vote_options = m
            .options
            .iter()
            .map(|o| (o.player_id, o.name.clone(), o.dead))
            .collect();
        ui.my_voted = m.local_voted;
        ui.result_text = m.result_text.clone();
    } else {
        ui.phase_timer = 0.0;
        ui.meeting_prompt.clear();
        ui.vote_options.clear();
        ui.my_voted = false;
        ui.result_text.clear();
    }
    if let Some(s) = sabotage {
        ui.sabotage_kind = s.kind.map(|k| format!("{k:?}"));
        ui.sabotage_remaining = s.critical_remaining();
        ui.lights_out = matches!(s.kind, Some(crate::game::SabotageKind::Lights));
    } else {
        ui.sabotage_kind = None;
        ui.sabotage_remaining = 0.0;
        ui.lights_out = false;
    }
}

fn tick_pending_unpause(
    real: Res<Time<Real>>,
    mut pending: ResMut<PendingUnpause>,
    mut paused: ResMut<Paused>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    let Some(timer) = pending.0.as_mut() else {
        return;
    };
    if timer.tick(real.delta()).just_finished() {
        pending.0 = None;
        paused.0 = false;
        virtual_time.unpause();
    }
}

fn set_vol(bridge: &UiBridge, field: impl Fn(&mut SharedUi) -> &mut f32, v: f32) {
    if let Ok(mut ui) = bridge.shared.lock() {
        *field(&mut ui) = v.clamp(0.0, 1.0);
    }
}

fn process_ui_actions(
    bridge: Res<UiBridge>,
    mut paused: ResMut<Paused>,
    mut overlay: ResMut<OverlayMenu>,
    mut save: ResMut<SaveData>,
    mut exit: MessageWriter<AppExit>,
    mut transition: ResMut<Transition<AppState>>,
    manager: Res<SaveManager>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut pending_unpause: ResMut<PendingUnpause>,
    mut locale: ResMut<LocaleResources>,
    mut lobby: Option<ResMut<crate::game::LobbyState>>,
    mut start_match: MessageWriter<crate::game::StartMatchRequest>,
    mut meeting_cmds: MessageWriter<crate::game::MeetingCommand>,
    mut game_phase: Option<ResMut<GamePhase>>,
    mut runtime_mode: ResMut<crate::game::RuntimeMode>,
    mut pending_network: ResMut<crate::game::PendingNetworkStart>,
) {
    let Ok(mut q) = bridge.actions.lock() else {
        return;
    };
    for action in q.drain(..) {
        match action {
            UiAction::HostLobby => {
                *runtime_mode = crate::game::RuntimeMode::Host;
                *pending_network = crate::game::PendingNetworkStart::HostLocal {
                    bind_addr: "127.0.0.1:5000".to_string(),
                };
                transition.begin_to_state(AppState::Loading);
            }
            UiAction::JoinLobby => {
                *runtime_mode = crate::game::RuntimeMode::Client;
                *pending_network = crate::game::PendingNetworkStart::JoinLocal {
                    server_addr: "127.0.0.1:5000".to_string(),
                };
                transition.begin_to_state(AppState::Loading);
            }
            UiAction::ToggleReady => {
                if let Some(ref mut lobby) = lobby {
                    lobby.local_ready = !lobby.local_ready;
                    let ready = lobby.local_ready;
                    if let Some(slot) = lobby.slots.iter_mut().find(|s| s.is_local) {
                        slot.ready = ready;
                    }
                }
            }
            UiAction::StartMatch => {
                start_match.write(crate::game::StartMatchRequest);
            }
            UiAction::LeaveLobby => {
                *pending_network = crate::game::PendingNetworkStart::None;
                *runtime_mode = crate::game::RuntimeMode::Local;
                transition.begin_to_state(AppState::Title);
            }
            UiAction::CallEmergency => {
                meeting_cmds.write(crate::game::MeetingCommand::Emergency);
            }
            UiAction::CastVote(id) => {
                meeting_cmds.write(crate::game::MeetingCommand::Vote(id));
            }
            UiAction::SkipVote => {
                meeting_cmds.write(crate::game::MeetingCommand::Skip);
            }
            UiAction::PlayAgain => {
                if let Some(ref mut gp) = game_phase {
                    **gp = GamePhase::None;
                }
                transition.begin_to_state(AppState::Lobby);
            }
            UiAction::CycleColor => {
                save.preferred_color_index =
                    (save.preferred_color_index + 1) % crate::game::PLAYER_COLORS.len() as u8;
                if let Some(ref mut lobby) = lobby
                    && let Some(slot) = lobby.slots.iter_mut().find(|s| s.is_local)
                {
                    slot.color_index = save.preferred_color_index;
                }
            }
            UiAction::OpenSettings => {
                if let Ok(mut ui) = bridge.shared.lock() {
                    ui.saved_language = locale.current.clone();
                }
                *overlay = OverlayMenu::Settings;
            }
            UiAction::TogglePause => {
                if !paused.0 {
                    paused.0 = true;
                    *overlay = OverlayMenu::Pause;
                    virtual_time.pause();
                    pending_unpause.0 = None;
                } else {
                    *overlay = OverlayMenu::None;
                    pending_unpause.0 = Some(Timer::from_seconds(0.2, TimerMode::Once));
                }
            }
            UiAction::OpenCredits => *overlay = OverlayMenu::Credits,
            UiAction::CloseOverlay => {
                if *overlay == OverlayMenu::Settings
                    && let Ok(ui) = bridge.shared.lock()
                {
                    locale.set_locale(&ui.saved_language);
                }
                match *overlay {
                    OverlayMenu::Settings | OverlayMenu::Credits if paused.0 => {
                        *overlay = OverlayMenu::Pause;
                    }
                    OverlayMenu::Pause if paused.0 => {
                        *overlay = OverlayMenu::None;
                        pending_unpause.0 = Some(Timer::from_seconds(0.2, TimerMode::Once));
                    }
                    _ => {
                        *overlay = OverlayMenu::None;
                    }
                }
            }
            UiAction::Resume => {
                *overlay = OverlayMenu::None;
                pending_unpause.0 = Some(Timer::from_seconds(0.2, TimerMode::Once));
            }
            UiAction::QuitToTitle => {
                *pending_network = crate::game::PendingNetworkStart::None;
                *runtime_mode = crate::game::RuntimeMode::Local;
                paused.0 = false;
                *overlay = OverlayMenu::None;
                pending_unpause.0 = None;
                virtual_time.unpause();
                transition.begin_to_state(AppState::Title);
            }
            UiAction::QuitApp => {
                exit.write(AppExit::Success);
            }
            UiAction::SetMasterVol(v) => set_vol(&bridge, |ui| &mut ui.master_vol, v),
            UiAction::SetSfxVol(v) => set_vol(&bridge, |ui| &mut ui.sfx_vol, v),
            UiAction::SetMusicVol(v) => set_vol(&bridge, |ui| &mut ui.music_vol, v),
            UiAction::SaveSettings => {
                if let Ok(ui) = bridge.shared.lock() {
                    save.settings.master_volume = ui.master_vol;
                    save.settings.sfx_volume = ui.sfx_vol;
                    save.settings.music_volume = ui.music_vol;
                    save.settings.language = locale.current.clone();
                }
                let _ = manager.save(&*save);
                if let Ok(mut ui) = bridge.shared.lock() {
                    ui.saved_language = locale.current.clone();
                }
                if paused.0 {
                    *overlay = OverlayMenu::Pause;
                } else {
                    *overlay = OverlayMenu::None;
                }
            }
            UiAction::SetLanguage(ref lang) => {
                if locale.available.contains(lang) {
                    locale.set_locale(lang);
                }
            }
        }
    }
}

fn handle_pause_input(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<AppState>>,
    mut paused: ResMut<Paused>,
    mut overlay: ResMut<OverlayMenu>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut pending_unpause: ResMut<PendingUnpause>,
    transition: Res<Transition<AppState>>,
    game_phase: Option<Res<GamePhase>>,
) {
    if *state.get() != AppState::InGame {
        return;
    }
    let _ = game_phase;
    if transition.block_input {
        return;
    }
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    match *overlay {
        OverlayMenu::None if !paused.0 => {
            paused.0 = true;
            *overlay = OverlayMenu::Pause;
            virtual_time.pause();
            pending_unpause.0 = None;
        }
        OverlayMenu::Pause => {
            *overlay = OverlayMenu::None;
            pending_unpause.0 = Some(Timer::from_seconds(0.2, TimerMode::Once));
        }
        OverlayMenu::Settings | OverlayMenu::Credits => {
            if paused.0 {
                *overlay = OverlayMenu::Pause;
            } else {
                *overlay = OverlayMenu::None;
            }
        }
        _ => {}
    }
}

fn sync_virtual_time_with_pause(paused: Res<Paused>, mut virtual_time: ResMut<Time<Virtual>>) {
    if paused.0 {
        if !virtual_time.is_paused() {
            virtual_time.pause();
        }
    } else if virtual_time.is_paused() {
        virtual_time.unpause();
    }
}
