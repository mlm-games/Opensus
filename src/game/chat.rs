use std::collections::VecDeque;

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use super::{Alive, GamePhase, LocalPlayer, Player, RuntimeMode};
use crate::app::AppState;

pub const CHAT_MAX_LEN: usize = 120;
pub const CHAT_LOG_CAP: usize = 50;

#[derive(Clone, Debug)]
pub struct ChatEntry {
    #[allow(
        dead_code,
        reason = "Kept for ghost-filtering follow-up; identity is server-derived"
    )]
    pub player_id: u64,
    pub name: String,
    pub text: String,
    pub ghost: bool,
}

#[derive(Resource, Default)]
pub struct ChatState {
    pub entries: VecDeque<ChatEntry>,
}

impl ChatState {
    pub fn push(&mut self, entry: ChatEntry) {
        if self.entries.len() >= CHAT_LOG_CAP {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[derive(Resource, Default)]
pub struct ChatInputBuffer(pub String);

/// The local player submitted a chat line (any runtime mode).
#[derive(Message, Clone, Debug)]
pub struct OutgoingChat(pub String);

pub struct ChatPlugin;

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatState>()
            .init_resource::<ChatInputBuffer>()
            .add_message::<OutgoingChat>()
            .add_systems(OnEnter(AppState::InGame), reset_chat)
            .add_systems(OnExit(AppState::InGame), reset_chat)
            .add_systems(
                Update,
                (
                    capture_chat_text.run_if(chat_open),
                    apply_authority_chat.run_if(|mode: Res<RuntimeMode>| mode.has_authority()),
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

fn chat_open(phase: Res<GamePhase>) -> bool {
    matches!(*phase, GamePhase::Meeting | GamePhase::Voting)
}

fn reset_chat(mut chat: ResMut<ChatState>, mut buffer: ResMut<ChatInputBuffer>) {
    chat.clear();
    buffer.0.clear();
}

fn capture_chat_text(
    mut keys: MessageReader<KeyboardInput>,
    mut buffer: ResMut<ChatInputBuffer>,
    mut outgoing: MessageWriter<OutgoingChat>,
) {
    for key in keys.read() {
        if !key.state.is_pressed() {
            continue;
        }
        match &key.logical_key {
            Key::Enter => {
                let text: String = buffer.0.trim().chars().take(CHAT_MAX_LEN).collect();
                buffer.0.clear();
                if !text.is_empty() {
                    outgoing.write(OutgoingChat(text));
                }
            }
            Key::Backspace => {
                buffer.0.pop();
            }
            Key::Space => {
                if buffer.0.len() < CHAT_MAX_LEN {
                    buffer.0.push(' ');
                }
            }
            Key::Character(input) => {
                for ch in input.chars().filter(|c| !c.is_control()) {
                    if buffer.0.len() < CHAT_MAX_LEN {
                        buffer.0.push(ch);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Local and Host apply their own chat immediately.
/// Remote clients wait for the authoritative server echo instead.
fn apply_authority_chat(
    mut outgoing: MessageReader<OutgoingChat>,
    mut chat: ResMut<ChatState>,
    local: Query<(&Player, Option<&Alive>), With<LocalPlayer>>,
) {
    for OutgoingChat(text) in outgoing.read() {
        let Ok((player, alive)) = local.single() else {
            continue;
        };
        chat.push(ChatEntry {
            player_id: player.id,
            name: player.name.clone(),
            text: text.clone(),
            ghost: alive.is_none(),
        });
    }
}
