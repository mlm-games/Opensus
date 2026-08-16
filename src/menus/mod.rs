use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use repose_core::PaddingValues;
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
use repose_ui::{Column, Image, ImageExt, Row, Text as RText, TextStyle, ViewExt, ZStack};

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
    PlayOffline,
    HostLobby,
    JoinLobby,
    ToggleReady,
    StartMatch,
    LeaveLobby,
    CallEmergency,
    CastVote(u64),
    SkipVote,
    PlayAgain,
    CycleColor,
    OpenSettings,
    OpenCredits,
    CloseOverlay,
    TogglePause,
    Resume,
    QuitToTitle,
    QuitApp,
    SetMasterVol(f32),
    SetSfxVol(f32),
    SetMusicVol(f32),
    SaveSettings,
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
            // Note: `lights_out` remains in SharedUi for the HUD warning.
            // The world vision/FOW is rendered by the radial vision mask.
            let hud = ingame_hud(&st, actions.clone());
            let meeting = meeting_overlay(&st, actions.clone());
            let gameover = gameover_overlay(&st, actions.clone());
            ZStack(Modifier::new().fill_max_size()).child((
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
    let tr = &st.translations;
    Column(
        Modifier::new()
            .fill_max_size()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER)
            .background(p_bg()),
    )
    .child(
        RText(t(tr, "loading", "Loading..."))
            .size(32.0)
            .color(p_text()),
    )
    .child(spacer(8.0))
    .child(
        RText(t(tr, "loading-subtitle", "Preparing ship systems..."))
            .size(16.0)
            .color(p_text_dim()),
    )
    .child(spacer(16.0))
    .child(
        RText(format!("{:.0}%", pct * 100.0))
            .size(18.0)
            .color(p_text_dim()),
    )
    .child(spacer(12.0))
    .child(
        Column(
            Modifier::new()
                .width(320.0)
                .height(12.0)
                .background(p_panel2())
                .clip_rounded(6.0),
        )
        .child(Column(
            Modifier::new()
                .width((320.0 * pct).max(1.0))
                .height(12.0)
                .background(p_cyan())
                .clip_rounded(6.0)
                .align_self(AlignSelf::FLEX_START),
        )),
    )
}

