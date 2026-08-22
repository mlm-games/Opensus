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
    Button, ButtonConfig, Card, CardConfig, DropdownMenu, DropdownMenuConfig, DropdownMenuEntry,
    DropdownMenuItem, FilledTonalButton, LinearProgressIndicator, LinearProgressIndicatorConfig,
    ListItem, ListItemConfig, MenuState, OutlinedButton, Slider, SliderConfig, TextButton,
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
                    matches!(st.game_phase, GamePhase::RoleReveal),
                    role_reveal_overlay(&st),
                    popup_anim_config("role_reveal"),
                ),
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
                    .background(RColor::from_rgba(0, 0, 0, fade_a))
                    .hit_passthrough(),
            ),
            Column(
                Modifier::new()
                    .fill_max_size()
                    .background(RColor::from_rgba(flash_a, flash_a, flash_a, flash_a))
                    .hit_passthrough(),
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
    let tr = &st.translations;

    let offline = actions.clone();
    let host = actions.clone();
    let join = actions.clone();
    let settings = actions.clone();
    let credits = actions.clone();
    let quit = actions.clone();

    let background = if let Some(handle) = st.ui_lab_bg {
        Image(Modifier::new().fill_max_size(), handle).image_fit(repose_core::ImageFit::Cover)
    } else {
        Column(Modifier::new().fill_max_size().background(p_bg()))
    };

    let branding = Column(
        Modifier::new()
            .width(470.0)
            .padding(24.0)
            .gap(12.0)
            .align_items(AlignItems::FLEX_START),
    )
    .child((
        RText(t(tr, "app-title", "Opensus"))
            .size(72.0)
            .color(p_cyan()),
        RText(t(tr, "app-tagline", "One among us is not like the rest."))
            .size(22.0)
            .color(p_text()),
        RText("Find the impostor. Finish your tasks. Survive the ship.")
            .size(16.0)
            .color(p_text_dim()),
        spacer(12.0),
        RText(t(
            tr,
            "controls-hint",
            "WASD move · E interact · Q kill · R report · F emergency",
        ))
        .size(13.0)
        .color(p_text_dim()),
    ));

    let menu_content = Column(
        Modifier::new()
            .padding(24.0)
            .gap(12.0)
            .align_items(AlignItems::STRETCH),
    )
    .child(RText(t(tr, "play", "Play")).size(28.0).color(p_text()))
    .child(
        RText("Choose how you want to enter the ship.")
            .size(14.0)
            .color(p_text_dim()),
    )
    .child(spacer(4.0))
    .child(action_button(
        t(tr, "start-game", "Play Offline"),
        ActionStyle::Primary,
        312.0,
        move || push(&offline, UiAction::PlayOffline),
    ))
    .child(action_button(
        t(tr, "host-game", "Host Game"),
        ActionStyle::Tonal,
        312.0,
        move || push(&host, UiAction::HostLobby),
    ))
    .child(action_button(
        t(tr, "join-game", "Join Game"),
        ActionStyle::Outlined,
        312.0,
        move || push(&join, UiAction::JoinLobby),
    ))
    .child(spacer(4.0))
    .child(action_button(
        t(tr, "settings", "Settings"),
        ActionStyle::Text,
        312.0,
        move || push(&settings, UiAction::OpenSettings),
    ))
    .child(action_button(
        t(tr, "credits", "Credits"),
        ActionStyle::Text,
        312.0,
        move || push(&credits, UiAction::OpenCredits),
    ))
    .child(action_button(
        t(tr, "quit", "Quit"),
        ActionStyle::Danger,
        312.0,
        move || push(&quit, UiAction::QuitApp),
    ));

    ZStack(Modifier::new().fill_max_size()).child((
        background,
        Column(
            Modifier::new()
                .fill_max_size()
                .background(RColor::from_rgba(0, 0, 0, 138))
                .justify_content(JustifyContent::CENTER)
                .align_items(AlignItems::CENTER),
        )
        .child(
            Row(Modifier::new().gap(40.0).align_items(AlignItems::CENTER))
                .child((branding, panel_card(360.0, menu_content))),
        ),
    ))
}

