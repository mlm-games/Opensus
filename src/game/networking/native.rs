use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, SystemTime};

use bevy::prelude::*;
use renet2::{ClientId, RenetClient, RenetServer, ServerEvent};
use renet2_netcode::{
    ClientAuthentication, NativeSocket, NetcodeClientTransport, NetcodeServerTransport,
    ServerAuthentication, ServerSetupConfig,
};

use crate::app::AppState;
use crate::game::{
    ActiveSabotage, Alive, Body, CHAT_MAX_LEN, ChatEntry, ChatState, EmergenciesLeft, GamePhase,
    Ghost, KillCooldownLeft, KillRequest, LobbySlot, LobbyState, LocalPlayer, LocalPlayerId,
    MatchConfig, MeetingCommand, MeetingState, OutgoingChat, Player, PlayerIntent, ReportBody,
    Role, RuntimeMode, SabotageAction, TaskBoard,
};

use super::PendingNetworkStart;
use super::channels::*;
use super::protocol::*;

const SNAPSHOT_HZ: f32 = 20.0;
const LOBBY_BROADCAST_HZ: f32 = 5.0;

#[derive(Resource)]
pub struct NetServerRes(pub RenetServer);

#[derive(Resource)]
pub struct NetServerTransportRes(pub NetcodeServerTransport);

#[derive(Resource)]
pub struct NetClientRes(pub RenetClient);

#[derive(Resource)]
pub struct NetClientTransportRes(pub NetcodeClientTransport);

#[derive(Resource, Default)]
pub struct NetworkIdentity {
    pub my_player_id: Option<u64>,
    pub hello_sent: bool,
    pub input_sequence: u32,
}

#[derive(Resource, Default)]
pub struct NetworkMappings {
    pub client_to_player: HashMap<ClientId, u64>,
    pub player_to_client: HashMap<u64, ClientId>,
    pub body_entities: HashMap<u64, Entity>,
    pub next_body_id: u64,
    pub last_input_sequence: HashMap<ClientId, u32>,
    pub authenticated_clients: std::collections::HashSet<ClientId>,
}

#[derive(Resource, Default)]
pub struct ServerSnapshotSequence(pub u32);

#[derive(Resource, Default)]
pub struct ClientSnapshotSequence {
    pub last_applied: Option<u32>,
}

fn sequence_is_newer(sequence: u32, previous: u32) -> bool {
    sequence != previous && sequence.wrapping_sub(previous) < (u32::MAX / 2)
}

#[derive(Resource)]
pub struct LobbyBroadcastTimer(pub Timer);

#[derive(Resource)]
pub struct SnapshotTimer(pub Timer);

#[derive(Component)]
pub struct ReplicaPlayer {
    pub player_id: u64,
}

#[derive(Component)]
pub struct ReplicaBody {
    pub body_id: u64,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum NativeNetSet {
    Bootstrap,
    ReceiveTransport,
    ReceivePackets,
    SendPackets,
    FlushTransport,
}

pub struct NativeNetworkingPlugin;

impl Plugin for NativeNetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkIdentity>()
            .init_resource::<NetworkMappings>()
            .init_resource::<ServerSnapshotSequence>()
            .init_resource::<ClientSnapshotSequence>()
            .insert_resource(LobbyBroadcastTimer(Timer::from_seconds(
                1.0 / LOBBY_BROADCAST_HZ,
                TimerMode::Repeating,
            )))
            .insert_resource(SnapshotTimer(Timer::from_seconds(
                1.0 / SNAPSHOT_HZ,
                TimerMode::Repeating,
            )))
            .configure_sets(
                PreUpdate,
                (
                    NativeNetSet::Bootstrap,
                    NativeNetSet::ReceiveTransport,
                    NativeNetSet::ReceivePackets,
                )
                    .chain(),
            )
            .configure_sets(
                PostUpdate,
                (NativeNetSet::SendPackets, NativeNetSet::FlushTransport).chain(),
            )
            .add_systems(PreUpdate, bootstrap_network.in_set(NativeNetSet::Bootstrap))
            .add_systems(
                PreUpdate,
                (
                    update_server_transport,
                    update_client_transport,
                    host_handle_connects_and_disconnects,
                )
                    .chain()
                    .in_set(NativeNetSet::ReceiveTransport),
            )
            .add_systems(
                PreUpdate,
                (
                    client_send_hello_once,
                    client_send_ready,
                    host_receive_reliable_packets,
                    host_receive_input_packets,
                )
                    .chain()
                    .in_set(NativeNetSet::ReceivePackets),
            )
            .add_systems(
                PostUpdate,
                (
                    host_broadcast_lobby_snapshot,
                    host_send_match_started,
                    host_send_world_snapshots,
                    host_relay_local_chat,
                    client_receive_packets,
                    client_send_input_packets,
                    client_send_actions,
                    client_send_chat,
                )
                    .chain()
                    .in_set(NativeNetSet::SendPackets),
            )
            .add_systems(
                PostUpdate,
                (send_server_packets, send_client_packets).in_set(NativeNetSet::FlushTransport),
            )
            .add_systems(Update, cleanup_network_on_title);
    }
}