fn title_ui(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let h0 = actions.clone();
    let h1 = actions.clone();
    let h2 = actions.clone();
    let h3 = actions.clone();
    let h4 = actions.clone();
    let h5 = actions.clone();
    let tr = &st.translations;

    let branding = Column(
        Modifier::new()
            .width(430.0)
            .gap(10.0)
            .align_items(AlignItems::FLEX_START)
            .padding(24.0),
    )
    .child((
        RText(t(tr, "app-title", "Opensus"))
            .size(72.0)
            .color(p_cyan()),
        RText(t(tr, "app-tagline", "One among us is not like the rest."))
            .size(20.0)
            .color(p_text()),
        Column(
            Modifier::new()
                .width(430.0)
                .height(3.0)
                .background(p_cyan())
                .clip_rounded(1.5),
        ),
        RText("Find the impostor. Finish your tasks. Survive the ship.")
            .size(15.0)
            .color(p_text_dim()),
        RText(t(
            tr,
            "controls-hint",
            "WASD move | E task/fix | Q kill | R report | F emergency | 1/2/3 sabotage | Esc pause",
        ))
        .size(13.0)
        .color(p_text_dim()),
    ));

    let menu = Column(
        Modifier::new()
            .width(360.0)
            .padding(24.0)
            .gap(2.0)
            .background(p_panel())
            .border(1.0, p_panel_border(), 14.0)
            .clip_rounded(14.0)
            .align_items(AlignItems::STRETCH),
    )
    .child([
        mk_button(&t(tr, "start-game", "Play Offline"), p_cyan_dark(), move || {
            push(&h0, UiAction::PlayOffline)
        }),
        mk_button(&t(tr, "host-game", "Host Game"), p_button(), move || {
            push(&h1, UiAction::HostLobby)
        }),
        mk_button(
            &t(tr, "join-game", "Join Game (local)"),
            p_button(),
            move || push(&h2, UiAction::JoinLobby),
        ),
        mk_button(&t(tr, "settings", "Settings"), p_button(), move || {
            push(&h3, UiAction::OpenSettings)
        }),
        mk_button(&t(tr, "credits", "Credits"), p_button(), move || {
            push(&h4, UiAction::OpenCredits)
        }),
        mk_button(&t(tr, "quit", "Quit"), p_red_dark(), move || {
            push(&h5, UiAction::QuitApp)
        }),
    ]);

    let bg = if let Some(h) = st.ui_lab_bg {
        Image(Modifier::new().fill_max_size(), h).image_fit(repose_core::ImageFit::Cover)
    } else {
        Column(Modifier::new().fill_max_size().background(p_bg()))
    };

    ZStack(Modifier::new().fill_max_size()).child((
        bg,
        Column(
            Modifier::new()
                .fill_max_size()
                .justify_content(JustifyContent::CENTER)
                .align_items(AlignItems::CENTER)
                .background(RColor::from_rgba(0, 0, 0, 120)),
        )
        .child(Row(Modifier::new().gap(28.0).align_items(AlignItems::CENTER)).child((branding, menu))),
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
            Column(
                Modifier::new()
                    .fill_max_width()
                    .padding_values(PaddingValues {
                        left: 10.0,
                        right: 10.0,
                        top: 6.0,
                        bottom: 6.0,
                    })
                    .background(p_panel2())
                    .clip_rounded(6.0),
            )
            .child(
                RText(format!("{mark} {}{}{}", s.name, host, you))
                    .size(18.0)
                    .color(p_text()),
            ),
        );
    }

    Column(
        Modifier::new()
            .fill_max_size()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER)
            .background(p_bg()),
    )
    .child(
        Column(
            Modifier::new()
                .width(560.0)
                .padding(28.0)
                .gap(6.0)
                .background(p_panel())
                .border(1.0, p_panel_border(), 14.0)
                .clip_rounded(14.0)
                .align_items(AlignItems::CENTER),
        )
        .child([
            RText(t(tr, "lobby-waiting", "Lobby"))
                .size(40.0)
                .color(p_cyan()),
            spacer(6.0),
            RText(format!("{}: {}", t(tr, "name", "Name"), st.player_name))
                .size(16.0)
                .color(p_text_dim()),
            mk_button(&t(tr, "color", "Cycle Color"), p_button(), move || {
                push(&a_color, UiAction::CycleColor)
            }),
            spacer(10.0),
            RText(t(tr, "players", "Players"))
                .size(22.0)
                .color(p_text()),
            list,
            spacer(14.0),
            mk_button(
                &if st.local_ready {
                    t(tr, "unready", "Unready")
                } else {
                    t(tr, "ready", "Ready")
                },
                p_green(),
                move || push(&a_ready, UiAction::ToggleReady),
            ),
            if st.is_host {
                mk_button(
                    &t(tr, "start-match", "Start Match"),
                    p_cyan_dark(),
                    move || push(&a_start, UiAction::StartMatch),
                )
            } else {
                spacer(1.0)
            },
            mk_button(&t(tr, "settings", "Settings"), p_button(), move || {
                push(&a_set, UiAction::OpenSettings)
            }),
            mk_button(&t(tr, "leave-lobby", "Leave"), p_red_dark(), move || {
                push(&a_leave, UiAction::LeaveLobby)
            }),
        ]),
    )
}

fn ingame_hud(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let tr = &st.translations;
    let role_str = match st.my_role {
        Some(Role::Impostor) => t(tr, "impostor", "Impostor"),
        Some(Role::Crewmate) => t(tr, "crewmate", "Crewmate"),
        None => "-".into(),
    };
    let a_em = actions.clone();
    let a_pause = actions.clone();

    let mut hud_col = Column(
        Modifier::new()
            .fill_max_size()
            .padding(16.0)
            .align_items(AlignItems::FLEX_START)
            .justify_content(JustifyContent::FLEX_START),
    );

    hud_col = hud_col.child((
        Row(Modifier::new().gap(8.0).align_items(AlignItems::CENTER)).child((
            mk_button_sm("!", p_red_dark(), move || {
                push(&a_em, UiAction::CallEmergency)
            }),
            mk_button_sm("II", p_button(), move || {
                push(&a_pause, UiAction::TogglePause)
            }),
            RText(t(tr, "pause", "Pause"))
                .size(13.0)
                .color(p_text_dim()),
        )),
        RText(format!("{}: {}", t(tr, "you-are", "You are"), role_str))
            .size(20.0)
            .color(if matches!(st.my_role, Some(Role::Impostor)) {
                p_red()
            } else {
                p_text()
            }),
        RText(format!(
            "{}: {}/{}",
            t(tr, "tasks-remaining", "Tasks"),
            st.tasks_done,
            st.tasks_total
        ))
        .size(16.0)
        .color(p_green()),
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
        RText(format!(
            "{}: {}  (F at map button)",
            t(tr, "emergency-meeting", "Emergencies"),
            st.emergencies_left
        ))
        .size(16.0)
        .color(col(180, 180, 220)),
        if let Some(kind) = &st.sabotage_kind {
            let time_part = if st.sabotage_remaining > 0.0 {
                format!(" - {:.0}s", st.sabotage_remaining)
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
        .color(p_text_dim()),
    ));

    hud_col
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
                .width(430.0)
                .padding(26.0)
                .gap(6.0)
                .background(p_panel())
                .border(1.0, p_panel_border(), 14.0)
                .clip_rounded(14.0)
                .align_items(AlignItems::CENTER),
        )
        .child((
            RText(t(tr, "emergency-meeting", "Meeting"))
                .size(32.0)
                .color(p_cyan()),
            RText(phase_label).size(18.0).color(p_text_dim()),
            RText(format!("{:.0}s", st.phase_timer))
                .size(16.0)
                .color(p_text_dim()),
            spacer(8.0),
            RText(st.meeting_prompt.clone()).size(18.0).color(p_text()),
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
                .width(440.0)
                .padding(30.0)
                .gap(4.0)
                .background(p_panel())
                .border(1.0, p_red_dark(), 16.0)
                .clip_rounded(16.0)
                .align_items(AlignItems::CENTER),
        )
        .child((
            RText(msg).size(38.0).color(color),
            spacer(14.0),
            mk_button(&t(tr, "play-again", "Play Again"), p_green(), move || {
                push(&a, UiAction::PlayAgain)
            }),
            mk_button(
                &t(tr, "quit-to-title", "Quit to Title"),
                p_button(),
                move || push(&a2, UiAction::QuitToTitle),
            ),
        )),
    )
}

fn pause_overlay(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let a1 = actions.clone();
    let a2 = actions.clone();
    let a3 = actions.clone();
    let a4 = actions.clone();
    let tr = &st.translations;

    Column(
        Modifier::new()
            .fill_max_size()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER)
            .background(RColor::from_rgba(0, 0, 0, 190)),
    )
    .child(pause_panel(tr, a1, a2, a3, a4))
}

