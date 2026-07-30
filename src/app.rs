use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use repose_bevy::{ReposePlugin, ReposePluginSettings};
use repose_core::{prelude::Modifier, remember};
use repose_ui::overlay::OverlayHandle;

use crate::demo::DemoPlugin;
use crate::dev_tools::DevToolsPlugin;
use crate::ecosystem::{
    EcosystemPlugin,
    audio::AudioChannels,
    i18n::{self, LocaleResources},
    post_process::{sync_post_process_settings, ScreenEffectSettings},
    transitions::Transition,
};
use crate::menus::{self, UiAction, UiBridge};
use crate::screens::ScreensPlugin;
use crate::theme::ThemePlugin;

#[derive(States, Default, Clone, Eq, PartialEq, Debug, Hash)]
pub enum AppState {
    #[default]
    Splash,
    Loading,
    Title,
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

#[derive(Resource, Clone)]
pub struct SharedUi {
    pub phase: AppState,
    pub paused: bool,
    pub overlay: OverlayMenu,
    pub master_vol: f32,
    pub sfx_vol: f32,
    pub music_vol: f32,
    pub high_score: u32,
    pub score: u32,
    pub transition_alpha: f32,
    pub flash_alpha: f32,
    pub language: String,
    pub saved_language: String,
    pub available_languages: Vec<String>,
    pub translations: HashMap<String, String>,
}

impl Default for SharedUi {
    fn default() -> Self {
        Self {
            phase: AppState::Splash,
            paused: false,
            overlay: OverlayMenu::None,
            master_vol: 1.0,
            sfx_vol: 1.0,
            music_vol: 0.8,
            high_score: 0,
            score: 0,
            transition_alpha: 0.0,
            flash_alpha: 0.0,
            language: "en".to_string(),
            saved_language: "en".to_string(),
            available_languages: vec!["en".to_string()],
            translations: HashMap::new(),
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
                EcosystemPlugin,
                ScreensPlugin,
                DemoPlugin,
                DevToolsPlugin,
            ))
            .add_systems(Startup, setup_camera)
            .add_systems(
                Update,
                (
                    sync_shared_ui,
                    sync_post_process_settings,
                    process_ui_actions,
                    handle_pause_input,
                    tick_pending_unpause,
                    sync_virtual_time_with_pause,
                )
                    .chain(),
            );
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Transform::from_xyz(0.0, 0.0, 1000.0),
        crate::ecosystem::screen_effects::CameraBase {
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
    save: Res<crate::ecosystem::save::SaveData>,
    score: Option<Res<crate::demo::Score>>,
    transition: Res<Transition>,
    flash: Res<crate::ecosystem::screen_effects::FlashWhite>,
    locale: Res<LocaleResources>,
    mut channels: ResMut<AudioChannels>,
) {
    let Ok(mut ui) = bridge.shared.lock() else {
        return;
    };
    ui.phase = state.get().clone();
    ui.paused = paused.0;
    ui.overlay = *overlay;
    ui.high_score = save.high_score;
    ui.score = score.map(|s| s.0).unwrap_or(0);
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
    channels.master = save.settings.master_volume;
    channels.sfx = save.settings.sfx_volume;
    channels.music = save.settings.music_volume;
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
    mut save: ResMut<crate::ecosystem::save::SaveData>,
    mut exit: MessageWriter<AppExit>,
    mut transition: ResMut<crate::ecosystem::transitions::Transition>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut pending_unpause: ResMut<PendingUnpause>,
    mut locale: ResMut<LocaleResources>,
) {
    let Ok(mut q) = bridge.actions.lock() else {
        return;
    };
    for action in q.drain(..) {
        match action {
            UiAction::StartGame => {
                transition.begin_to_state(AppState::InGame);
            }
            UiAction::OpenSettings => {
                if let Ok(mut ui) = bridge.shared.lock() {
                    ui.saved_language = locale.current.clone();
                }
                *overlay = OverlayMenu::Settings;
            }
            UiAction::OpenCredits => *overlay = OverlayMenu::Credits,
            UiAction::CloseOverlay => {
                if *overlay == OverlayMenu::Settings {
                    if let Ok(ui) = bridge.shared.lock() {
                        locale.set_locale(&ui.saved_language);
                    }
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
                let _ = crate::ecosystem::save::SaveManager::save(&save);
                if let Ok(mut ui) = bridge.shared.lock() {
                    ui.saved_language = locale.current.clone();
                }
                if paused.0 {
                    *overlay = OverlayMenu::Pause;
                } else {
                    *overlay = OverlayMenu::None;
                }
            }
            UiAction::NextLanguage => {
                let available = locale.available.clone();
                let current = locale.current.clone();
                let idx = available.iter().position(|l| *l == current).unwrap_or(0);
                let next = (idx + 1) % available.len();
                if let Some(next_locale) = available.get(next) {
                    locale.set_locale(next_locale);
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
    transition: Res<Transition>,
) {
    if *state.get() != AppState::InGame {
        return;
    }
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