fn lobby_ui(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let tr = &st.translations;

    let ready = actions.clone();
    let start = actions.clone();
    let leave = actions.clone();
    let color = actions.clone();
    let settings = actions.clone();

    let player_count = st.lobby_slots.len();
    let ready_count = st.lobby_slots.iter().filter(|slot| slot.ready).count();

    let mut players = Column(Modifier::new().fill_max_width().gap(2.0));

    for slot in &st.lobby_slots {
        let status = if slot.ready {
            t(tr, "ready", "Ready")
        } else {
            t(tr, "not-ready", "Not ready")
        };

        let supporting = if slot.is_host {
            format!("{status} · Host")
        } else if slot.is_bot {
            format!("{status} · Bot")
        } else if slot.is_local {
            format!("{status} · You")
        } else {
            status
        };

        let trailing = RText(if slot.ready { "READY" } else { "WAITING" })
            .size(12.0)
            .color(if slot.ready { p_green() } else { p_text_dim() });

        players = players.child(ListItem(
            slot.name.clone(),
            Some(supporting),
            None,
            None,
            Some(trailing),
            None,
            None,
            ListItemConfig {
                selected: slot.is_local,
                shape_radius: 12.0,
                ..ListItemConfig::default()
            },
        ));
    }

    let roster = panel_card(
        560.0,
        Column(
            Modifier::new()
                .padding(24.0)
                .gap(12.0)
                .align_items(AlignItems::STRETCH),
        )
        .child((
            Row(Modifier::new()
                .fill_max_width()
                .justify_content(JustifyContent::SPACE_BETWEEN)
                .align_items(AlignItems::CENTER))
            .child((
                Column(Modifier::new()).child((
                    RText(t(tr, "lobby-waiting", "Lobby"))
                        .size(34.0)
                        .color(p_text()),
                    RText(format!("{ready_count}/{player_count} ready"))
                        .size(14.0)
                        .color(p_text_dim()),
                )),
                action_button(
                    t(tr, "color", "Color"),
                    ActionStyle::Outlined,
                    110.0,
                    move || push(&color, UiAction::CycleColor),
                ),
            )),
            progress_bar(
                512.0,
                if player_count == 0 {
                    0.0
                } else {
                    ready_count as f32 / player_count as f32
                },
                p_green(),
            ),
            players,
        )),
    );

    let mut controls = Column(
        Modifier::new()
            .padding(24.0)
            .gap(12.0)
            .align_items(AlignItems::STRETCH),
    )
    .child((
        RText(st.player_name.clone()).size(24.0).color(p_text()),
        RText(if st.is_host {
            "You are hosting this lobby."
        } else {
            "Waiting for the host to begin."
        })
        .size(14.0)
        .color(p_text_dim()),
        spacer(8.0),
        action_button(
            if st.local_ready {
                t(tr, "unready", "Unready")
            } else {
                t(tr, "ready", "Ready")
            },
            if st.local_ready {
                ActionStyle::Outlined
            } else {
                ActionStyle::Success
            },
            260.0,
            move || push(&ready, UiAction::ToggleReady),
        ),
    ));

    if st.is_host {
        controls = controls.child(action_button(
            t(tr, "start-match", "Start Match"),
            ActionStyle::Primary,
            260.0,
            move || push(&start, UiAction::StartMatch),
        ));
    }

    controls = controls
        .child(action_button(
            t(tr, "settings", "Settings"),
            ActionStyle::Text,
            260.0,
            move || push(&settings, UiAction::OpenSettings),
        ))
        .child(action_button(
            t(tr, "leave-lobby", "Leave Lobby"),
            ActionStyle::Danger,
            260.0,
            move || push(&leave, UiAction::LeaveLobby),
        ));

    Column(
        Modifier::new()
            .fill_max_size()
            .background(p_bg())
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER),
    )
    .child(
        Row(Modifier::new()
            .gap(24.0)
            .align_items(AlignItems::FLEX_START))
        .child((roster, panel_card(308.0, controls))),
    )
}

