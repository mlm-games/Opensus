use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use repose_core::View;
use repose_core::prelude::{
    AlignItems, AlignSelf, AnimationSpec, Color as RColor, Easing, JustifyContent, Modifier,
    remember,
};
use repose_material::material3::{
    ButtonConfig, DropdownMenu, DropdownMenuConfig, DropdownMenuEntry, DropdownMenuItem,
    FilledTonalButton, MenuState,
};
use repose_ui::anim_ext::{
    AnimatedVisibility, AnimatedVisibilityConfig, EnterTransition, ExitTransition,
};
use repose_ui::overlay::OverlayHandle;
use repose_ui::{Column, Row, Text as RText, TextStyle, ViewExt, ZStack};

use crate::app::{AppState, OverlayMenu, SharedUi};
use crate::game::{GamePhase, Role};

fn t(translations: &HashMap<String, String>, key: &str, fallback: &str) -> String {
    translations
        .get(key)
        .cloned()
        .unwrap_or_else(|| fallback.to_string())
}

#[derive(Clone, Debug)]
pub enum UiAction {
    #[expect(dead_code)]
    StartGame,
    HostLobby,
    JoinLobby,
    ToggleReady,
    StartMatch,
    LeaveLobby,
    CallEmergency,
    CastVote(u64),
    SkipVote,
    PlayAgain,
    #[expect(dead_code)]
    SetPlayerName(String),
    CycleColor,
    OpenSettings,
    OpenCredits,
    CloseOverlay,
    Resume,
    QuitToTitle,
    QuitApp,
    SetMasterVol(f32),
    SetSfxVol(f32),
    SetMusicVol(f32),
    SaveSettings,
    #[expect(dead_code)]
    NextLanguage,
    SetLanguage(String),
}

#[derive(bevy::prelude::Resource, Clone)]
pub struct UiBridge {
    pub shared: Arc<Mutex<SharedUi>>,
    pub actions: Arc<Mutex<Vec<UiAction>>>,
}

fn spacer(h: f32) -> View {
    Column(Modifier::new().height(h).width(1.0))
}

fn popup_anim_config(key: &str) -> AnimatedVisibilityConfig {
    AnimatedVisibilityConfig {
        key: key.into(),
        spec: AnimationSpec::tween(Duration::from_millis(200), Easing::EaseOut),
        enter: EnterTransition::ScaleIn { initial: 0.95 },
        exit: ExitTransition::ScaleOut { target: 0.95 },
    }
}