fn now_duration() -> Duration {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
}

fn bootstrap_network(
    mut commands: Commands,
    mut pending: ResMut<PendingNetworkStart>,
    mode: Res<RuntimeMode>,
    server: Option<Res<NetServerRes>>,
    client: Option<Res<NetClientRes>>,
) {
    if matches!(*pending, PendingNetworkStart::None) {
        return;
    }
    if server.is_some() || client.is_some() {
        return;
    }

    match (&*mode, &*pending) {
        (RuntimeMode::Host, PendingNetworkStart::HostLocal { bind_addr }) => {
            let Ok(server_addr) = bind_addr.parse::<SocketAddr>() else {
                error!("Invalid host bind addr: {bind_addr}");
                *pending = PendingNetworkStart::None;
                return;
            };

            let Ok(socket) = UdpSocket::bind(server_addr) else {
                error!("Failed binding host socket at {server_addr}");
                *pending = PendingNetworkStart::None;
                return;
            };

            let server = RenetServer::new(connection_config());
            let setup = ServerSetupConfig {
                current_time: now_duration(),
                max_clients: 16,
                protocol_id: PROTOCOL_ID,
                socket_addresses: vec![vec![server_addr]],
                authentication: ServerAuthentication::Unsecure,
            };

            let Ok(transport) =
                NetcodeServerTransport::new(setup, NativeSocket::new(socket).unwrap())
            else {
                error!("Failed creating netcode server transport");
                *pending = PendingNetworkStart::None;
                return;
            };

            commands.insert_resource(NetServerRes(server));
            commands.insert_resource(NetServerTransportRes(transport));
            info!("Host listening on {server_addr}");
        }
        (RuntimeMode::Client, PendingNetworkStart::JoinLocal { server_addr }) => {
            let Ok(server_addr) = server_addr.parse::<SocketAddr>() else {
                error!("Invalid join addr: {server_addr}");
                *pending = PendingNetworkStart::None;
                return;
            };

            let client_id = now_duration().as_micros() as u64;
            let auth = ClientAuthentication::Unsecure {
                server_addr,
                client_id,
                user_data: None,
                protocol_id: PROTOCOL_ID,
                socket_id: 0,
            };

            let Ok(socket) = UdpSocket::bind("0.0.0.0:0") else {
                error!("Failed binding client udp socket");
                *pending = PendingNetworkStart::None;
                return;
            };

            let client = RenetClient::new(connection_config(), false);
            let Ok(transport) = NetcodeClientTransport::new(
                now_duration(),
                auth,
                NativeSocket::new(socket).unwrap(),
            ) else {
                error!("Failed creating netcode client transport");
                *pending = PendingNetworkStart::None;
                return;
            };

            commands.insert_resource(NetClientRes(client));
            commands.insert_resource(NetClientTransportRes(transport));
            info!("Client connecting to {server_addr}");
        }
        _ => {}
    }

    *pending = PendingNetworkStart::None;
}

fn update_server_transport(
    time: Res<Time<Real>>,
    server: Option<ResMut<NetServerRes>>,
    transport: Option<ResMut<NetServerTransportRes>>,
) {
    let (Some(mut server), Some(mut transport)) = (server, transport) else {
        return;
    };

    let delta = Duration::from_secs_f32(time.delta_secs());

    server.0.update(delta);
    if let Err(err) = transport.0.update(delta, &mut server.0) {
        warn!("server transport update failed: {err:?}");
    }
}