fn ingame_hud(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let tr = &st.translations;
    let emergency = actions.clone();
    let pause = actions.clone();

    let role_label = match st.my_role {
        Some(Role::Impostor) => t(tr, "impostor", "Impostor"),
        Some(Role::Crewmate) => t(tr, "crewmate", "Crewmate"),
        None => "-".into(),
    };

    let task_fraction = if st.tasks_total == 0 {
        0.0
    } else {
        st.tasks_done as f32 / st.tasks_total as f32
    };

    let status = panel_card(
        320.0,
        Column(
            Modifier::new()
                .padding(18.0)
                .gap(8.0)
                .align_items(AlignItems::STRETCH),
        )
        .child((
            Row(Modifier::new()
                .fill_max_width()
                .justify_content(JustifyContent::SPACE_BETWEEN)
                .align_items(AlignItems::CENTER))
            .child((
                RText(role_label)
                    .size(20.0)
                    .color(if matches!(st.my_role, Some(Role::Impostor)) {
                        p_red()
                    } else {
                        p_cyan()
                    }),
                RText(format!("{}/{}", st.tasks_done, st.tasks_total))
                    .size(14.0)
                    .color(p_text_dim()),
            )),
            progress_bar(284.0, task_fraction, p_green()),
            if matches!(st.my_role, Some(Role::Impostor)) {
                RText(format!(
                    "{} · {:.0}s",
                    t(tr, "kill-cooldown", "Kill cooldown"),
                    st.kill_cd
                ))
                .size(14.0)
                .color(p_red())
            } else {
                spacer(1.0)
            },
            if let Some(kind) = &st.sabotage_kind {
                RText(format!(
                    "{kind}{}",
                    if st.sabotage_remaining > 0.0 {
                        format!(" · {:.0}s", st.sabotage_remaining)
                    } else {
                        String::new()
                    }
                ))
                .size(14.0)
                .color(col(245, 175, 55))
            } else {
                spacer(1.0)
            },
        )),
    );

    let actions_card = panel_card(
        144.0,
        Row(Modifier::new()
            .padding(10.0)
            .gap(8.0)
            .align_items(AlignItems::CENTER))
        .child((
            compact_action_button("!", ActionStyle::Danger, move || {
                push(&emergency, UiAction::CallEmergency)
            }),
            compact_action_button("Ⅱ", ActionStyle::Tonal, move || {
                push(&pause, UiAction::TogglePause)
            }),
        )),
    );

    let prompt = if st.interact_prompt.is_empty() {
        Column(Modifier::new())
    } else {
        panel_card(
            420.0,
            Column(
                Modifier::new()
                    .padding(14.0)
                    .align_items(AlignItems::CENTER),
            )
            .child(RText(st.interact_prompt.clone()).size(18.0).color(p_cyan())),
        )
    };

    ZStack(Modifier::new().fill_max_size()).child((
        Row(Modifier::new()
            .fill_max_width()
            .padding(16.0)
            .justify_content(JustifyContent::SPACE_BETWEEN)
            .align_items(AlignItems::FLEX_START)
            .hit_passthrough())
        .child((status, actions_card)),
        Column(
            Modifier::new()
                .fill_max_size()
                .padding(20.0)
                .justify_content(JustifyContent::FLEX_END)
                .align_items(AlignItems::CENTER)
                .hit_passthrough(),
        )
        .child(prompt),
    ))
}