pub fn compose_root(
    overlay: OverlayHandle,
    st: SharedUi,
    actions: Arc<Mutex<Vec<UiAction>>>,
) -> View {
    let root = ZStack(Modifier::new().fill_max_size());
    let settings_view = settings_ui(overlay.clone(), &st, actions.clone());

    let content = match st.phase {
        AppState::Splash => splash_ui(),
        AppState::Loading => loading_ui(&st),
        AppState::Title => ZStack(Modifier::new().fill_max_size()).child((
            title_ui(&st, actions.clone()),
            AnimatedVisibility(
                st.overlay == OverlayMenu::Settings,
                settings_view.clone(),
                popup_anim_config("title_settings"),
            ),
            AnimatedVisibility(
                st.overlay == OverlayMenu::Credits,
                credits_ui(&st, actions.clone()),
                popup_anim_config("title_credits"),
            ),
        )),
        AppState::Lobby => ZStack(Modifier::new().fill_max_size()).child((
            lobby_ui(&st, actions.clone()),
            AnimatedVisibility(
                st.overlay == OverlayMenu::Settings,
                settings_view.clone(),
                popup_anim_config("lobby_settings"),
            ),
        )),
        AppState::InGame => {
            // Lights sabotage: dim the world (simple v1; replace with a
            // radial-hole shader when you add real vision/FOW).
            let darkness = if st.lights_out {
                Column(
                    Modifier::new()
                        .fill_max_size()
                        .background(RColor::from_rgba(0, 0, 5, 170)),
                )
            } else {
                spacer(1.0)
            };
            let hud = ingame_hud(&st, actions.clone());
            let meeting = meeting_overlay(&st, actions.clone());
            let gameover = gameover_overlay(&st, actions.clone());
            ZStack(Modifier::new().fill_max_size()).child((
                darkness,
                hud,
                AnimatedVisibility(
                    matches!(
                        st.game_phase,
                        GamePhase::Meeting | GamePhase::Voting | GamePhase::Results
                    ),
                    meeting,
                    popup_anim_config("meeting"),
                ),
                AnimatedVisibility(
                    matches!(st.game_phase, GamePhase::GameOver { .. }),
                    gameover,
                    popup_anim_config("gameover"),
                ),
                AnimatedVisibility(
                    st.overlay == OverlayMenu::Pause,
                    pause_overlay(&st, actions.clone()),
                    popup_anim_config("pause"),
                ),
                AnimatedVisibility(
                    st.overlay == OverlayMenu::Settings,
                    settings_view.clone(),
                    popup_anim_config("ingame_settings"),
                ),
                AnimatedVisibility(
                    st.overlay == OverlayMenu::Credits,
                    credits_ui(&st, actions.clone()),
                    popup_anim_config("ingame_credits"),
                ),
            ))
        }
    };

    if st.transition_alpha > 0.001 || st.flash_alpha > 0.001 {
        let fade_a = (st.transition_alpha.clamp(0.0, 1.0) * 255.0) as u8;
        let flash_a = (st.flash_alpha.clamp(0.0, 1.0) * 255.0) as u8;
        root.child((
            content,
            Column(
                Modifier::new()
                    .fill_max_size()
                    .background(RColor::from_rgba(0, 0, 0, fade_a)),
            ),
            Column(
                Modifier::new()
                    .fill_max_size()
                    .background(RColor::from_rgba(flash_a, flash_a, flash_a, flash_a)),
            ),
        ))
    } else {
        root.child(content)
    }
}

fn splash_ui() -> View {
    Column(
        Modifier::new()
            .fill_max_size()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER)
            .background(col(8, 12, 16)),
    )
    .child(RText("Opensus").size(48.0).color(RColor::WHITE))
}

fn loading_ui(st: &SharedUi) -> View {
    let pct = st.loading_progress.clamp(0.0, 1.0);
    Column(
        Modifier::new()
            .fill_max_size()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER)
            .background(col(8, 12, 16)),
    )
    .child(
        RText(t(&st.translations, "loading", "Loading..."))
            .size(32.0)
            .color(RColor::WHITE),
    )
    .child(spacer(16.0))
    .child(
        RText(format!("{:.0}%", pct * 100.0))
            .size(18.0)
            .color(RColor::WHITE),
    )
    .child(spacer(12.0))
    .child(
        Column(
            Modifier::new()
                .width(320.0)
                .height(12.0)
                .background(col(30, 30, 38))
                .clip_rounded(6.0),
        )
        .child(Column(
            Modifier::new()
                .width((320.0 * pct).max(1.0))
                .height(12.0)
                .background(col(160, 50, 50))
                .clip_rounded(6.0)
                .align_self(AlignSelf::FLEX_START),
        )),
    )
}

fn title_ui(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let a1 = actions.clone();
    let a2 = actions.clone();
    let a3 = actions.clone();
    let a4 = actions.clone();
    let a5 = actions.clone();
    let tr = &st.translations;

    Column(
        Modifier::new()
            .fill_max_size()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER)
            .background(col(8, 12, 16)),
    )
    .child(
        RText(t(tr, "app-title", "Opensus"))
            .size(56.0)
            .color(RColor::WHITE),
    )
    .child(spacer(8.0))
    .child(
        RText("One amongst us is not like the rest")
            .size(16.0)
            .color(col(180, 180, 190)),
    )
    .child(spacer(24.0))
    .child(mk_button(
        &t(tr, "host-game", "Host Game"),
        col(120, 40, 40),
        move || push(&a1, UiAction::HostLobby),
    ))
    .child(mk_button(
        &t(tr, "join-game", "Join Game (local)"),
        col(60, 80, 120),
        move || push(&a5, UiAction::JoinLobby),
    ))
    .child(mk_button(
        &t(tr, "settings", "Settings"),
        col(70, 70, 90),
        move || push(&a2, UiAction::OpenSettings),
    ))
    .child(mk_button(
        &t(tr, "credits", "Credits"),
        col(70, 70, 90),
        move || push(&a3, UiAction::OpenCredits),
    ))
    .child(mk_button(
        &t(tr, "quit", "Quit"),
        col(180, 60, 60),
        move || push(&a4, UiAction::QuitApp),
    ))
}

