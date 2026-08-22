#![allow(unused_imports)]
use bevy::prelude::*;
use renet2::ServerEvent;

use crate::app::AppState;
use crate::game::{
    ActiveSabotage, Alive, Body, CHAT_MAX_LEN, ChatEntry, ChatState, EmergenciesLeft, GamePhase,
    Ghost, KillCooldownLeft, KillRequest, LobbySlot, LobbyState, LocalPlayer, MatchConfig,
    MeetingCommand, MeetingState, OutgoingChat, Player, PlayerIntent, ReportBody, Role,
    SabotageAction, TaskBoard,
};

use super::channels::*;
use super::common::{
    LobbyBroadcastTimer, NetServerRes, NetworkMappings, ServerSnapshotSequence, SnapshotTimer,
};
use super::protocol::*;

pub fn host_handle_connects_and_disconnects(
    server: Option<ResMut<NetServerRes>>,
    mut lobby: ResMut<LobbyState>,
    mut mappings: ResMut<NetworkMappings>,
    config: Res<MatchConfig>,
) {
    let Some(mut server) = server else { return };

    while let Some(event) = server.0.get_event() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                if lobby.slots.len() >= config.max_players as usize {
                    server.0.disconnect(client_id);
                    continue;
                }

                let player_id = client_id;
                mappings.client_to_player.insert(client_id, player_id);
                mappings.player_to_client.insert(player_id, client_id);

                if !lobby.slots.iter().any(|slot| slot.id == player_id) {
                    lobby.slots.push(LobbySlot {
                        id: player_id,
                        name: format!("Agent-{player_id}"),
                        color_index: 0,
                        ready: false,
                        is_local: false,
                        is_host: false,
                        is_bot: false,
                    });
                }
            }
            ServerEvent::ClientDisconnected { client_id, .. } => {
                mappings.authenticated_clients.remove(&client_id);
                mappings.last_input_sequence.remove(&client_id);
                if let Some(player_id) = mappings.client_to_player.remove(&client_id) {
                    mappings.player_to_client.remove(&player_id);
                    lobby.slots.retain(|slot| slot.id != player_id);
                }
            }
        }
    }
}

pub fn host_relay_local_chat(
    mut outgoing: MessageReader<OutgoingChat>,
    server: Option<ResMut<NetServerRes>>,
    mappings: Res<NetworkMappings>,
    players: Query<(&Player, Option<&Alive>)>,
    local: Query<(&Player, Option<&Alive>), With<LocalPlayer>>,
) {
    let Some(mut server) = server else { return };
    for OutgoingChat(text) in outgoing.read() {
        let Ok((player, alive)) = local.single() else {
            continue;
        };
        let ghost = alive.is_none();
        let packet = ServerPacket::Chat {
            player_id: player.id,
            name: player.name.clone(),
            text: text.clone(),
            ghost,
        };
        let Ok(bytes) = bincode::serialize(&packet) else {
            continue;
        };
        for client_id in server.0.clients_id() {
            let Some(recipient_player_id) = mappings.client_to_player.get(&client_id).copied()
            else {
                continue;
            };
            let recipient_is_ghost = players
                .iter()
                .find_map(|(p, a)| (p.id == recipient_player_id).then_some(a.is_none()))
                .unwrap_or(true);
            if ghost && !recipient_is_ghost {
                continue;
            }
            server
                .0
                .send_message(client_id, S2C_RELIABLE, bytes.clone());
        }
    }
}

