use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bevy_ecs::prelude::*;
use game_utils::i18n;
use game_utils_repame::I18nStrings;
use repame_fx::{Flash, TransitionFx};
use repose_core::ImageHandle;

use crate::app::{AppState, LOADING_SECS, LoadingTimer, OverlayMenu, Paused};
use crate::game::{
    ActiveSabotage, Alive, EmergenciesLeft, GamePhase, KillCooldownLeft, LobbySlot, LobbyState,
    LocalPlayer, LocalPrompt, LocalRole, MatchConfig, MeetingState, Player, Role, RoleRevealTimer,
    SabotageKind, TaskBoard,
};
use crate::save::SaveData;
use crate::ui::menus::UiAction;

/// Snapshot for Repose (clone every frame).
#[derive(Clone)]
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
    pub game_phase: GamePhase,
    pub lobby_slots: Vec<LobbySlot>,
    pub local_ready: bool,
    pub is_host: bool,
    pub my_role: Option<Role>,
    pub tasks_done: u32,
    pub tasks_total: u32,
    pub kill_cd: f32,
    pub emergencies_left: u32,
    pub phase_timer: f32,
    pub meeting_prompt: String,
    pub vote_options: Vec<(u64, String, bool)>,
    pub my_voted: bool,
    pub result_text: String,
    pub vote_tallies: Vec<(String, u32)>,
    pub player_name: String,
    pub color_index: u8,
    pub sabotage_kind: Option<String>,
    pub sabotage_remaining: f32,
    pub sabotage_cooldown: f32,
    pub interact_prompt: String,
    pub lights_out: bool,
    pub local_alive: bool,
    pub local_player_id: Option<u64>,
    pub chat_entries: Vec<(String, String, bool)>,
    pub chat_buffer: String,
    pub chat_is_ghost_channel: bool,
    pub ui_lab_bg: Option<ImageHandle>,
    pub ui_background: Option<ImageHandle>,
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
            emergencies_left: 0,
            phase_timer: 0.0,
            meeting_prompt: String::new(),
            vote_options: Vec::new(),
            my_voted: false,
            result_text: String::new(),
            vote_tallies: Vec::new(),
            player_name: "Agent".to_string(),
            color_index: 0,
            sabotage_kind: None,
            sabotage_remaining: 0.0,
            sabotage_cooldown: 0.0,
            interact_prompt: String::new(),
            lights_out: false,
            local_alive: true,
            local_player_id: None,
            chat_entries: Vec::new(),
            chat_buffer: String::new(),
            chat_is_ghost_channel: false,
            ui_lab_bg: None,
            ui_background: None,
        }
    }
}

#[derive(Resource, Clone, Default)]
pub struct UiActions(pub Arc<Mutex<Vec<UiAction>>>);

pub fn drain_actions(world: &mut World) -> Vec<UiAction> {
    let Some(actions) = world.get_resource_mut::<UiActions>() else {
        return Vec::new();
    };
    actions
        .0
        .lock()
        .map(|mut queue| queue.drain(..).collect())
        .unwrap_or_default()
}

pub fn sync_shared_ui(world: &mut World, ui: &mut SharedUi) {
    ui.phase = *world.resource::<AppState>();
    ui.paused = world.resource::<Paused>().0;
    ui.overlay = *world.resource::<OverlayMenu>();
    let save = world.resource::<SaveData>();
    ui.player_name = save.player_name.clone();
    ui.color_index = save.preferred_color_index;
    if ui.overlay != OverlayMenu::Settings {
        ui.master_vol = save.settings.master_volume;
        ui.sfx_vol = save.settings.sfx_volume;
        ui.music_vol = save.settings.music_volume;
    }
    ui.transition_alpha = world.resource::<TransitionFx>().alpha();
    ui.flash_alpha = world
        .resource::<Flash>()
        .rgba()
        .map(|rgba| rgba[3])
        .unwrap_or(0.0);
    let locale = &world.resource::<I18nStrings>().0;
    ui.language = locale.current.clone();
    ui.available_languages = locale.available.clone();
    ui.translations = i18n::get_current_translations(locale);
    ui.loading_progress = if ui.phase == AppState::Loading {
        world
            .get_resource::<LoadingTimer>()
            .map(|timer| 1.0 - (timer.0 / LOADING_SECS).clamp(0.0, 1.0))
            .unwrap_or(1.0)
    } else {
        1.0
    };
    ui.game_phase = *world.resource::<GamePhase>();
    let lobby = world.resource::<LobbyState>();
    ui.lobby_slots = lobby.slots.clone();
    ui.local_ready = lobby.local_ready;
    ui.is_host = lobby.is_host;
    sync_game_fields(world, ui);
}

