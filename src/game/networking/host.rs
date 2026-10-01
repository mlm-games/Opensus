use bevy_ecs::prelude::*;
use glam::Vec2;
use log::warn;
use renet2::ServerEvent;

use crate::app::AppState;
use crate::game::{
    ActiveSabotage, Alive, Body, CHAT_MAX_LEN, ChatEntry, ChatState, EmergenciesLeft, GamePhase,
    Ghost, KillCooldownLeft, LobbySlot, LobbyState, LocalPlayer, MatchConfig, MeetingCommands,
    MeetingState, OutgoingChat, Player, PlayerIntent, Position, RemoteNetworkPlayer, ReportBodies,
    Role, SabotageFixStation, SabotageRequests, SolidAabb, TaskBoard, TaskStation,
};

use super::channels::*;
use super::common::{
    self, INPUT_BATCH_SIZE, LobbyBroadcastTimer, MAX_SERVER_PENDING_INPUTS, NetServerRes,
    NetworkMappings, ServerSnapshotSequence, SnapshotTimer,
};
use super::protocol::*;

pub fn host_handle_connects_and_disconnects(
    server: Option<ResMut<NetServerRes>>,
    mut lobby: ResMut<LobbyState>,
    mut mappings: ResMut<NetworkMappings>,
    config: Res<MatchConfig>,
    state: Res<AppState>,
    mut commands: Commands,
    remote_players: Query<(Entity, &Player), With<RemoteNetworkPlayer>>,
) {
    let Some(mut server) = server else { return };

    let now = common::now_duration();
    let mut timed_out = Vec::new();
    for (client_id, deadline) in mappings.handshake_deadline.iter() {
        if !mappings.authenticated_clients.contains(client_id) && now > *deadline {
            timed_out.push(*client_id);
        }
    }
    for client_id in timed_out {
        mappings.handshake_deadline.remove(&client_id);
        mappings.reliable_buckets.remove(&client_id);
        mappings.chat_buckets.remove(&client_id);
        mappings.action_buckets.remove(&client_id);
        server.0.disconnect(client_id);
    }

    while let Some(event) = server.0.get_event() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                if *state != AppState::Lobby {
                    server.0.disconnect(client_id);
                    continue;
                }
                if lobby.slots.len() >= config.max_players as usize {
                    server.0.disconnect(client_id);
                    continue;
                }

                let player_id = client_id;
                mappings.client_to_player.insert(client_id, player_id);
                mappings.player_to_client.insert(player_id, client_id);
                mappings
                    .handshake_deadline
                    .insert(client_id, now + common::HANDSHAKE_TIMEOUT);
                mappings.reliable_buckets.insert(
                    client_id,
                    common::TokenBucket::new(
                        common::RELIABLE_BURST,
                        common::RELIABLE_TOKENS_PER_SEC,
                        now,
                    ),
                );
                mappings.chat_buckets.insert(
                    client_id,
                    common::TokenBucket::new(common::CHAT_BURST, common::CHAT_TOKENS_PER_SEC, now),
                );
                mappings.action_buckets.insert(
                    client_id,
                    common::TokenBucket::new(
                        common::ACTION_BURST,
                        common::ACTION_TOKENS_PER_SEC,
                        now,
                    ),
                );
            }
            ServerEvent::ClientDisconnected { client_id, .. } => {
                mappings.authenticated_clients.remove(&client_id);
                mappings.pending_inputs.remove(&client_id);
                mappings.last_enqueued_input_sequence.remove(&client_id);
                mappings.last_processed_input_sequence.remove(&client_id);
                mappings.handshake_deadline.remove(&client_id);
                mappings.reliable_buckets.remove(&client_id);
                mappings.chat_buckets.remove(&client_id);
                mappings.action_buckets.remove(&client_id);
                if let Some(player_id) = mappings.client_to_player.remove(&client_id) {
                    mappings.player_to_client.remove(&player_id);
                    lobby.slots.retain(|slot| slot.id != player_id);
                    for (entity, player) in &remote_players {
                        if player.id == player_id {
                            commands.entity(entity).despawn();
                        }
                    }
                }
            }
        }
    }
}

