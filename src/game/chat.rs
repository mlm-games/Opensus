use std::collections::VecDeque;

use bevy_ecs::prelude::*;
use repose_core::input::{Key, KeyEvent, KeyEventType};

use super::{Alive, LocalPlayer, Player};

pub const CHAT_MAX_LEN: usize = 120;
pub const CHAT_LOG_CAP: usize = 50;

#[derive(Clone, Debug)]
pub struct ChatEntry {
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

    pub fn visible_to(&self, viewer_is_ghost: bool) -> impl Iterator<Item = &ChatEntry> {
        self.entries
            .iter()
            .filter(move |entry| viewer_is_ghost || !entry.ghost)
    }
}

#[derive(Resource, Default)]
pub struct ChatInputBuffer(pub String);

#[derive(Resource, Default)]
pub struct OutgoingChat(pub Vec<String>);

#[derive(Resource, Default)]
pub struct ChatKeys(pub Vec<KeyEvent>);

#[derive(Resource, Default)]
pub struct ChatIme(pub Vec<String>);

pub fn reset_chat(world: &mut World) {
    world.resource_mut::<ChatState>().clear();
    world.resource_mut::<ChatInputBuffer>().0.clear();
    world.resource_mut::<OutgoingChat>().0.clear();
    world.resource_mut::<ChatKeys>().0.clear();
    world.resource_mut::<ChatIme>().0.clear();
}

pub fn capture_chat_text(
    mut keys: ResMut<ChatKeys>,
    mut ime: ResMut<ChatIme>,
    mut buffer: ResMut<ChatInputBuffer>,
    mut outgoing: ResMut<OutgoingChat>,
) {
    for text in std::mem::take(&mut ime.0) {
        for ch in text.chars() {
            if !ch.is_control() && buffer.0.len() < CHAT_MAX_LEN {
                buffer.0.push(ch);
            }
        }
    }
    for key in std::mem::take(&mut keys.0) {
        if !matches!(key.event_type, KeyEventType::Down) {
            continue;
        }
        match key.key {
            Key::Enter => {
                if key.is_repeat {
                    continue;
                }
                let text: String = buffer.0.trim().chars().take(CHAT_MAX_LEN).collect();
                buffer.0.clear();
                if !text.is_empty() {
                    outgoing.0.push(text);
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
            Key::Character(ch) if !ch.is_control() && buffer.0.len() < CHAT_MAX_LEN => {
                buffer.0.push(ch);
            }
            _ => {}
        }
    }
}

pub fn apply_authority_chat(
    mut outgoing: ResMut<OutgoingChat>,
    mut chat: ResMut<ChatState>,
    local: Query<(&Player, Option<&Alive>), With<LocalPlayer>>,
) {
    for text in std::mem::take(&mut outgoing.0) {
        let Ok((player, alive)) = local.single() else {
            continue;
        };
        chat.push(ChatEntry {
            player_id: player.id,
            name: player.name.clone(),
            text,
            ghost: alive.is_none(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u64, ghost: bool) -> ChatEntry {
        ChatEntry {
            player_id: id,
            name: format!("P{id}"),
            text: "hi".into(),
            ghost,
        }
    }

    #[test]
    fn living_viewer_hides_ghost_messages() {
        let mut chat = ChatState::default();
        chat.push(entry(1, false));
        chat.push(entry(2, true));
        chat.push(entry(3, false));

        let visible: Vec<_> = chat.visible_to(false).map(|e| e.player_id).collect();
        assert_eq!(visible, vec![1, 3]);
    }

    #[test]
    fn ghost_viewer_sees_everything() {
        let mut chat = ChatState::default();
        chat.push(entry(1, false));
        chat.push(entry(2, true));

        let visible: Vec<_> = chat.visible_to(true).map(|e| e.player_id).collect();
        assert_eq!(visible, vec![1, 2]);
    }
}