fn lobby_ui(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let tr = &st.translations;
    let a_ready = actions.clone();
    let a_start = actions.clone();
    let a_leave = actions.clone();
    let a_color = actions.clone();
    let a_set = actions.clone();

    let mut list = Column(Modifier::new().gap(6.0).align_items(AlignItems::FLEX_START));
    for s in &st.lobby_slots {
        let mark = if s.ready { "[R]" } else { "[ ]" };
        let host = if s.is_host { " (host)" } else { "" };
        let you = if s.is_local { " *" } else { "" };
        list = list.child(
            RText(format!("{mark} {}{}{}", s.name, host, you))
                .size(18.0)
                .color(RColor::WHITE),
        );
    }

    Column(
        Modifier::new()
            .fill_max_size()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER)
            .background(col(8, 12, 16)),
    )
    .child(
        RText(t(tr, "lobby-waiting", "Lobby"))
            .size(40.0)
            .color(RColor::WHITE),
    )
    .child(spacer(8.0))
    .child(
        RText(format!("{}: {}", t(tr, "name", "Name"), st.player_name))
            .size(16.0)
            .color(col(200, 200, 200)),
    )
    .child(mk_button(
        &t(tr, "color", "Cycle Color"),
        col(70, 70, 90),
        move || push(&a_color, UiAction::CycleColor),
    ))
    .child(spacer(12.0))
    .child(
        RText(t(tr, "players", "Players"))
            .size(22.0)
            .color(RColor::WHITE),
    )
    .child(list)
    .child(spacer(16.0))
    .child(mk_button(
        &if st.local_ready {
            t(tr, "unready", "Unready")
        } else {
            t(tr, "ready", "Ready")
        },
        col(60, 120, 80),
        move || push(&a_ready, UiAction::ToggleReady),
    ))
    .child(if st.is_host {
        mk_button(
            &t(tr, "start-match", "Start Match"),
            col(140, 50, 50),
            move || push(&a_start, UiAction::StartMatch),
        )
    } else {
        spacer(1.0)
    })
    .child(mk_button(
        &t(tr, "settings", "Settings"),
        col(70, 70, 90),
        move || push(&a_set, UiAction::OpenSettings),
    ))
    .child(mk_button(
        &t(tr, "leave-lobby", "Leave"),
        col(100, 60, 60),
        move || push(&a_leave, UiAction::LeaveLobby),
    ))
}

fn ingame_hud(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let tr = &st.translations;
    let role_str = match st.my_role {
        Some(Role::Impostor) => t(tr, "impostor", "Impostor"),
        Some(Role::Crewmate) => t(tr, "crewmate", "Crewmate"),
        None => "—".into(),
    };
    let a_em = actions.clone();

    Column(
        Modifier::new()
            .fill_max_size()
            .padding(16.0)
            .align_items(AlignItems::FLEX_START)
            .justify_content(JustifyContent::FLEX_START),
    )
    .child((
        RText(format!("{}: {}", t(tr, "you-are", "You are"), role_str))
            .size(20.0)
            .color(if matches!(st.my_role, Some(Role::Impostor)) {
                col(220, 80, 80)
            } else {
                RColor::WHITE
            }),
        RText(format!(
            "{}: {}/{}",
            t(tr, "tasks-remaining", "Tasks"),
            st.tasks_done,
            st.tasks_total
        ))
        .size(16.0)
        .color(col(180, 220, 180)),
        if matches!(st.my_role, Some(Role::Impostor)) {
            RText(format!(
                "{}: {:.0}s  (Q)",
                t(tr, "kill-cooldown", "Kill CD"),
                st.kill_cd
            ))
            .size(16.0)
            .color(col(220, 160, 160))
        } else {
            spacer(1.0)
        },
        if let Some(kind) = &st.sabotage_kind {
            let time_part = if st.sabotage_remaining > 0.0 {
                format!(" — {:.0}s", st.sabotage_remaining)
            } else {
                String::new()
            };
            RText(format!("⚠ SABOTAGE: {kind}{time_part} (hold E at station)"))
                .size(16.0)
                .color(col(235, 160, 40))
        } else {
            spacer(1.0)
        },
        RText(t(
            tr,
            "controls-hint",
            "WASD move | E task/fix | Q kill | R report | F emergency | 1/2/3 sabotage | Esc pause",
        ))
        .size(13.0)
        .color(col(160, 160, 170)),
        spacer(8.0),
        mk_button_sm("!", move || push(&a_em, UiAction::CallEmergency)),
    ))
}