pub fn host_receive_reliable_packets(
    server: Option<ResMut<NetServerRes>>,
    mut lobby: ResMut<LobbyState>,
    mut mappings: ResMut<NetworkMappings>,
    mut kill_tx: MessageWriter<KillRequest>,
    mut report_tx: MessageWriter<ReportBody>,
    mut meeting_tx: MessageWriter<MeetingCommand>,
    mut sab_tx: MessageWriter<SabotageAction>,
    phase: Res<GamePhase>,
    mut chat: ResMut<ChatState>,
    players: Query<(&Player, Option<&Alive>)>,
) {
    let Some(mut server) = server else { return };

    for client_id in server.0.clients_id() {
        let mut processed = 0u32;
        while let Some(bytes) = server.0.receive_message(client_id, C2S_RELIABLE) {
            processed += 1;
            if processed > 32 {
                warn!("client {client_id:?} exceeded reliable packet cap");
                break;
            }
            if bytes.len() > 4096 {
                continue;
            }
            let Ok(packet) = bincode::deserialize::<ClientPacket>(&bytes) else {
                continue;
            };

            let is_authed = mappings.authenticated_clients.contains(&client_id);
            if !is_authed && !matches!(packet, ClientPacket::Hello { .. }) {
                continue;
            }

            match packet {
                ClientPacket::Hello {
                    protocol_version,
                    name,
                    color_index,
                } => {
                    if protocol_version != PROTOCOL_VERSION {
                        if let Ok(bytes) = bincode::serialize(&ServerPacket::Rejected {
                            reason: "protocol mismatch".into(),
                        }) {
                            server.0.send_message(client_id, S2C_RELIABLE, bytes);
                        }
                        server.0.disconnect(client_id);
                        break;
                    }

                    mappings.authenticated_clients.insert(client_id);

                    if let Some(slot) = lobby.slots.iter_mut().find(|s| s.id == client_id) {
                        slot.name = name.chars().take(16).collect();
                        slot.color_index = color_index;
                    }

                    if let Ok(bytes) = bincode::serialize(&ServerPacket::Welcome {
                        player_id: client_id,
                    }) {
                        server.0.send_message(client_id, S2C_RELIABLE, bytes);
                    }
                }
                ClientPacket::Ready { ready } => {
                    if let Some(slot) = lobby.slots.iter_mut().find(|s| s.id == client_id) {
                        slot.ready = ready;
                    }
                }
                ClientPacket::Kill => {
                    kill_tx.write(KillRequest {
                        actor_id: client_id,
                    });
                }
                ClientPacket::Report => {
                    report_tx.write(ReportBody {
                        reporter_id: client_id,
                    });
                }
                ClientPacket::Emergency => {
                    meeting_tx.write(MeetingCommand::Emergency {
                        actor_id: client_id,
                    });
                }
                ClientPacket::Vote { target } => {
                    match target {
                        Some(t) => meeting_tx.write(MeetingCommand::Vote {
                            voter_id: client_id,
                            target: t,
                        }),
                        None => meeting_tx.write(MeetingCommand::Skip {
                            voter_id: client_id,
                        }),
                    };
                }
                ClientPacket::Sabotage { kind } => {
                    sab_tx.write(SabotageAction {
                        actor_id: client_id,
                        kind,
                    });
                }
                ClientPacket::Chat { text } => {
                    if !mappings.authenticated_clients.contains(&client_id) {
                        continue;
                    }
                    if !matches!(*phase, GamePhase::Meeting | GamePhase::Voting) {
                        continue;
                    }
                    let text: String = text.trim().chars().take(CHAT_MAX_LEN).collect();
                    if text.is_empty() {
                        continue;
                    }
                    let Some(player_id) = mappings.client_to_player.get(&client_id).copied() else {
                        continue;
                    };
                    let Some((player, alive)) = players.iter().find(|(p, _)| p.id == player_id)
                    else {
                        continue;
                    };

                    let ghost = alive.is_none();
                    let entry = ChatEntry {
                        player_id,
                        name: player.name.clone(),
                        text: text.clone(),
                        ghost,
                    };
                    chat.push(entry);

                    let packet = ServerPacket::Chat {
                        player_id,
                        name: player.name.clone(),
                        text,
                        ghost,
                    };
                    let Ok(bytes) = bincode::serialize(&packet) else {
                        continue;
                    };
                    for recipient_client_id in server.0.clients_id() {
                        let Some(recipient_player_id) =
                            mappings.client_to_player.get(&recipient_client_id).copied()
                        else {
                            continue;
                        };
                        let recipient_is_ghost = players
                            .iter()
                            .find_map(|(p, a)| (p.id == recipient_player_id).then_some(a.is_none()))
                            .unwrap_or(true);
                        if ghost && !recipient_is_ghost {
                            continue;
                        }
                        server
                            .0
                            .send_message(recipient_client_id, S2C_RELIABLE, bytes.clone());
                    }
                }
                ClientPacket::Input { .. } => {}
            }
        }
    }
}