fn sync_game_fields(world: &mut World, ui: &mut SharedUi) {
    if ui.phase != AppState::InGame {
        ui.my_role = None;
        ui.tasks_done = 0;
        ui.tasks_total = 0;
        ui.kill_cd = 0.0;
        ui.emergencies_left = 0;
        ui.phase_timer = 0.0;
        ui.meeting_prompt.clear();
        ui.vote_options.clear();
        ui.my_voted = false;
        ui.result_text.clear();
        ui.vote_tallies.clear();
        ui.sabotage_kind = None;
        ui.sabotage_remaining = 0.0;
        ui.sabotage_cooldown = 0.0;
        ui.interact_prompt.clear();
        ui.lights_out = false;
        ui.local_alive = false;
        ui.local_player_id = None;
        return;
    }

    ui.my_role = world.resource::<LocalRole>().0;
    {
        let board = world.resource::<TaskBoard>();
        ui.tasks_done = board.completed;
        ui.tasks_total = board.total;
    }
    {
        let meeting = world.resource::<MeetingState>();
        ui.meeting_prompt = meeting.prompt.clone();
        ui.vote_options = meeting
            .options
            .iter()
            .map(|option| (option.player_id, option.name.clone(), option.dead))
            .collect();
        ui.my_voted = meeting.local_voted;
        ui.result_text = meeting.result_text.clone();
        ui.vote_tallies = meeting.tallies.clone();
        ui.phase_timer = match ui.game_phase {
            GamePhase::Meeting | GamePhase::Voting | GamePhase::Results => {
                meeting.timer.remaining_secs()
            }
            _ => 0.0,
        };
    }
    if matches!(ui.game_phase, GamePhase::RoleReveal) {
        ui.phase_timer = world.resource::<RoleRevealTimer>().0.remaining_secs();
    }
    ui.interact_prompt = world.resource::<LocalPrompt>().0.clone();
    {
        let sabotage = world.resource::<ActiveSabotage>();
        ui.sabotage_kind = sabotage.kind.map(|kind| match kind {
            SabotageKind::Lights => "Lights".to_string(),
            SabotageKind::Oxygen => "Oxygen".to_string(),
            SabotageKind::Reactor => "Reactor".to_string(),
        });
        ui.sabotage_remaining = sabotage.critical_remaining();
        ui.lights_out = sabotage.kind == Some(SabotageKind::Lights);
    }
    ui.sabotage_cooldown = world.resource::<MatchConfig>().sabotage_cooldown;
    let mut query = world.query_filtered::<(
        &Player,
        Option<&KillCooldownLeft>,
        Option<&EmergenciesLeft>,
        Option<&Alive>,
    ), With<LocalPlayer>>();
    if let Ok((player, kill_cd, emergencies, alive)) = query.single(world) {
        ui.local_player_id = Some(player.id);
        ui.kill_cd = kill_cd.map(|cd| cd.0).unwrap_or(0.0);
        ui.emergencies_left = emergencies.map(|left| left.0 as u32).unwrap_or(0);
        ui.local_alive = alive.is_some();
    } else {
        ui.local_player_id = None;
        ui.kill_cd = 0.0;
        ui.emergencies_left = 0;
        ui.local_alive = false;
    }
}