fn send_server_packets(
    server: Option<ResMut<NetServerRes>>,
    transport: Option<ResMut<NetServerTransportRes>>,
) {
    let (Some(mut server), Some(mut transport)) = (server, transport) else {
        return;
    };
    transport.0.send_packets(&mut server.0);
}

fn update_client_transport(
    time: Res<Time<Real>>,
    client: Option<ResMut<NetClientRes>>,
    transport: Option<ResMut<NetClientTransportRes>>,
) {
    let (Some(mut client), Some(mut transport)) = (client, transport) else {
        return;
    };

    let delta = Duration::from_secs_f32(time.delta_secs());

    client.0.update(delta);
    if let Err(err) = transport.0.update(delta, &mut client.0) {
        warn!("client transport update failed: {err}");
    }
}

fn send_client_packets(
    client: Option<ResMut<NetClientRes>>,
    transport: Option<ResMut<NetClientTransportRes>>,
) {
    let (Some(mut client), Some(mut transport)) = (client, transport) else {
        return;
    };
    if let Err(err) = transport.0.send_packets(&mut client.0) {
        warn!("client transport send failed: {err}");
    }
}

fn host_handle_connects_and_disconnects(
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

fn client_send_hello_once(
    client: Option<ResMut<NetClientRes>>,
    mut identity: ResMut<NetworkIdentity>,
    save: Res<crate::save::SaveData>,
) {
    let Some(mut client) = client else { return };
    if identity.hello_sent || !client.0.is_connected() {
        return;
    }

    let packet = ClientPacket::Hello {
        protocol_version: PROTOCOL_VERSION,
        name: save.player_name.chars().take(16).collect(),
        color_index: save.preferred_color_index,
    };

    if let Ok(bytes) = bincode::serialize(&packet) {
        client.0.send_message(C2S_RELIABLE, bytes);
        identity.hello_sent = true;
    }
}

fn client_send_ready(
    client: Option<ResMut<NetClientRes>>,
    lobby: Res<LobbyState>,
    mut previous: Local<Option<bool>>,
) {
    let Some(mut client) = client else { return };
    if !client.0.is_connected() {
        return;
    }
    if *previous == Some(lobby.local_ready) {
        return;
    }
    *previous = Some(lobby.local_ready);

    let packet = ClientPacket::Ready {
        ready: lobby.local_ready,
    };
    if let Ok(bytes) = bincode::serialize(&packet) {
        client.0.send_message(C2S_RELIABLE, bytes);
    }
}

fn client_send_chat(
    mut outgoing: MessageReader<OutgoingChat>,
    client: Option<ResMut<NetClientRes>>,
) {
    let Some(mut client) = client else { return };
    if !client.0.is_connected() {
        return;
    }
    for OutgoingChat(text) in outgoing.read() {
        let packet = ClientPacket::Chat {
            text: text.chars().take(CHAT_MAX_LEN).collect(),
        };
        if let Ok(bytes) = bincode::serialize(&packet) {
            client.0.send_message(C2S_RELIABLE, bytes);
        }
    }
}

fn host_relay_local_chat(
    mut outgoing: MessageReader<OutgoingChat>,
    server: Option<ResMut<NetServerRes>>,
    local: Query<(&Player, Option<&Alive>), With<LocalPlayer>>,
) {
    let Some(mut server) = server else { return };
    for OutgoingChat(text) in outgoing.read() {
        let Ok((player, alive)) = local.single() else {
            continue;
        };
        let packet = ServerPacket::Chat {
            player_id: player.id,
            name: player.name.clone(),
            text: text.clone(),
            ghost: alive.is_none(),
        };
        if let Ok(bytes) = bincode::serialize(&packet) {
            server.0.broadcast_message(S2C_RELIABLE, bytes);
        }
    }
}

fn host_receive_reliable_packets(
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
                // Client intent actions: player_id == client_id by construction.
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
                    // Identity from the connection mapping — never from the packet.
                    let Some((player, alive)) = players.iter().find(|(p, _)| p.id == player_id)
                    else {
                        continue;
                    };

                    let entry = ChatEntry {
                        player_id,
                        name: player.name.clone(),
                        text: text.clone(),
                        ghost: alive.is_none(),
                    };
                    chat.push(entry);

                    let packet = ServerPacket::Chat {
                        player_id,
                        name: player.name.clone(),
                        text,
                        ghost: alive.is_none(),
                    };
                    if let Ok(bytes) = bincode::serialize(&packet) {
                        server.0.broadcast_message(S2C_RELIABLE, bytes);
                    }
                }
                ClientPacket::Input { .. } => {}
            }
        }
    }
}