pub fn host_receive_input_packets(
    server: Option<ResMut<NetServerRes>>,
    mut mappings: ResMut<NetworkMappings>,
    mut players: Query<(&Player, &mut PlayerIntent), With<Alive>>,
) {
    let Some(mut server) = server else { return };

    for client_id in server.0.clients_id() {
        let mut processed = 0u32;
        while let Some(bytes) = server.0.receive_message(client_id, C2S_INPUT) {
            processed += 1;
            if processed > 64 {
                break;
            }
            if bytes.len() > 4096 {
                continue;
            }
            let Ok(packet) = bincode::deserialize::<ClientPacket>(&bytes) else {
                continue;
            };

            let ClientPacket::Input {
                sequence,
                movement,
                interact,
            } = packet
            else {
                continue;
            };

            if !movement[0].is_finite() || !movement[1].is_finite() {
                continue;
            }

            if !mappings.authenticated_clients.contains(&client_id) {
                continue;
            }

            if let Some(previous) = mappings.last_input_sequence.get(&client_id)
                && !super::common::sequence_is_newer(sequence, *previous)
            {
                continue;
            }

            mappings.last_input_sequence.insert(client_id, sequence);

            let Some(player_id) = mappings.client_to_player.get(&client_id).copied() else {
                continue;
            };

            for (player, mut intent) in &mut players {
                if player.id == player_id {
                    intent.movement = Vec2::new(movement[0], movement[1]).clamp_length_max(1.0);
                    intent.interact = interact;
                    break;
                }
            }
        }
    }
}

pub fn host_broadcast_lobby_snapshot(
    time: Res<Time>,
    mut timer: ResMut<LobbyBroadcastTimer>,
    server: Option<ResMut<NetServerRes>>,
    lobby: Res<LobbyState>,
) {
    let Some(mut server) = server else { return };
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let players = lobby
        .slots
        .iter()
        .map(|slot| NetLobbyPlayer {
            player_id: slot.id,
            name: slot.name.clone(),
            color_index: slot.color_index,
            ready: slot.ready,
            is_host: slot.is_host,
        })
        .collect::<Vec<_>>();

    let Ok(bytes) = bincode::serialize(&ServerPacket::LobbySnapshot { players }) else {
        return;
    };

    server.0.broadcast_message(S2C_RELIABLE, bytes);
}

pub fn host_send_match_started(
    state: Res<State<AppState>>,
    mut previous: Local<Option<AppState>>,
    server: Option<ResMut<NetServerRes>>,
    mappings: Res<NetworkMappings>,
    players: Query<(&Player, &Role)>,
) {
    let entered_ingame = *state.get() == AppState::InGame && *previous != Some(AppState::InGame);
    *previous = Some(state.get().clone());

    if !entered_ingame {
        return;
    }

    let Some(mut server) = server else { return };

    for (player_id, client_id) in &mappings.player_to_client {
        let Some((_, role)) = players.iter().find(|(p, _)| p.id == *player_id) else {
            continue;
        };

        if let Ok(bytes) = bincode::serialize(&ServerPacket::MatchStarted { your_role: *role }) {
            server.0.send_message(*client_id, S2C_RELIABLE, bytes);
        }
    }
}