fn role_reveal_overlay(st: &SharedUi) -> View {
    let tr = &st.translations;
    let (title, color, hint) = match st.my_role {
        Some(Role::Impostor) => (
            t(tr, "impostor", "IMPOSTOR"),
            col(200, 70, 70),
            t(tr, "impostor-hint", "Kill crewmates. Sabotage. Blend in."),
        ),
        Some(Role::Crewmate) => (
            t(tr, "crewmate", "CREWMATE"),
            col(80, 180, 220),
            t(tr, "crewmate-hint", "Finish tasks. Find the impostor."),
        ),
        None => ("...".to_string(), RColor::WHITE, String::new()),
    };

    Column(
        Modifier::new()
            .fill_max_size()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER)
            .background(RColor::from_rgba(0, 0, 0, 220))
            .z_index(80.0)
            .input_blocker(),
    )
    .child(
        Column(
            Modifier::new()
                .padding(32.0)
                .gap(8.0)
                .align_items(AlignItems::CENTER),
        )
        .child((
            RText(t(tr, "you-are", "You are"))
                .size(22.0)
                .color(p_text_dim()),
            RText(title).size(56.0).color(color),
            RText(hint).size(16.0).color(p_text()),
            RText(format!("{:.0}", st.phase_timer.max(0.0)))
                .size(22.0)
                .color(p_cyan()),
        )),
    )
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
    if matches!(st.game_phase, GamePhase::Results) {
        for (name, n) in &st.vote_tallies {
            votes = votes.child(RText(format!("{name}: {n}")).size(15.0).color(p_text()));
        }
    } else if matches!(st.game_phase, GamePhase::Voting) && !st.my_voted && st.local_alive {
        for (id, name, dead) in &st.vote_options {
            if *dead {
                continue;
            }
            let a = actions.clone();
            let id = *id;
            votes = votes.child(action_button(
                format!("{} {}", t(tr, "vote", "Vote"), name),
                ActionStyle::Tonal,
                260.0,
                move || push(&a, UiAction::CastVote(id)),
            ));
        }
        let a_skip = actions.clone();
        votes = votes.child(action_button(
            t(tr, "skip", "Skip"),
            ActionStyle::Outlined,
            260.0,
            move || push(&a_skip, UiAction::SkipVote),
        ));
    }

    // Chat log (last 8) + input line
    let mut chat_col = Column(
        Modifier::new()
            .width(380.0)
            .padding(8.0)
            .background(col(14, 14, 20))
            .clip_rounded(8.0)
            .align_items(AlignItems::FLEX_START)
            .gap(2.0),
    );
    let start = st.chat_entries.len().saturating_sub(8);
    for (name, text, ghost) in &st.chat_entries[start..] {
        let (name_col, tag) = if *ghost {
            (col(150, 150, 200), " (ghost)")
        } else {
            (col(220, 200, 120), "")
        };
        chat_col = chat_col.child(
            Row(Modifier::new().gap(6.0))
                .child(RText(format!("{name}{tag}:")).size(14.0).color(name_col))
                .child(RText(text.clone()).size(14.0).color(RColor::WHITE)),
        );
    }
    let input_label = if st.chat_is_ghost_channel {
        format!("> [ghost] {}_", st.chat_buffer)
    } else {
        format!("> {}_", st.chat_buffer)
    };

    chat_col = chat_col.child(
        RText(input_label)
            .size(14.0)
            .color(if st.chat_is_ghost_channel {
                col(150, 150, 200)
            } else {
                col(160, 220, 160)
            }),
    );

    let panel = Column(
        Modifier::new()
            .width(430.0)
            .padding(26.0)
            .gap(6.0)
            .background(p_panel())
            .border(1.0, p_panel_border(), 14.0)
            .clip_rounded(14.0)
            .align_items(AlignItems::CENTER),
    )
    .child(
        RText(t(tr, "emergency-meeting", "Meeting"))
            .size(32.0)
            .color(p_cyan()),
    )
    .child(RText(phase_label).size(18.0).color(p_text_dim()))
    .child(
        RText(format!("{:.0}s", st.phase_timer))
            .size(16.0)
            .color(p_text_dim()),
    )
    .child(spacer(8.0))
    .child(RText(st.meeting_prompt.clone()).size(18.0).color(p_text()))
    .child(spacer(8.0))
    .child(
        RText(st.result_text.clone())
            .size(18.0)
            .color(col(220, 200, 120)),
    )
    .child(chat_col)
    .child(spacer(8.0))
    .child(votes);

    Column(
        Modifier::new()
            .fill_max_size()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER)
            .background(RColor::from_rgba(0, 0, 0, 200))
            .z_index(50.0)
            .input_blocker(),
    )
    .child(panel)
}