fn pause_panel(
    tr: &HashMap<String, String>,
    a1: Arc<Mutex<Vec<UiAction>>>,
    a2: Arc<Mutex<Vec<UiAction>>>,
    a3: Arc<Mutex<Vec<UiAction>>>,
    a4: Arc<Mutex<Vec<UiAction>>>,
) -> View {
    Column(
        Modifier::new()
            .width(340.0)
            .padding(26.0)
            .gap(2.0)
            .background(p_panel())
            .border(1.0, p_panel_border(), 14.0)
            .clip_rounded(14.0)
            .align_items(AlignItems::STRETCH),
    )
    .child([
        RText(t(tr, "paused", "Paused")).size(36.0).color(p_text()),
        spacer(14.0),
        mk_button(&t(tr, "resume", "Resume"), p_green(), move || {
            push(&a1, UiAction::Resume)
        }),
        mk_button(&t(tr, "settings", "Settings"), p_button(), move || {
            push(&a2, UiAction::OpenSettings)
        }),
        mk_button(&t(tr, "credits", "Credits"), p_button(), move || {
            push(&a3, UiAction::OpenCredits)
        }),
        mk_button(
            &t(tr, "quit-to-title", "Quit to Title"),
            p_red_dark(),
            move || push(&a4, UiAction::QuitToTitle),
        ),
    ])
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
            .width(380.0)
            .padding(26.0)
            .gap(8.0)
            .background(p_panel())
            .border(1.0, p_panel_border(), 14.0)
            .clip_rounded(14.0)
            .align_items(AlignItems::CENTER),
    )
    .child(
        RText(t(tr, "settings", "Settings"))
            .size(36.0)
            .color(p_cyan()),
    )
    .child(spacer(8.0))
    .child(
        RText(format!(
            "{}: {:.0}%",
            t(tr, "master-volume", "Master"),
            master * 100.0
        ))
        .size(18.0)
        .color(p_text()),
    )
    .child(progress_bar(328.0, master, p_cyan()))
    .child(Row(Modifier::new().gap(8.0)).child((
        mk_button_sm("-", p_button(), move || {
            push(&a_m_down, UiAction::SetMasterVol(master - 0.1))
        }),
        mk_button_sm("+", p_button(), move || {
            push(&a_m_up, UiAction::SetMasterVol(master + 0.1))
        }),
    )))
    .child(spacer(6.0))
    .child(
        RText(format!(
            "{}: {:.0}%",
            t(tr, "sfx-volume", "SFX"),
            sfx * 100.0
        ))
        .size(18.0)
        .color(p_text()),
    )
    .child(progress_bar(328.0, sfx, p_cyan()))
    .child(Row(Modifier::new().gap(8.0)).child((
        mk_button_sm("-", p_button(), move || {
            push(&a_s_down, UiAction::SetSfxVol(sfx - 0.1))
        }),
        mk_button_sm("+", p_button(), move || {
            push(&a_s_up, UiAction::SetSfxVol(sfx + 0.1))
        }),
    )))
    .child(spacer(6.0))
    .child(
        RText(format!(
            "{}: {:.0}%",
            t(tr, "music-volume", "Music"),
            music * 100.0
        ))
        .size(18.0)
        .color(p_text()),
    )
    .child(progress_bar(328.0, music, p_cyan()))
    .child(Row(Modifier::new().gap(8.0)).child((
        mk_button_sm("-", p_button(), move || {
            push(&a_mu_down, UiAction::SetMusicVol(music - 0.1))
        }),
        mk_button_sm("+", p_button(), move || {
            push(&a_mu_up, UiAction::SetMusicVol(music + 0.1))
        }),
    )))
    .child(spacer(8.0))
    .child(
        RText(format!("{}:", t(tr, "language", "Language")))
            .size(18.0)
            .color(p_text()),
    )
    .child(Row(Modifier::new().gap(6.0)).child(lang_dropdown))
    .child(spacer(14.0))
    .child(mk_button(
        &t(tr, "save", "Save"),
        p_cyan_dark(),
        move || push(&a_save, UiAction::SaveSettings),
    ))
    .child(mk_button(&t(tr, "back", "Back"), p_button(), move || {
        push(&a_back, UiAction::CloseOverlay)
    }));

    Column(
        Modifier::new()
            .fill_max_size()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER)
            .background(RColor::from_rgba(0, 0, 0, 190)),
    )
    .child(inner)
}