fn meeting_overlay(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let tr = &st.translations;
    let phase_label = match st.game_phase {
        GamePhase::Meeting => t(tr, "discussion", "Discussion"),
        GamePhase::Voting => t(tr, "voting", "Voting"),
        GamePhase::Results => t(tr, "ejected", "Results"),
        _ => String::new(),
    };

    let mut votes = Column(Modifier::new().gap(6.0));
    if matches!(st.game_phase, GamePhase::Voting) && !st.my_voted {
        for (id, name, dead) in &st.vote_options {
            if *dead {
                continue;
            }
            let a = actions.clone();
            let id = *id;
            votes = votes.child(mk_button(
                &format!("{} {}", t(tr, "vote", "Vote"), name),
                col(70, 70, 100),
                move || push(&a, UiAction::CastVote(id)),
            ));
        }
        let a_skip = actions.clone();
        votes = votes.child(mk_button(
            &t(tr, "skip", "Skip"),
            col(90, 90, 90),
            move || push(&a_skip, UiAction::SkipVote),
        ));
    }

    Column(
        Modifier::new()
            .fill_max_size()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER)
            .background(RColor::from_rgba(0, 0, 0, 200)),
    )
    .child(
        Column(
            Modifier::new()
                .width(420.0)
                .padding(24.0)
                .background(col(24, 24, 32))
                .clip_rounded(12.0)
                .align_items(AlignItems::CENTER),
        )
        .child((
            RText(t(tr, "emergency-meeting", "Meeting"))
                .size(32.0)
                .color(RColor::WHITE),
            RText(phase_label).size(18.0).color(col(200, 180, 180)),
            RText(format!("{:.0}s", st.phase_timer))
                .size(16.0)
                .color(col(180, 180, 180)),
            spacer(8.0),
            RText(st.meeting_prompt.clone())
                .size(18.0)
                .color(RColor::WHITE),
            spacer(8.0),
            RText(st.result_text.clone())
                .size(18.0)
                .color(col(220, 200, 120)),
            votes,
        )),
    )
}

fn gameover_overlay(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let tr = &st.translations;
    let (msg, color) = match st.game_phase {
        GamePhase::GameOver { crew_win: true } => {
            (t(tr, "crewmates-win", "Crewmates win!"), col(80, 180, 100))
        }
        GamePhase::GameOver { crew_win: false } => {
            (t(tr, "impostors-win", "Impostors win!"), col(200, 70, 70))
        }
        _ => (String::new(), RColor::WHITE),
    };
    let a = actions.clone();
    let a2 = actions.clone();

    Column(
        Modifier::new()
            .fill_max_size()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER)
            .background(RColor::from_rgba(0, 0, 0, 210)),
    )
    .child(
        Column(
            Modifier::new()
                .width(400.0)
                .padding(28.0)
                .background(col(20, 20, 28))
                .clip_rounded(12.0)
                .align_items(AlignItems::CENTER),
        )
        .child((
            RText(msg).size(36.0).color(color),
            spacer(16.0),
            mk_button(
                &t(tr, "play-again", "Play Again"),
                col(120, 50, 50),
                move || push(&a, UiAction::PlayAgain),
            ),
            mk_button(
                &t(tr, "quit-to-title", "Quit to Title"),
                col(70, 70, 90),
                move || push(&a2, UiAction::QuitToTitle),
            ),
        )),
    )
}