fn gameover_overlay(st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
    let tr = &st.translations;
    let (msg, sub, color) = match st.game_phase {
        GamePhase::GameOver {
            crew_win: true,
            reason,
        } => (
            t(tr, "crewmates-win", "Crewmates win!"),
            reason.label().to_string(),
            col(80, 180, 100),
        ),
        GamePhase::GameOver {
            crew_win: false,
            reason,
        } => (
            t(tr, "impostors-win", "Impostors win!"),
            reason.label().to_string(),
            col(200, 70, 70),
        ),
        _ => (String::new(), String::new(), RColor::WHITE),
    };
    let a = actions.clone();
    let a2 = actions.clone();

    Column(
        Modifier::new()
            .fill_max_size()
            .justify_content(JustifyContent::CENTER)
            .align_items(AlignItems::CENTER)
            .background(RColor::from_rgba(0, 0, 0, 210))
            .z_index(100.0)
            .input_blocker(),
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
            spacer(6.0),
            RText(sub).size(16.0).color(p_text_dim()),
            spacer(14.0),
            action_button(
                t(tr, "play-again", "Play Again"),
                ActionStyle::Success,
                260.0,
                move || push(&a, UiAction::PlayAgain),
            ),
            action_button(
                t(tr, "quit-to-title", "Quit to Title"),
                ActionStyle::Outlined,
                260.0,
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
        action_button(
            t(tr, "resume", "Resume"),
            ActionStyle::Success,
            260.0,
            move || push(&a1, UiAction::Resume),
        ),
        action_button(
            t(tr, "settings", "Settings"),
            ActionStyle::Text,
            260.0,
            move || push(&a2, UiAction::OpenSettings),
        ),
        action_button(
            t(tr, "credits", "Credits"),
            ActionStyle::Text,
            260.0,
            move || push(&a3, UiAction::OpenCredits),
        ),
        action_button(
            t(tr, "quit-to-title", "Quit to Title"),
            ActionStyle::Danger,
            260.0,
            move || push(&a4, UiAction::QuitToTitle),
        ),
    ])
}