fn host_receive_input_packets(
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
                && !sequence_is_newer(sequence, *previous)
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

/// Server-authoritative movement for remote clients lives in
/// `player::apply_intent_movement` (GameSimSet::Resolve, authority only), which
/// integrates the intents received here into the remote players' transforms.
fn host_broadcast_lobby_snapshot(
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

fn host_send_match_started(
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

fn host_send_world_snapshots(
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

    // Build per-client private state
    let client_ids: Vec<ClientId> = server.0.clients_id().into_iter().collect();
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

fn client_receive_packets(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    client: Option<ResMut<NetClientRes>>,
    mut identity: ResMut<NetworkIdentity>,
    mut lobby: ResMut<LobbyState>,
    mut local_role: ResMut<crate::game::LocalRole>,
    mut game_phase: Option<ResMut<GamePhase>>,
    mut active_sab: Option<ResMut<ActiveSabotage>>,
    mut tasks: Option<ResMut<TaskBoard>>,
    mut meeting: Option<ResMut<MeetingState>>,
    mut snapshot_seq: Option<ResMut<ClientSnapshotSequence>>,
    mut chat: ResMut<ChatState>,
    mut replica_players: Query<(
        Entity,
        &ReplicaPlayer,
        &mut Transform,
        &mut Sprite,
        Option<&Alive>,
        Option<&Ghost>,
    )>,
    replica_bodies: Query<(Entity, &ReplicaBody)>,
    mut local_state: Query<
        (
            Entity,
            Option<&mut crate::game::KillCooldownLeft>,
            Option<&mut crate::game::EmergenciesLeft>,
        ),
        With<crate::game::LocalPlayer>,
    >,
) {
    let Some(mut client) = client else { return };

    while let Some(bytes) = client.0.receive_message(S2C_RELIABLE) {
        let Ok(packet) = bincode::deserialize::<ServerPacket>(&bytes) else {
            continue;
        };

        match packet {
            ServerPacket::Welcome { player_id } => {
                identity.my_player_id = Some(player_id);
            }
            ServerPacket::LobbySnapshot { players } => {
                lobby.is_host = false;
                lobby.slots = players
                    .into_iter()
                    .map(|p| LobbySlot {
                        id: p.player_id,
                        name: p.name,
                        color_index: p.color_index,
                        ready: p.ready,
                        is_local: identity.my_player_id == Some(p.player_id),
                        is_host: p.is_host,
                        is_bot: false,
                    })
                    .collect();

                // DO NOT force local_ready every packet: preserve the local
                // player's toggle and let client_send_ready diff it later.
                if let Some(slot) = lobby.slots.iter().find(|s| s.is_local) {
                    lobby.local_ready = slot.ready;
                }
            }
            ServerPacket::MatchStarted { your_role } => {
                local_role.0 = Some(your_role);
                next_state.set(AppState::InGame);
            }
            ServerPacket::Rejected { reason } => {
                warn!("server rejected client: {reason}");
            }
            ServerPacket::Chat {
                player_id,
                name,
                text,
                ghost,
            } => {
                chat.push(ChatEntry {
                    player_id,
                    name,
                    text,
                    ghost,
                });
            }
            ServerPacket::WorldSnapshot { .. } => {}
        }
    }

    while let Some(bytes) = client.0.receive_message(S2C_SNAPSHOT) {
        if bytes.len() > 64 * 1024 {
            continue;
        }
        let Ok(packet) = bincode::deserialize::<ServerPacket>(&bytes) else {
            continue;
        };

        let ServerPacket::WorldSnapshot {
            sequence,
            players,
            bodies,
            phase,
            sabotage,
            tasks_completed,
            tasks_total,
            meeting_prompt,
            meeting_timer,
            vote_options,
            result_text,
            private,
        } = packet
        else {
            continue;
        };

        if let Some(seq) = snapshot_seq.as_mut()
            && let Some(prev) = seq.last_applied
            && !sequence_is_newer(sequence, prev)
        {
            continue;
        }
        if let Some(seq) = snapshot_seq.as_mut() {
            seq.last_applied = Some(sequence);
        }

        // Keep the client's game/rules in sync with the host authority.
        if let Some(gp) = game_phase.as_mut() {
            **gp = phase;
        }
        if let Some(sab) = active_sab.as_mut() {
            if let Some(s) = sabotage {
                sab.kind = Some(s.kind);
                sab.fixes_needed = s.fixes_needed;
                sab.fixes_done = s.fixes_done;
                sab.timer =
                    (s.remaining > 0.0).then(|| Timer::from_seconds(s.remaining, TimerMode::Once));
            } else {
                sab.clear();
            }
        }
        if let Some(tb) = tasks.as_mut() {
            tb.completed = tasks_completed;
            tb.total = tasks_total;
        }
        if let Some(m) = meeting.as_mut() {
            m.prompt = meeting_prompt;
            m.timer = Timer::from_seconds(meeting_timer.max(0.1), TimerMode::Once);
            m.options = vote_options
                .into_iter()
                .map(|(id, name, dead)| crate::game::VoteOption {
                    player_id: id,
                    name,
                    dead,
                })
                .collect();
            m.result_text = result_text;
            if let Some(p) = &private {
                m.local_voted = p.voted;
                m.tallies = p.vote_tallies.clone();
            }
        }
        if let Some(p) = private {
            local_role.0 = Some(p.role);
            if let Ok((entity, cd_opt, em_opt)) = local_state.single_mut() {
                if let Some(mut cd) = cd_opt {
                    cd.0 = p.kill_cooldown;
                } else if p.kill_cooldown > 0.0 {
                    commands
                        .entity(entity)
                        .insert(crate::game::KillCooldownLeft(p.kill_cooldown));
                }
                if let Some(mut em) = em_opt {
                    em.0 = p.emergencies_left;
                } else {
                    commands
                        .entity(entity)
                        .insert(crate::game::EmergenciesLeft(p.emergencies_left));
                }
            }
        }

        let mut seen_players = Vec::new();

        for state in players {
            seen_players.push(state.player_id);

            if let Some((entity, _, mut transform, mut sprite, alive, ghost)) = replica_players
                .iter_mut()
                .find(|(_, marker, ..)| marker.player_id == state.player_id)
            {
                transform.translation.x = state.position[0];
                transform.translation.y = state.position[1];

                if state.alive {
                    sprite.color.set_alpha(1.0);
                    if alive.is_none() {
                        commands.entity(entity).insert(Alive);
                    }
                    if ghost.is_some() {
                        commands.entity(entity).remove::<Ghost>();
                    }
                } else {
                    sprite.color.set_alpha(0.35);
                    if alive.is_some() {
                        commands.entity(entity).remove::<Alive>();
                    }
                    if ghost.is_none() {
                        commands.entity(entity).insert(Ghost);
                    }
                }

                continue;
            }

            let color = crate::game::PLAYER_COLORS
                [state.color_index as usize % crate::game::PLAYER_COLORS.len()];

            let mut entity = commands.spawn((
                crate::game::MatchCleanup,
                ReplicaPlayer {
                    player_id: state.player_id,
                },
                Player {
                    id: state.player_id,
                    name: state.name,
                    color_index: state.color_index,
                    speed: 0.0,
                },
                Sprite {
                    color,
                    custom_size: Some(Vec2::splat(28.0)),
                    ..default()
                },
                Transform::from_xyz(state.position[0], state.position[1], 10.0),
            ));

            if state.alive {
                entity.insert(Alive);
            } else {
                entity.insert(Ghost);
            }

            if identity.my_player_id == Some(state.player_id) {
                entity.insert(LocalPlayer);
                entity.insert(PlayerIntent::default());
                commands.insert_resource(LocalPlayerId(Some(state.player_id)));
            }
        }

        for (entity, marker, _, _, _, _) in &mut replica_players {
            if !seen_players.contains(&marker.player_id) {
                commands.entity(entity).despawn();
            }
        }

        let mut seen_bodies = Vec::new();
        for body in bodies {
            seen_bodies.push(body.body_id);

            if replica_bodies
                .iter()
                .any(|(_, marker)| marker.body_id == body.body_id)
            {
                continue;
            }

            commands.spawn((
                crate::game::MatchCleanup,
                ReplicaBody {
                    body_id: body.body_id,
                },
                Body {
                    player_id: body.player_id,
                    name: body.name.clone(),
                    reported: body.reported,
                },
                Sprite {
                    color: Color::srgb(0.5, 0.05, 0.08),
                    custom_size: Some(Vec2::new(30.0, 14.0)),
                    ..default()
                },
                Transform::from_xyz(body.position[0], body.position[1], 3.0),
            ));
        }

        for (entity, marker) in &replica_bodies {
            if !seen_bodies.contains(&marker.body_id) {
                commands.entity(entity).despawn();
            }
        }
    }
}

fn client_send_input_packets(
    client: Option<ResMut<NetClientRes>>,
    mut identity: ResMut<NetworkIdentity>,
    mode: Res<RuntimeMode>,
    players: Query<&PlayerIntent, With<LocalPlayer>>,
    state: Res<State<AppState>>,
) {
    if !matches!(*mode, RuntimeMode::Client) || *state.get() != AppState::InGame {
        return;
    }
    let Some(mut client) = client else { return };
    if !client.0.is_connected() {
        return;
    }
    let Ok(intent) = players.single() else { return };

    identity.input_sequence = identity.input_sequence.wrapping_add(1);

    let packet = ClientPacket::Input {
        sequence: identity.input_sequence,
        movement: [intent.movement.x, intent.movement.y],
        interact: intent.interact,
    };

    if let Ok(bytes) = bincode::serialize(&packet) {
        client.0.send_message(C2S_INPUT, bytes);
    }
}

/// Forwards local input-gathered actions to the host over the reliable
/// channel. The authority validates every one of them; resolve systems do not
/// run on the client.
fn client_send_actions(
    client: Option<ResMut<NetClientRes>>,
    mode: Res<RuntimeMode>,
    mut kills: MessageReader<KillRequest>,
    mut reports: MessageReader<ReportBody>,
    mut meetings: MessageReader<MeetingCommand>,
    mut sabs: MessageReader<SabotageAction>,
) {
    if !matches!(*mode, RuntimeMode::Client) {
        return;
    }
    let Some(mut client) = client else { return };
    if !client.0.is_connected() {
        return;
    }
    for _ in kills.read() {
        if let Ok(b) = bincode::serialize(&ClientPacket::Kill) {
            client.0.send_message(C2S_RELIABLE, b);
        }
    }
    for _ in reports.read() {
        if let Ok(b) = bincode::serialize(&ClientPacket::Report) {
            client.0.send_message(C2S_RELIABLE, b);
        }
    }
    for cmd in meetings.read() {
        let packet = match cmd {
            MeetingCommand::Emergency { .. } => ClientPacket::Emergency,
            MeetingCommand::Vote { target, .. } => ClientPacket::Vote {
                target: Some(*target),
            },
            MeetingCommand::Skip { .. } => ClientPacket::Vote { target: None },
        };
        if let Ok(b) = bincode::serialize(&packet) {
            client.0.send_message(C2S_RELIABLE, b);
        }
    }
    for a in sabs.read() {
        if let Ok(b) = bincode::serialize(&ClientPacket::Sabotage { kind: a.kind }) {
            client.0.send_message(C2S_RELIABLE, b);
        }
    }
}

fn cleanup_network_on_title(
    state: Res<State<AppState>>,
    mut commands: Commands,
    mut was_title: Local<bool>,
    server: Option<Res<NetServerRes>>,
    client: Option<Res<NetClientRes>>,
) {
    let in_title = *state.get() == AppState::Title;
    if in_title == *was_title {
        return;
    }
    *was_title = in_title;
    if !in_title {
        return;
    }

    if server.is_some() {
        commands.remove_resource::<NetServerRes>();
        commands.remove_resource::<NetServerTransportRes>();
    }
    if client.is_some() {
        commands.remove_resource::<NetClientRes>();
        commands.remove_resource::<NetClientTransportRes>();
    }

    // Fresh identity/mappings so a later Host/Join session starts clean.
    commands.insert_resource(NetworkIdentity::default());
    commands.insert_resource(NetworkMappings::default());
    commands.insert_resource(ServerSnapshotSequence::default());
    commands.insert_resource(ClientSnapshotSequence::default());
}