fn pause_overlay(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let a1 = actions.clone();
    let a2 = actions.clone();
    let a3 = actions.clone();
    let tr = &st.translations;

    Column(
        Modifier::new()
            .fill_max_size()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER)
            .background(RColor::from_rgba(0, 0, 0, 180)),
    )
    .child(pause_panel(tr, a1, a2, a3))
}

fn pause_panel(
    tr: &HashMap<String, String>,
    a1: Arc<Mutex<Vec<UiAction>>>,
    a2: Arc<Mutex<Vec<UiAction>>>,
    a3: Arc<Mutex<Vec<UiAction>>>,
) -> View {
    Column(
        Modifier::new()
            .width(320.0)
            .padding(24.0)
            .background(col(20, 20, 28))
            .clip_rounded(12.0)
            .align_items(AlignItems::CENTER),
    )
    .child((
        RText(t(tr, "paused", "Paused"))
            .size(36.0)
            .color(RColor::WHITE),
        spacer(16.0),
        mk_button(&t(tr, "resume", "Resume"), col(60, 140, 90), move || {
            push(&a1, UiAction::Resume)
        }),
        mk_button(&t(tr, "settings", "Settings"), col(70, 70, 90), move || {
            push(&a2, UiAction::OpenSettings)
        }),
        mk_button(
            &t(tr, "quit-to-title", "Quit to Title"),
            col(180, 60, 60),
            move || push(&a3, UiAction::QuitToTitle),
        ),
    ))
}

fn settings_ui(overlay: OverlayHandle, st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let a_m_down = actions.clone();
    let a_m_up = actions.clone();
    let a_s_down = actions.clone();
    let a_s_up = actions.clone();
    let a_mu_down = actions.clone();
    let a_mu_up = actions.clone();
    let a_save = actions.clone();
    let a_back = actions.clone();
    let master = st.master_vol;
    let sfx = st.sfx_vol;
    let music = st.music_vol;
    let tr = &st.translations;
    let lang = &st.language;
    let langs = &st.available_languages;
    let overlay_clone = overlay.clone();
    let actions_clone = actions.clone();

    let menu_state: Rc<MenuState> = remember(MenuState::new);
    let lang_items: Vec<DropdownMenuEntry> = langs
        .iter()
        .map(|l| {
            let a = actions_clone.clone();
            let code = l.clone();
            let mut item = DropdownMenuItem::new(l.clone(), move || {
                push(&a, UiAction::SetLanguage(code.clone()))
            });
            if l == lang {
                item = item.disabled();
            }
            DropdownMenuEntry::Item(item)
        })
        .collect();
    let menu_trigger = menu_state.clone();
    let lang_label = st.language.clone();
    let trigger = FilledTonalButton(
        Modifier::new().width(100.0).height(40.0),
        move || menu_trigger.open(),
        ButtonConfig::default(),
        move || RText(lang_label.clone()).size(20.0),
    );

    let lang_dropdown = DropdownMenu(
        menu_state,
        overlay_clone,
        Modifier::new(),
        trigger,
        lang_items,
        DropdownMenuConfig {
            min_width: 100.0,
            ..Default::default()
        },
    );

    let inner = Column(
        Modifier::new()
            .width(360.0)
            .padding(24.0)
            .background(col(20, 20, 28))
            .clip_rounded(12.0)
            .align_items(AlignItems::CENTER),
    )
    .child(
        RText(t(tr, "settings", "Settings"))
            .size(36.0)
            .color(RColor::WHITE),
    )
    .child(spacer(12.0))
    .child(
        RText(format!(
            "{}: {:.0}%",
            t(tr, "master-volume", "Master"),
            master * 100.0
        ))
        .size(18.0)
        .color(RColor::WHITE),
    )
    .child(Row(Modifier::new().gap(8.0)).child((
        mk_button_sm("-", move || {
            push(&a_m_down, UiAction::SetMasterVol(master - 0.1))
        }),
        mk_button_sm("+", move || {
            push(&a_m_up, UiAction::SetMasterVol(master + 0.1))
        }),
    )))
    .child(spacer(8.0))
    .child(
        RText(format!(
            "{}: {:.0}%",
            t(tr, "sfx-volume", "SFX"),
            sfx * 100.0
        ))
        .size(18.0)
        .color(RColor::WHITE),
    )
    .child(Row(Modifier::new().gap(8.0)).child((
        mk_button_sm("-", move || push(&a_s_down, UiAction::SetSfxVol(sfx - 0.1))),
        mk_button_sm("+", move || push(&a_s_up, UiAction::SetSfxVol(sfx + 0.1))),
    )))
    .child(spacer(8.0))
    .child(
        RText(format!(
            "{}: {:.0}%",
            t(tr, "music-volume", "Music"),
            music * 100.0
        ))
        .size(18.0)
        .color(RColor::WHITE),
    )
    .child(Row(Modifier::new().gap(8.0)).child((
        mk_button_sm("-", move || {
            push(&a_mu_down, UiAction::SetMusicVol(music - 0.1))
        }),
        mk_button_sm("+", move || {
            push(&a_mu_up, UiAction::SetMusicVol(music + 0.1))
        }),
    )))
    .child(spacer(8.0))
    .child(
        RText(format!("{}:", t(tr, "language", "Language")))
            .size(18.0)
            .color(RColor::WHITE),
    )
    .child(Row(Modifier::new().gap(6.0)).child(lang_dropdown))
    .child(spacer(16.0))
    .child(mk_button(
        &t(tr, "save", "Save"),
        col(60, 120, 200),
        move || push(&a_save, UiAction::SaveSettings),
    ))
    .child(mk_button(
        &t(tr, "back", "Back"),
        col(70, 70, 90),
        move || push(&a_back, UiAction::CloseOverlay),
    ));

    Column(
        Modifier::new()
            .fill_max_size()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER)
            .background(RColor::from_rgba(0, 0, 0, 180)),
    )
    .child(inner)
}