fn credits_ui(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let a = actions.clone();
    let tr = &st.translations;
    let inner = Column(
        Modifier::new()
            .width(480.0)
            .padding(28.0)
            .gap(8.0)
            .background(p_panel())
            .border(1.0, p_panel_border(), 14.0)
            .clip_rounded(14.0)
            .align_items(AlignItems::CENTER),
    )
    .child((
        RText(t(tr, "credits", "Credits"))
            .size(36.0)
            .color(p_cyan()),
        RText("Opensus - open-source social deduction in Bevy")
            .size(16.0)
            .color(p_text()),
        RText("Inspired by OpenSuspect (GPL)")
            .size(16.0)
            .color(p_text_dim()),
        RText("Engine: Bevy  |  UI: Repose")
            .size(16.0)
            .color(p_text_dim()),
        RText("Font: Fredoka, licensed under OFL")
            .size(16.0)
            .color(p_text_dim()),
        RText("Project: mlm-games").size(16.0).color(p_cyan()),
        spacer(10.0),
        mk_button(&t(tr, "back", "Back"), p_button(), move || {
            push(&a, UiAction::CloseOverlay)
        }),
    ));

    Column(
        Modifier::new()
            .fill_max_size()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER)
            .background(RColor::from_rgba(0, 0, 0, 190)),
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

fn mk_button_sm(label: &str, _bg: RColor, on_click: impl Fn() + 'static) -> View {
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

fn p_bg() -> RColor {
    RColor::from_rgba(0x0E, 0x13, 0x1D, 0xFF)
}

fn p_panel() -> RColor {
    RColor::from_rgba(0x18, 0x20, 0x2D, 0xFF)
}

fn p_panel2() -> RColor {
    RColor::from_rgba(0x1F, 0x29, 0x3B, 0xFF)
}

fn p_panel_border() -> RColor {
    RColor::from_rgba(0x2E, 0x3A, 0x4E, 0xFF)
}

fn p_text() -> RColor {
    RColor::from_rgba(0xF0, 0xF5, 0xFA, 0xFF)
}

fn p_text_dim() -> RColor {
    RColor::from_rgba(0xA3, 0xB0, 0xC2, 0xFF)
}

fn p_cyan() -> RColor {
    RColor::from_rgba(0x40, 0xBD, 0xDB, 0xFF)
}

fn p_cyan_dark() -> RColor {
    RColor::from_rgba(0x1F, 0x61, 0x75, 0xFF)
}

fn p_red() -> RColor {
    RColor::from_rgba(0xD9, 0x38, 0x47, 0xFF)
}

fn p_red_dark() -> RColor {
    RColor::from_rgba(0x73, 0x1A, 0x24, 0xFF)
}

fn p_button() -> RColor {
    RColor::from_rgba(0x29, 0x38, 0x4F, 0xFF)
}

fn p_green() -> RColor {
    RColor::from_rgba(0x47, 0xB8, 0x6B, 0xFF)
}

fn progress_bar(width: f32, frac: f32, color: RColor) -> View {
    let inner_w = (width * frac.clamp(0.0, 1.0)).max(2.0);
    Column(
        Modifier::new()
            .width(width)
            .height(7.0)
            .background(col(30, 38, 50))
            .clip_rounded(3.5),
    )
    .child(Column(
        Modifier::new()
            .width(inner_w)
            .height(7.0)
            .background(color)
            .clip_rounded(3.5)
            .align_self(AlignSelf::FLEX_START),
    ))
}

fn push(actions: &Arc<Mutex<Vec<UiAction>>>, a: UiAction) {
    if let Ok(mut q) = actions.lock() {
        q.push(a);
    }
}