pub fn host_send_world_snapshots(
    time: Res<Time>,
    mut timer: ResMut<SnapshotTimer>,
    mut sequence: ResMut<ServerSnapshotSequence>,
    server: Option<ResMut<NetServerRes>>,
    phase: Res<GamePhase>,
    sabotage: Option<Res<ActiveSabotage>>,
    tasks: Option<Res<TaskBoard>>,
    meeting: Option<Res<MeetingState>>,
    players_q: Query<(
        &Player,
        &Transform,
        Option<&Alive>,
        Option<&Role>,
        Option<&KillCooldownLeft>,
        Option<&EmergenciesLeft>,
    )>,
    bodies: Query<(Entity, &Body, &Transform)>,
    mut mappings: ResMut<NetworkMappings>,
) {
    let Some(mut server) = server else { return };
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }
    sequence.0 = sequence.0.wrapping_add(1);

    let players = players_q
        .iter()
        .map(|(player, transform, alive, _, _, _)| NetPlayerState {
            player_id: player.id,
            name: player.name.clone(),
            color_index: player.color_index,
            position: [transform.translation.x, transform.translation.y],
            alive: alive.is_some(),
        })
        .collect::<Vec<_>>();

    let mut body_states = Vec::new();
    for (entity, body, transform) in &bodies {
        let body_id = mappings
            .body_entities
            .iter()
            .find_map(|(id, e)| (*e == entity).then_some(*id))
            .unwrap_or_else(|| {
                mappings.next_body_id += 1;
                let id = mappings.next_body_id;
                mappings.body_entities.insert(id, entity);
                id
            });

        body_states.push(NetBodyState {
            body_id,
            player_id: body.player_id,
            name: body.name.clone(),
            position: [transform.translation.x, transform.translation.y],
            reported: body.reported,
        });
    }

    let sabotage_state = sabotage.and_then(|s| {
        s.kind.map(|kind| NetSabotageState {
            kind,
            remaining: s.critical_remaining(),
            fixes_needed: s.fixes_needed,
            fixes_done: s.fixes_done,
        })
    });

    let (tasks_completed, tasks_total) = tasks.map(|t| (t.completed, t.total)).unwrap_or((0, 0));
    let (meeting_prompt, meeting_timer, vote_options, result_text) = meeting
        .as_ref()
        .map(|m| {
            (
                m.prompt.clone(),
                m.timer.remaining_secs().max(0.0),
                m.options
                    .iter()
                    .map(|o| (o.player_id, o.name.clone(), o.dead))
                    .collect::<Vec<_>>(),
                m.result_text.clone(),
            )
        })
        .unwrap_or_default();

    let client_ids: Vec<renet2::ClientId> = server.0.clients_id().into_iter().collect();
    if client_ids.is_empty() {
        return;
    }

    for client_id in client_ids {
        let player_id = mappings.client_to_player.get(&client_id).copied();
        let private = player_id.and_then(|pid| {
            let (_, _, _, role, cd, em) =
                players_q.iter().find(|(p, _, _, _, _, _)| p.id == pid)?;
            let role = role.copied()?;
            let voted = meeting
                .as_ref()
                .map(|m| m.votes.contains_key(&pid))
                .unwrap_or(false);
            let tallies = meeting
                .as_ref()
                .map(|m| m.tallies.clone())
                .unwrap_or_default();
            Some(PrivatePlayerState {
                kill_cooldown: cd.map(|c| c.0).unwrap_or(0.0),
                emergencies_left: em.map(|e| e.0).unwrap_or(0),
                role,
                voted,
                vote_tallies: tallies,
            })
        });

        let packet = ServerPacket::WorldSnapshot {
            sequence: sequence.0,
            players: players.clone(),
            bodies: body_states.clone(),
            phase: *phase,
            sabotage: sabotage_state.clone(),
            tasks_completed,
            tasks_total,
            meeting_prompt: meeting_prompt.clone(),
            meeting_timer,
            vote_options: vote_options.clone(),
            result_text: result_text.clone(),
            private,
        };

        if let Ok(bytes) = bincode::serialize(&packet) {
            server.0.send_message(client_id, S2C_SNAPSHOT, bytes);
        }
    }
}