pub fn host_relay_local_chat(
    outgoing: Res<OutgoingChat>,
    server: Option<ResMut<NetServerRes>>,
    mappings: Res<NetworkMappings>,
    players: Query<(&Player, Option<&Alive>)>,
    local: Query<(&Player, Option<&Alive>), With<LocalPlayer>>,
) {
    if outgoing.0.is_empty() {
        return;
    }
    let Some(mut server) = server else { return };
    for text in &outgoing.0 {
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
    mut kills: ResMut<crate::game::KillRequests>,
    mut reports: ResMut<ReportBodies>,
    mut meetings: ResMut<MeetingCommands>,
    mut sabotage: ResMut<SabotageRequests>,
    phase: Res<GamePhase>,
    mut chat: ResMut<ChatState>,
    players: Query<(&Player, Option<&Alive>)>,
    config: Res<MatchConfig>,
) {
    let Some(mut server) = server else { return };
    let now = common::now_duration();

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

            if let Some(bucket) = mappings.reliable_buckets.get_mut(&client_id)
                && !bucket.try_consume(now, 1.0)
            {
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
                    mappings.handshake_deadline.remove(&client_id);

                    if !lobby.slots.iter().any(|s| s.id == client_id) {
                        if lobby.slots.len() >= config.max_players as usize {
                            server.0.disconnect(client_id);
                            continue;
                        }
                        lobby.slots.push(LobbySlot {
                            id: client_id,
                            name: name.chars().take(16).collect(),
                            color_index,
                            ready: false,
                            is_local: false,
                            is_host: false,
                            is_bot: false,
                        });
                    } else if let Some(slot) = lobby.slots.iter_mut().find(|s| s.id == client_id) {
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
                    if let Some(bucket) = mappings.action_buckets.get_mut(&client_id)
                        && !bucket.try_consume(now, 1.0)
                    {
                        continue;
                    }
                    kills.0.push(crate::game::KillRequest {
                        actor_id: client_id,
                    });
                }
                ClientPacket::Report => {
                    if let Some(bucket) = mappings.action_buckets.get_mut(&client_id)
                        && !bucket.try_consume(now, 1.0)
                    {
                        continue;
                    }
                    reports.0.push(crate::game::ReportBody {
                        reporter_id: client_id,
                    });
                }
                ClientPacket::Emergency => {
                    if let Some(bucket) = mappings.action_buckets.get_mut(&client_id)
                        && !bucket.try_consume(now, 1.0)
                    {
                        continue;
                    }
                    meetings.0.push(crate::game::MeetingCommand::Emergency {
                        actor_id: client_id,
                    });
                }
                ClientPacket::Vote { target } => {
                    if let Some(bucket) = mappings.action_buckets.get_mut(&client_id)
                        && !bucket.try_consume(now, 1.0)
                    {
                        continue;
                    }
                    match target {
                        Some(t) => meetings.0.push(crate::game::MeetingCommand::Vote {
                            voter_id: client_id,
                            target: t,
                        }),
                        None => meetings.0.push(crate::game::MeetingCommand::Skip {
                            voter_id: client_id,
                        }),
                    };
                }
                ClientPacket::Sabotage { kind } => {
                    if let Some(bucket) = mappings.action_buckets.get_mut(&client_id)
                        && !bucket.try_consume(now, 1.0)
                    {
                        continue;
                    }
                    sabotage.0.push(crate::game::SabotageRequest {
                        actor_id: client_id,
                        kind,
                    });
                }
                ClientPacket::Chat { text } => {
                    if !mappings.authenticated_clients.contains(&client_id) {
                        continue;
                    }
                    if let Some(bucket) = mappings.chat_buckets.get_mut(&client_id)
                        && !bucket.try_consume(now, 1.0)
                    {
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
                    chat.push(ChatEntry {
                        player_id,
                        name: player.name.clone(),
                        text: text.clone(),
                        ghost,
                    });

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
) {
    let Some(mut server) = server else { return };
    let now = common::now_duration();

    for client_id in server.0.clients_id() {
        let mut processed_packets = 0usize;

        while let Some(bytes) = server.0.receive_message(client_id, C2S_INPUT) {
            processed_packets += 1;

            if processed_packets > 64 || bytes.len() > 4096 {
                break;
            }

            if let Some(bucket) = mappings.reliable_buckets.get_mut(&client_id)
                && !bucket.try_consume(now, 1.0)
            {
                continue;
            }

            let Ok(ClientPacket::Input { commands }) = bincode::deserialize::<ClientPacket>(&bytes)
            else {
                continue;
            };

            if !mappings.authenticated_clients.contains(&client_id)
                || commands.len() > INPUT_BATCH_SIZE
            {
                continue;
            }

            for command in commands {
                if !command.movement[0].is_finite() || !command.movement[1].is_finite() {
                    continue;
                }

                if let Some(previous) = mappings.last_enqueued_input_sequence.get(&client_id)
                    && !common::sequence_is_newer(command.sequence, *previous)
                {
                    continue;
                }

                let queue = mappings.pending_inputs.entry(client_id).or_default();

                if queue.len() >= MAX_SERVER_PENDING_INPUTS {
                    break;
                }

                queue.push_back(command);
                mappings
                    .last_enqueued_input_sequence
                    .insert(client_id, command.sequence);
            }
        }
    }
}

pub fn apply_remote_input_commands(
    time: Res<repame_sim::SimTime>,
    phase: Res<GamePhase>,
    config: Res<MatchConfig>,
    mut mappings: ResMut<NetworkMappings>,
    solids: Query<(&Position, &SolidAabb), Without<Player>>,
    mut players: Query<
        (
            &Player,
            &mut PlayerIntent,
            &mut Position,
            Option<&Alive>,
            Option<&Ghost>,
        ),
        With<RemoteNetworkPlayer>,
    >,
) {
    let boxes = crate::game::collision::solid_boxes(&solids);
    let playing = matches!(*phase, GamePhase::Playing);

    for (_, mut intent, _, _, _) in &mut players {
        intent.movement = Vec2::ZERO;
        intent.interact = false;
    }

    let client_ids: Vec<_> = mappings.pending_inputs.keys().copied().collect();

    for client_id in client_ids {
        let command = {
            let Some(queue) = mappings.pending_inputs.get_mut(&client_id) else {
                continue;
            };

            if playing {
                queue.pop_front()
            } else {
                let newest = queue.pop_back();
                queue.clear();
                newest
            }
        };

        let Some(command) = command else {
            continue;
        };

        let Some(player_id) = mappings.client_to_player.get(&client_id).copied() else {
            continue;
        };

        let Some((player, mut intent, mut position, alive, ghost)) = players
            .iter_mut()
            .find(|(player, _, _, _, _)| player.id == player_id)
        else {
            continue;
        };

        let movement = Vec2::new(command.movement[0], command.movement[1]).clamp_length_max(1.0);

        intent.movement = movement;
        intent.interact = playing && command.interact;

        if playing {
            let speed = if ghost.is_some() {
                player.speed * config.ghost_speed_mul
            } else {
                player.speed
            };

            position.0 = crate::game::collision::step_player_position(
                position.0,
                movement,
                speed,
                time.delta_secs,
                alive.is_some(),
                &boxes,
            );
        }

        mappings
            .last_processed_input_sequence
            .insert(client_id, command.sequence);
    }
}

pub fn host_broadcast_lobby_snapshot(
    time: Res<repame_sim::SimTime>,
    mut timer: ResMut<LobbyBroadcastTimer>,
    server: Option<ResMut<NetServerRes>>,
    lobby: Res<LobbyState>,
) {
    let Some(mut server) = server else { return };
    timer.0.tick(time.delta_secs);
    if !timer.0.just_finished() {
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
    state: Res<AppState>,
    mut previous: Local<Option<AppState>>,
    server: Option<ResMut<NetServerRes>>,
    mappings: Res<NetworkMappings>,
    players: Query<(&Player, &Role)>,
) {
    let entered_ingame = *state == AppState::InGame && *previous != Some(AppState::InGame);
    *previous = Some(*state);

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
    time: Res<repame_sim::SimTime>,
    mut timer: ResMut<SnapshotTimer>,
    mut sequence: ResMut<ServerSnapshotSequence>,
    server: Option<ResMut<NetServerRes>>,
    phase: Res<GamePhase>,
    sabotage: Res<ActiveSabotage>,
    tasks: Res<TaskBoard>,
    meeting: Res<MeetingState>,
    players_q: Query<(
        &Player,
        &Position,
        Option<&Alive>,
        Option<&Role>,
        Option<&KillCooldownLeft>,
        Option<&EmergenciesLeft>,
    )>,
    bodies: Query<(Entity, &Body, &Position)>,
    task_stations: Query<&TaskStation>,
    fix_stations: Query<&SabotageFixStation>,
    mut mappings: ResMut<NetworkMappings>,
) {
    let Some(mut server) = server else { return };
    timer.0.tick(time.delta_secs);
    if !timer.0.just_finished() {
        return;
    }
    sequence.0 = sequence.0.wrapping_add(1);

    let players = players_q
        .iter()
        .map(|(player, position, alive, _, _, _)| NetPlayerState {
            player_id: player.id,
            name: player.name.clone(),
            color_index: player.color_index,
            position: [position.0.x, position.0.y],
            alive: alive.is_some(),
        })
        .collect::<Vec<_>>();

    let mut body_states = Vec::new();
    for (entity, body, position) in &bodies {
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
            position: [position.0.x, position.0.y],
            reported: body.reported,
        });
    }

    let sabotage_state = sabotage.kind.map(|kind| NetSabotageState {
        kind,
        remaining: sabotage.critical_remaining(),
        fixes_needed: sabotage.fixes_needed,
        fixes_done: sabotage.fixes_done,
    });

    let (tasks_completed, tasks_total) = (tasks.completed, tasks.total);

    let task_states = task_stations
        .iter()
        .map(|station| NetTaskState {
            id: station.id,
            progress: 0.0,
            done: false,
        })
        .collect::<Vec<_>>();

    let fix_station_states = fix_stations
        .iter()
        .map(|station| NetFixStationState {
            id: station.id,
            kind: station.kind,
            progress: station.progress,
        })
        .collect::<Vec<_>>();

    let (meeting_prompt, meeting_timer, vote_options, result_text) = (
        meeting.prompt.clone(),
        meeting.timer.remaining_secs().max(0.0),
        meeting
            .options
            .iter()
            .map(|o| (o.player_id, o.name.clone(), o.dead))
            .collect::<Vec<_>>(),
        meeting.result_text.clone(),
    );

    let client_ids = server.0.clients_id();
    if client_ids.is_empty() {
        return;
    }

    for client_id in client_ids {
        let player_id = mappings.client_to_player.get(&client_id).copied();
        let private = player_id.and_then(|pid| {
            let (_, _, _, role, cd, em) =
                players_q.iter().find(|(p, _, _, _, _, _)| p.id == pid)?;
            let role = role.copied()?;
            let voted = meeting.votes.contains_key(&pid);
            let tallies = meeting.tallies.clone();
            let acknowledged_input_sequence =
                mappings
                    .player_to_client
                    .get(&pid)
                    .and_then(|mapped_client| {
                        mappings
                            .last_processed_input_sequence
                            .get(mapped_client)
                            .copied()
                    });
            Some(PrivatePlayerState {
                kill_cooldown: cd.map(|c| c.0).unwrap_or(0.0),
                emergencies_left: em.map(|e| e.0).unwrap_or(0),
                role,
                voted,
                vote_tallies: tallies,
                acknowledged_input_sequence,
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
            task_states: task_states.clone(),
            fix_station_states: fix_station_states.clone(),
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

#[cfg(test)]
mod tests {
    use bevy_ecs::schedule::Schedule;
    use bevy_ecs::world::World;
    use glam::Vec2;
    use repame_sim::SimTime;

    use super::*;
    use crate::game::{KillRequests, MatchConfig, ReportBodies};

    fn world_playing() -> World {
        let mut world = World::new();
        world.insert_resource(SimTime {
            elapsed_secs: 0.0,
            delta_secs: 1.0 / 60.0,
        });
        world.insert_resource(GamePhase::Playing);
        world.insert_resource(MatchConfig::default());
        world.insert_resource(NetworkMappings::default());
        world.spawn((
            RemoteNetworkPlayer,
            Player {
                id: 42,
                name: "Net".into(),
                color_index: 0,
                speed: 200.0,
            },
            Position(Vec2::ZERO),
            PlayerIntent::default(),
            Alive,
        ));
        {
            let mut mappings = world.resource_mut::<NetworkMappings>();
            mappings.client_to_player.insert(7, 42);
        }
        world
    }

    fn enqueue(world: &mut World, commands: [NetInputCommand; 2]) {
        let mut mappings = world.resource_mut::<NetworkMappings>();
        let queue = mappings.pending_inputs.entry(7).or_default();
        queue.push_back(commands[0]);
        queue.push_back(commands[1]);
    }

    fn remote_position(world: &mut World) -> Vec2 {
        let mut query = world.query_filtered::<&Position, With<RemoteNetworkPlayer>>();
        query.single(world).unwrap().0
    }

    fn remote_intent(world: &mut World) -> PlayerIntent {
        let mut query = world.query_filtered::<&PlayerIntent, With<RemoteNetworkPlayer>>();
        *query.single(world).unwrap()
    }

    #[test]
    fn remote_input_moves_player_and_acknowledges_sequence() {
        let mut world = world_playing();
        enqueue(
            &mut world,
            [
                NetInputCommand {
                    sequence: 1,
                    movement: [1.0, 0.0],
                    interact: false,
                },
                NetInputCommand {
                    sequence: 2,
                    movement: [0.0, 1.0],
                    interact: false,
                },
            ],
        );

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_remote_input_commands);
        schedule.run(&mut world);

        assert_eq!(remote_intent(&mut world).movement, Vec2::X);
        let position = remote_position(&mut world);
        assert!(position.x > 0.0 && position.y == 0.0);
        {
            let mappings = world.resource::<NetworkMappings>();
            assert_eq!(mappings.last_processed_input_sequence.get(&7), Some(&1));
            assert_eq!(mappings.pending_inputs[&7].len(), 1);
        }

        schedule.run(&mut world);
        assert_eq!(remote_intent(&mut world).movement, Vec2::Y);
        let position = remote_position(&mut world);
        assert!(position.y > 0.0);
        {
            let mappings = world.resource::<NetworkMappings>();
            assert_eq!(mappings.last_processed_input_sequence.get(&7), Some(&2));
            assert!(mappings.pending_inputs[&7].is_empty());
        }
    }

    #[test]
    fn remote_input_drains_newest_when_not_playing() {
        let mut world = world_playing();
        *world.resource_mut::<GamePhase>() = GamePhase::None;
        enqueue(
            &mut world,
            [
                NetInputCommand {
                    sequence: 3,
                    movement: [1.0, 0.0],
                    interact: false,
                },
                NetInputCommand {
                    sequence: 4,
                    movement: [0.0, -1.0],
                    interact: false,
                },
            ],
        );

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_remote_input_commands);
        schedule.run(&mut world);

        assert_eq!(remote_intent(&mut world).movement, -Vec2::Y);
        assert_eq!(remote_position(&mut world), Vec2::ZERO);
        let mappings = world.resource::<NetworkMappings>();
        assert_eq!(mappings.last_processed_input_sequence.get(&7), Some(&4));
        assert!(mappings.pending_inputs[&7].is_empty());
    }

    #[test]
    fn reliable_packets_turn_into_authority_requests() {
        let mut world = World::new();
        world.init_resource::<MatchConfig>();
        world.init_resource::<LobbyState>();
        world.init_resource::<NetworkMappings>();
        world.init_resource::<KillRequests>();
        world.init_resource::<ReportBodies>();
        world.init_resource::<crate::game::MeetingCommands>();
        world.init_resource::<crate::game::SabotageRequests>();
        world.init_resource::<ChatState>();
        world.insert_resource(GamePhase::Meeting);

        let mut schedule = Schedule::default();
        schedule.add_systems(host_receive_reliable_packets);
        schedule.run(&mut world);

        assert!(
            world
                .resource::<crate::game::MeetingCommands>()
                .0
                .is_empty()
        );
    }
}