fn credits_ui(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let a = actions.clone();
    let tr = &st.translations;
    let inner = Column(
        Modifier::new()
            .width(400.0)
            .padding(24.0)
            .background(col(20, 20, 28))
            .clip_rounded(12.0)
            .align_items(AlignItems::CENTER),
    )
    .child((
        RText(t(tr, "credits", "Credits"))
            .size(36.0)
            .color(RColor::WHITE),
        spacer(12.0),
        RText("Opensus — social deduction in Bevy")
            .size(16.0)
            .color(RColor::WHITE),
        RText("Inspired by OpenSuspect (GPL)")
            .size(16.0)
            .color(RColor::WHITE),
        RText("Ecosystem: mlm-games template + game-utils")
            .size(16.0)
            .color(RColor::WHITE),
        RText("Engine: Bevy  |  UI: Repose")
            .size(16.0)
            .color(RColor::WHITE),
        spacer(16.0),
        mk_button(&t(tr, "back", "Back"), col(70, 70, 90), move || {
            push(&a, UiAction::CloseOverlay)
        }),
    ));

    Column(
        Modifier::new()
            .fill_max_size()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER)
            .background(RColor::from_rgba(0, 0, 0, 180)),
    )
    .child(inner)
}

fn mk_button(label: &str, _bg: RColor, on_click: impl Fn() + 'static) -> View {
    FilledTonalButton(
        Modifier::new().width(260.0).height(52.0).margin(8.0),
        on_click,
        ButtonConfig::default(),
        move || RText(label).size(20.0),
    )
}

fn mk_button_sm(label: &str, on_click: impl Fn() + 'static) -> View {
    FilledTonalButton(
        Modifier::new().width(48.0).height(40.0),
        on_click,
        ButtonConfig::default(),
        move || RText(label).size(20.0),
    )
}

fn col(r: u8, g: u8, b: u8) -> RColor {
    RColor::from_rgba(r, g, b, 255)
}

fn push(actions: &Arc<Mutex<Vec<UiAction>>>, a: UiAction) {
    if let Ok(mut q) = actions.lock() {
        q.push(a);
    }
}
