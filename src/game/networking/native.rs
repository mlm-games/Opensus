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
    ActiveSabotage, Alive, Body, GamePhase, Ghost, KillRequest, LobbySlot, LobbyState,
    LocalPlayer, LocalPlayerId, MatchConfig, MeetingCommand, MeetingState, Player, PlayerIntent,
    ReportBody, Role, RuntimeMode, SabotageAction, TaskBoard,
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

pub struct NativeNetworkingPlugin;

impl Plugin for NativeNetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkIdentity>()
            .init_resource::<NetworkMappings>()
            .insert_resource(LobbyBroadcastTimer(Timer::from_seconds(
                1.0 / LOBBY_BROADCAST_HZ,
                TimerMode::Repeating,
            )))
            .insert_resource(SnapshotTimer(Timer::from_seconds(
                1.0 / SNAPSHOT_HZ,
                TimerMode::Repeating,
            )))
            .add_systems(
                Update,
                (
                    bootstrap_network,
                    update_server_transport,
                    update_client_transport,
                    host_handle_connects_and_disconnects,
                    client_send_hello_once,
                    client_send_ready,
                    host_receive_reliable_packets,
                    host_receive_input_packets,
                    host_broadcast_lobby_snapshot,
                    host_send_match_started,
                    host_send_world_snapshots,
                    client_receive_packets,
                    client_send_input_packets,
                    client_send_actions,
                    cleanup_network_on_title,
                ),
            );
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
    time: Res<Time>,
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
    transport.0.send_packets(&mut server.0);
}

fn update_client_transport(
    time: Res<Time>,
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
                    });
                }
            }
            ServerEvent::ClientDisconnected { client_id, .. } => {
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

fn host_receive_reliable_packets(
    server: Option<ResMut<NetServerRes>>,
    mut lobby: ResMut<LobbyState>,
    mut kill_tx: MessageWriter<KillRequest>,
    mut report_tx: MessageWriter<ReportBody>,
    mut meeting_tx: MessageWriter<MeetingCommand>,
    mut sab_tx: MessageWriter<SabotageAction>,
) {
    let Some(mut server) = server else { return };

    for client_id in server.0.clients_id() {
        while let Some(bytes) = server.0.receive_message(client_id, C2S_RELIABLE) {
            let Ok(packet) = bincode::deserialize::<ClientPacket>(&bytes) else {
                continue;
            };

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
                        continue;
                    }

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
                ClientPacket::Chat { .. } | ClientPacket::Input { .. } => {}
            }
        }
    }
}

fn host_receive_input_packets(
    server: Option<ResMut<NetServerRes>>,
    mappings: Res<NetworkMappings>,
    mut players: Query<(&Player, &mut PlayerIntent), With<Alive>>,
) {
    let Some(mut server) = server else { return };

    for client_id in server.0.clients_id() {
        while let Some(bytes) = server.0.receive_message(client_id, C2S_INPUT) {
            let Ok(packet) = bincode::deserialize::<ClientPacket>(&bytes) else {
                continue;
            };

            let ClientPacket::Input {
                movement, interact, ..
            } = packet
            else {
                continue;
            };

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
    server: Option<ResMut<NetServerRes>>,
    phase: Res<GamePhase>,
    sabotage: Option<Res<ActiveSabotage>>,
    tasks: Option<Res<TaskBoard>>,
    meeting: Option<Res<MeetingState>>,
    players: Query<(&Player, &Transform, Option<&Alive>)>,
    bodies: Query<(Entity, &Body, &Transform)>,
    mut mappings: ResMut<NetworkMappings>,
) {
    let Some(mut server) = server else { return };
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let players = players
        .iter()
        .map(|(player, transform, alive)| NetPlayerState {
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

    let sabotage = sabotage.and_then(|s| {
        s.kind.map(|kind| NetSabotageState {
            kind,
            remaining: s.critical_remaining(),
            fixes_needed: s.fixes_needed,
            fixes_done: s.fixes_done,
        })
    });

    let (tasks_completed, tasks_total) = tasks
        .map(|t| (t.completed, t.total))
        .unwrap_or((0, 0));
    let (meeting_prompt, meeting_timer, vote_options, result_text) = meeting
        .map(|m| {
            (
                m.prompt.clone(),
                m.timer.remaining_secs().max(0.0),
                m.options
                    .iter()
                    .map(|o| (o.player_id, o.name.clone(), o.dead))
                    .collect(),
                m.result_text.clone(),
            )
        })
        .unwrap_or_default();

    let packet = ServerPacket::WorldSnapshot {
        sequence: 0,
        players,
        bodies: body_states,
        phase: *phase,
        sabotage,
        tasks_completed,
        tasks_total,
        meeting_prompt,
        meeting_timer,
        vote_options,
        result_text,
    };

    let Ok(bytes) = bincode::serialize(&packet) else {
        return;
    };

    server.0.broadcast_message(S2C_SNAPSHOT, bytes);
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
    mut replica_players: Query<(Entity, &ReplicaPlayer, &mut Transform, &mut Sprite)>,
    replica_bodies: Query<(Entity, &ReplicaBody)>,
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
            ServerPacket::WorldSnapshot { .. } | ServerPacket::Chat { .. } => {}
        }
    }

    while let Some(bytes) = client.0.receive_message(S2C_SNAPSHOT) {
        let Ok(packet) = bincode::deserialize::<ServerPacket>(&bytes) else {
            continue;
        };

        let ServerPacket::WorldSnapshot {
            sequence: _,
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
        } = packet
        else {
            continue;
        };

        // Keep the client's game/rules in sync with the host authority.
        if let Some(gp) = game_phase.as_mut() {
            **gp = phase;
        }
        if let Some(sab) = active_sab.as_mut() {
            if let Some(s) = sabotage {
                sab.kind = Some(s.kind);
                sab.fixes_needed = s.fixes_needed;
                sab.fixes_done = s.fixes_done;
                sab.timer = (s.remaining > 0.0)
                    .then(|| Timer::from_seconds(s.remaining, TimerMode::Once));
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
        }

        let mut seen_players = Vec::new();

        for state in players {
            seen_players.push(state.player_id);

            if let Some((_, _, mut transform, mut sprite)) = replica_players
                .iter_mut()
                .find(|(_, marker, _, _)| marker.player_id == state.player_id)
            {
                transform.translation.x = state.position[0];
                transform.translation.y = state.position[1];
                sprite.color.set_alpha(if state.alive { 1.0 } else { 0.35 });
                continue;
            }

            let color = crate::game::PLAYER_COLORS
                [state.color_index as usize % crate::game::PLAYER_COLORS.len()];

            let mut entity = commands.spawn((
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

        for (entity, marker, _, _) in &mut replica_players {
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
                ReplicaBody {
                    body_id: body.body_id,
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
}