fn settings_ui(overlay: OverlayHandle, st: &SharedUi, actions: Arc<Mutex<Vec<UiAction>>>) -> View {
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

    let master_action = actions.clone();
    let sfx_action = actions.clone();
    let music_action = actions.clone();

    let master_slider = Slider(
        master,
        (0.0, 1.0),
        Some(0.05),
        move |value| push(&master_action, UiAction::SetMasterVol(value)),
        SliderConfig {
            modifier: Modifier::new().width(328.0),
            active_track_color: p_cyan(),
            thumb_color: p_cyan(),
            ..SliderConfig::default()
        },
    );

    let sfx_slider = Slider(
        sfx,
        (0.0, 1.0),
        Some(0.05),
        move |value| push(&sfx_action, UiAction::SetSfxVol(value)),
        SliderConfig {
            modifier: Modifier::new().width(328.0),
            active_track_color: p_cyan(),
            thumb_color: p_cyan(),
            ..SliderConfig::default()
        },
    );

    let music_slider = Slider(
        music,
        (0.0, 1.0),
        Some(0.05),
        move |value| push(&music_action, UiAction::SetMusicVol(value)),
        SliderConfig {
            modifier: Modifier::new().width(328.0),
            active_track_color: p_cyan(),
            thumb_color: p_cyan(),
            ..SliderConfig::default()
        },
    );

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
            "{} · {:.0}%",
            t(tr, "master-volume", "Master"),
            master * 100.0
        ))
        .size(16.0)
        .color(p_text()),
    )
    .child(master_slider)
    .child(
        RText(format!(
            "{} · {:.0}%",
            t(tr, "sfx-volume", "SFX"),
            sfx * 100.0
        ))
        .size(16.0)
        .color(p_text()),
    )
    .child(sfx_slider)
    .child(
        RText(format!(
            "{} · {:.0}%",
            t(tr, "music-volume", "Music"),
            music * 100.0
        ))
        .size(16.0)
        .color(p_text()),
    )
    .child(music_slider)
    .child(spacer(8.0))
    .child(
        RText(format!("{}:", t(tr, "language", "Language")))
            .size(18.0)
            .color(p_text()),
    )
    .child(Row(Modifier::new().gap(6.0)).child(lang_dropdown))
    .child(spacer(14.0))
    .child(action_button(
        t(tr, "save", "Save"),
        ActionStyle::Primary,
        328.0,
        move || push(&a_save, UiAction::SaveSettings),
    ))
    .child(action_button(
        t(tr, "back", "Back"),
        ActionStyle::Text,
        328.0,
        move || push(&a_back, UiAction::CloseOverlay),
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
        action_button(t(tr, "back", "Back"), ActionStyle::Text, 260.0, move || {
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

#[derive(Clone, Copy)]
enum ActionStyle {
    Primary,
    Tonal,
    Outlined,
    Text,
    Danger,
    Success,
}

fn action_button(
    label: impl Into<String>,
    style: ActionStyle,
    width: f32,
    on_click: impl Fn() + 'static,
) -> View {
    let label = label.into();

    let mut config = ButtonConfig {
        height: 48.0,
        shape_radius: 24.0,
        ..ButtonConfig::default()
    };

    let modifier = Modifier::new().width(width).min_height(48.0);

    match style {
        ActionStyle::Primary => Button(modifier, on_click, config, move || RText(label).size(16.0)),
        ActionStyle::Tonal => {
            FilledTonalButton(modifier, on_click, config, move || RText(label).size(16.0))
        }
        ActionStyle::Outlined => {
            OutlinedButton(modifier, on_click, config, move || RText(label).size(16.0))
        }
        ActionStyle::Text => {
            TextButton(modifier, on_click, config, move || RText(label).size(16.0))
        }
        ActionStyle::Danger => {
            config.container_color = Some(p_red());
            config.content_color = Some(RColor::WHITE);

            Button(modifier, on_click, config, move || RText(label).size(16.0))
        }
        ActionStyle::Success => {
            config.container_color = Some(p_green());
            config.content_color = Some(col(5, 35, 15));

            Button(modifier, on_click, config, move || RText(label).size(16.0))
        }
    }
}

fn compact_action_button(
    label: impl Into<String>,
    style: ActionStyle,
    on_click: impl Fn() + 'static,
) -> View {
    action_button(label, style, 52.0, on_click)
}

fn panel_card(width: f32, content: View) -> View {
    Card(
        CardConfig {
            modifier: Modifier::new().width(width),
            container_color: p_panel(),
            content_color: p_text(),
            shape_radius: 24.0,
            tonal_elevation: 2.0,
            border: Some((1.0, p_panel_border())),
            ..CardConfig::default()
        },
        move || content,
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

#[allow(dead_code)]
fn p_cyan_dark() -> RColor {
    RColor::from_rgba(0x1F, 0x61, 0x75, 0xFF)
}

fn p_red() -> RColor {
    RColor::from_rgba(0xD9, 0x38, 0x47, 0xFF)
}

fn p_red_dark() -> RColor {
    RColor::from_rgba(0x73, 0x1A, 0x24, 0xFF)
}

#[allow(dead_code)]
fn p_button() -> RColor {
    RColor::from_rgba(0x29, 0x38, 0x4F, 0xFF)
}

fn p_green() -> RColor {
    RColor::from_rgba(0x47, 0xB8, 0x6B, 0xFF)
}

fn progress_bar(width: f32, fraction: f32, color: RColor) -> View {
    LinearProgressIndicator(
        Some(fraction.clamp(0.0, 1.0)),
        LinearProgressIndicatorConfig {
            modifier: Modifier::new().width(width).height(6.0),
            color,
            track_color: p_panel2(),
            ..LinearProgressIndicatorConfig::default()
        },
    )
}

fn push(actions: &Arc<Mutex<Vec<UiAction>>>, a: UiAction) {
    if let Ok(mut q) = actions.lock() {
        q.push(a);
    }
}
