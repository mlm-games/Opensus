#![allow(unused_imports)]
use bevy::prelude::*;

use crate::app::AppState;
use crate::game::{
    ActiveSabotage, Alive, Body, CHAT_MAX_LEN, ChatEntry, ChatState, EmergenciesLeft, Ghost,
    KillCooldownLeft, KillRequest, LobbySlot, LobbyState, LocalPlayer, LocalPlayerId,
    MeetingCommand, MeetingState, OutgoingChat, Player, PlayerIntent, ReportBody, Role,
    RuntimeMode, SabotageAction, TaskBoard,
};

use super::channels::*;
use super::common::{
    ClientSnapshotSequence, INTERPOLATION_DELAY, NetClientRes, NetworkIdentity, ReplicaBody,
    ReplicaInterpolation, ReplicaPlayer, sample_position, sequence_is_newer,
};
use super::protocol::*;

pub fn client_send_hello_once(
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

pub fn client_send_ready(
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

pub fn client_send_chat(
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

pub fn client_send_input_packets(
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

pub fn client_send_actions(
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

pub fn client_receive_packets(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    client: Option<ResMut<NetClientRes>>,
    mut identity: ResMut<NetworkIdentity>,
    mut lobby: ResMut<LobbyState>,
    mut local_role: ResMut<crate::game::LocalRole>,
    mut game_phase: Option<ResMut<crate::game::GamePhase>>,
    mut active_sab: Option<ResMut<ActiveSabotage>>,
    mut tasks: Option<ResMut<TaskBoard>>,
    mut meeting: Option<ResMut<MeetingState>>,
    mut snapshot_seq: Option<ResMut<ClientSnapshotSequence>>,
    mut chat: ResMut<ChatState>,
    time: Res<Time<Real>>,
    mut replica_players: Query<(
        Entity,
        &ReplicaPlayer,
        &mut Transform,
        &mut Sprite,
        &mut ReplicaInterpolation,
        Option<&Alive>,
        Option<&Ghost>,
    )>,
    replica_bodies: Query<(Entity, &ReplicaBody)>,
    mut local_state: Query<
        (
            Entity,
            Option<&mut KillCooldownLeft>,
            Option<&mut EmergenciesLeft>,
        ),
        With<LocalPlayer>,
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
                        .insert(KillCooldownLeft(p.kill_cooldown));
                }
                if let Some(mut em) = em_opt {
                    em.0 = p.emergencies_left;
                } else {
                    commands
                        .entity(entity)
                        .insert(EmergenciesLeft(p.emergencies_left));
                }
            }
        }

        let now = time.elapsed_secs_f64();
        let mut seen_players = Vec::new();

        for state in players {
            seen_players.push(state.player_id);

            if let Some((entity, _, _, mut sprite, mut interp, alive, ghost)) = replica_players
                .iter_mut()
                .find(|(_, marker, ..)| marker.player_id == state.player_id)
            {
                interp.push_sample(now, Vec2::new(state.position[0], state.position[1]));

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

            let position = Vec2::new(state.position[0], state.position[1]);
            let color = crate::game::PLAYER_COLORS
                [state.color_index as usize % crate::game::PLAYER_COLORS.len()];

            let mut entity = commands.spawn((
                crate::game::MatchCleanup,
                ReplicaPlayer {
                    player_id: state.player_id,
                },
                ReplicaInterpolation::with_initial(now, position),
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
                Transform::from_xyz(position.x, position.y, 10.0),
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

        for (entity, marker, _, _, _, _, _) in &mut replica_players {
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

pub fn interpolate_replicas(
    time: Res<Time<Real>>,
    mut replicas: Query<(&mut Transform, &ReplicaInterpolation), With<ReplicaPlayer>>,
) {
    let render_time = time.elapsed_secs_f64() - INTERPOLATION_DELAY;
    for (mut transform, interp) in &mut replicas {
        if let Some(position) = sample_position(&interp.samples, render_time) {
            transform.translation.x = position.x;
            transform.translation.y = position.y;
        }
    }
}
