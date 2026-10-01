use bevy_ecs::prelude::*;
use bevy_ecs::world::World;
use glam::Vec2;
use log::warn;

use crate::app::AppState;
use crate::game::{
    ActiveSabotage, Alive, Body, ChatEntry, ChatState, EmergenciesLeft, GamePhase, Ghost,
    KillCooldownLeft, KillRequests, LobbySlot, LobbyState, LocalPlayer, LocalPlayerId, LocalRole,
    MatchCleanup, MatchConfig, MeetingCommand, MeetingCommands, MeetingState, OutgoingChat, Player,
    PlayerIntent, Position, ReportBodies, RuntimeMode, SabotageFixStation, SabotageRequests,
    SimTimer, SolidAabb, StateRequest, TaskBoard, TimerMode, VoteOption,
};

use super::channels::*;
use super::common::{
    ClientPredictionState, ClientReconciliation, ClientSnapshotSequence, FrameTime,
    INTERPOLATION_DELAY, NetClientRes, NetworkIdentity, ReconciliationSample, ReplicaBody,
    ReplicaInterpolation, ReplicaPlayer, sample_position, sequence_is_newer,
};
use super::protocol::*;

pub fn client_send_hello_once(
    client: Option<ResMut<NetClientRes>>,
    mut identity: ResMut<NetworkIdentity>,
    save: Res<crate::save::SaveData>,
    state: Res<AppState>,
) {
    if identity.hello_sent || *state != AppState::Lobby {
        return;
    }
    let Some(mut client) = client else { return };
    let packet = ClientPacket::Hello {
        protocol_version: PROTOCOL_VERSION,
        name: save.player_name.clone(),
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
    mut last_sent: Local<Option<bool>>,
    state: Res<AppState>,
) {
    if *state != AppState::Lobby {
        return;
    }
    let Some(mut client) = client else { return };
    if !client.0.is_connected() {
        return;
    }
    if *last_sent == Some(lobby.local_ready) {
        return;
    }
    if let Ok(bytes) = bincode::serialize(&ClientPacket::Ready {
        ready: lobby.local_ready,
    }) {
        client.0.send_message(C2S_RELIABLE, bytes);
        *last_sent = Some(lobby.local_ready);
    }
}

pub fn client_send_chat(
    mode: Res<RuntimeMode>,
    mut outgoing: ResMut<OutgoingChat>,
    client: Option<ResMut<NetClientRes>>,
) {
    if !mode.is_remote_client() || outgoing.0.is_empty() {
        return;
    }
    let texts = std::mem::take(&mut outgoing.0);
    let Some(mut client) = client else { return };
    for text in texts {
        if let Ok(bytes) = bincode::serialize(&ClientPacket::Chat { text }) {
            client.0.send_message(C2S_RELIABLE, bytes);
        }
    }
}

pub fn client_send_actions(
    mode: Res<RuntimeMode>,
    mut kills: ResMut<KillRequests>,
    mut reports: ResMut<ReportBodies>,
    mut meetings: ResMut<MeetingCommands>,
    mut sabotage: ResMut<SabotageRequests>,
    client: Option<ResMut<NetClientRes>>,
) {
    if !mode.is_remote_client() {
        return;
    }
    let kills = std::mem::take(&mut kills.0);
    let reports = std::mem::take(&mut reports.0);
    let meetings = std::mem::take(&mut meetings.0);
    let sabotage = std::mem::take(&mut sabotage.0);
    if kills.is_empty() && reports.is_empty() && meetings.is_empty() && sabotage.is_empty() {
        return;
    }
    let Some(mut client) = client else { return };

    for _ in kills {
        if let Ok(bytes) = bincode::serialize(&ClientPacket::Kill) {
            client.0.send_message(C2S_RELIABLE, bytes);
        }
    }
    for _ in reports {
        if let Ok(bytes) = bincode::serialize(&ClientPacket::Report) {
            client.0.send_message(C2S_RELIABLE, bytes);
        }
    }
    for command in meetings {
        let packet = match command {
            MeetingCommand::Emergency { .. } => ClientPacket::Emergency,
            MeetingCommand::Vote { target, .. } => ClientPacket::Vote {
                target: Some(target),
            },
            MeetingCommand::Skip { .. } => ClientPacket::Vote { target: None },
        };
        if let Ok(bytes) = bincode::serialize(&packet) {
            client.0.send_message(C2S_RELIABLE, bytes);
        }
    }
    for request in sabotage {
        if let Ok(bytes) = bincode::serialize(&ClientPacket::Sabotage { kind: request.kind }) {
            client.0.send_message(C2S_RELIABLE, bytes);
        }
    }
}

pub fn client_send_input_packets(
    client: Option<ResMut<NetClientRes>>,
    prediction: Res<ClientPredictionState>,
    state: Res<AppState>,
) {
    if *state != AppState::InGame {
        return;
    }
    let Some(mut client) = client else { return };
    if !client.0.is_connected() {
        return;
    }
    let commands = prediction.send_batch();
    if commands.is_empty() {
        return;
    }
    if let Ok(bytes) = bincode::serialize(&ClientPacket::Input { commands }) {
        client.0.send_message(C2S_INPUT, bytes);
    }
}

pub fn reconcile_local_prediction(
    mut reconciliation: ResMut<ClientReconciliation>,
    mut prediction: ResMut<ClientPredictionState>,
    config: Res<MatchConfig>,
    time: Res<repame_sim::SimTime>,
    solids: Query<(&Position, &SolidAabb), Without<Player>>,
    mut local: Query<(&mut Position, &Player, Option<&Alive>, Option<&Ghost>), With<LocalPlayer>>,
) {
    let Some(sample) = reconciliation.pending.take() else {
        return;
    };
    prediction.acknowledge(sample.acknowledged_input_sequence);

    let Ok((mut position, player, alive, ghost)) = local.single_mut() else {
        return;
    };

    let boxes = crate::game::collision::solid_boxes(&solids);
    position.0 = sample.authoritative_position;

    for command in &prediction.pending {
        let movement = Vec2::new(command.movement[0], command.movement[1]).clamp_length_max(1.0);
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
}

pub fn predict_local_player(
    time: Res<repame_sim::SimTime>,
    phase: Res<GamePhase>,
    config: Res<MatchConfig>,
    mut identity: ResMut<NetworkIdentity>,
    mut prediction: ResMut<ClientPredictionState>,
    solids: Query<(&Position, &SolidAabb), Without<Player>>,
    mut local: Query<
        (
            &Player,
            &PlayerIntent,
            &mut Position,
            Option<&Alive>,
            Option<&Ghost>,
        ),
        With<LocalPlayer>,
    >,
) {
    if !matches!(*phase, GamePhase::Playing) {
        return;
    }
    let Ok((player, intent, mut position, alive, ghost)) = local.single_mut() else {
        return;
    };

    identity.input_sequence = identity.input_sequence.wrapping_add(1);
    prediction.push(NetInputCommand {
        sequence: identity.input_sequence,
        movement: [intent.movement.x, intent.movement.y],
        interact: intent.interact,
    });

    let boxes = crate::game::collision::solid_boxes(&solids);
    let speed = if ghost.is_some() {
        player.speed * config.ghost_speed_mul
    } else {
        player.speed
    };
    position.0 = crate::game::collision::step_player_position(
        position.0,
        intent.movement,
        speed,
        time.delta_secs,
        alive.is_some(),
        &boxes,
    );
}

pub fn client_receive_reliable(world: &mut World) {
    let Some(mut client) = world.remove_resource::<NetClientRes>() else {
        return;
    };
    let mut packets = Vec::new();
    while let Some(bytes) = client.0.receive_message(S2C_RELIABLE) {
        if bytes.len() > 4096 {
            continue;
        }
        if let Ok(packet) = bincode::deserialize::<ServerPacket>(&bytes) {
            packets.push(packet);
        }
    }
    world.insert_resource(client);

    for packet in packets {
        if matches!(packet, ServerPacket::WorldSnapshot { .. }) {
            apply_world_snapshot(world, packet);
            continue;
        }
        match packet {
            ServerPacket::Welcome { player_id } => {
                world.resource_mut::<NetworkIdentity>().my_player_id = Some(player_id);
            }
            ServerPacket::LobbySnapshot { players } => {
                let my_id = world.resource::<NetworkIdentity>().my_player_id;
                let mut lobby = world.resource_mut::<LobbyState>();
                lobby.is_host = false;
                lobby.slots = players
                    .into_iter()
                    .map(|player| LobbySlot {
                        id: player.player_id,
                        name: player.name,
                        color_index: player.color_index,
                        ready: player.ready,
                        is_local: Some(player.player_id) == my_id,
                        is_host: player.is_host,
                        is_bot: false,
                    })
                    .collect();
            }
            ServerPacket::MatchStarted { your_role } => {
                world.resource_mut::<LocalRole>().0 = Some(your_role);
                if *world.resource::<AppState>() != AppState::InGame {
                    world.resource_mut::<StateRequest>().0 = Some(AppState::InGame);
                }
            }
            ServerPacket::Chat {
                player_id,
                name,
                text,
                ghost,
            } => {
                world.resource_mut::<ChatState>().push(ChatEntry {
                    player_id,
                    name,
                    text,
                    ghost,
                });
            }
            ServerPacket::Rejected { reason } => {
                warn!("server rejected connection: {reason}");
            }
            ServerPacket::WorldSnapshot { .. } => unreachable!(),
        }
    }
}

pub fn client_receive_snapshots(world: &mut World) {
    let Some(mut client) = world.remove_resource::<NetClientRes>() else {
        return;
    };
    let mut packets = Vec::new();
    while let Some(bytes) = client.0.receive_message(S2C_SNAPSHOT) {
        if bytes.len() > 64 * 1024 {
            continue;
        }
        if let Ok(packet) = bincode::deserialize::<ServerPacket>(&bytes) {
            packets.push(packet);
        }
    }
    world.insert_resource(client);

    for packet in packets {
        apply_world_snapshot(world, packet);
    }
}

fn apply_world_snapshot(world: &mut World, packet: ServerPacket) {
    let ServerPacket::WorldSnapshot {
        sequence,
        players,
        bodies,
        phase,
        sabotage,
        tasks_completed,
        tasks_total,
        task_states: _,
        fix_station_states,
        meeting_prompt,
        meeting_timer,
        vote_options,
        result_text,
        private,
    } = packet
    else {
        return;
    };

    if *world.resource::<AppState>() != AppState::InGame {
        return;
    }

    {
        let mut seq = world.resource_mut::<ClientSnapshotSequence>();
        if let Some(previous) = seq.last_applied
            && !sequence_is_newer(sequence, previous)
        {
            return;
        }
        seq.last_applied = Some(sequence);
    }

    let my_id = world.resource::<NetworkIdentity>().my_player_id;
    if let Some(local_id) = my_id
        && let Some(state) = players.iter().find(|p| p.player_id == local_id)
    {
        world.resource_mut::<ClientReconciliation>().pending = Some(ReconciliationSample {
            authoritative_position: Vec2::new(state.position[0], state.position[1]),
            acknowledged_input_sequence: private
                .as_ref()
                .and_then(|p| p.acknowledged_input_sequence),
        });
    }

    *world.resource_mut::<GamePhase>() = phase;

    {
        let mut active = world.resource_mut::<ActiveSabotage>();
        if let Some(state) = sabotage {
            active.kind = Some(state.kind);
            active.fixes_needed = state.fixes_needed;
            active.fixes_done = state.fixes_done;
            active.timer = (state.remaining > 0.0)
                .then(|| SimTimer::from_seconds(state.remaining, TimerMode::Once));
        } else {
            active.clear();
        }
    }

    {
        let mut board = world.resource_mut::<TaskBoard>();
        board.completed = tasks_completed;
        board.total = tasks_total;
    }

    {
        let mut fix_stations = world.query::<&mut SabotageFixStation>();
        for state in &fix_station_states {
            for mut station in fix_stations.iter_mut(world) {
                if station.id == state.id {
                    station.kind = state.kind;
                    station.progress = state.progress.clamp(0.0, 1.0);
                }
            }
        }
    }

    {
        let mut meeting = world.resource_mut::<MeetingState>();
        meeting.prompt = meeting_prompt;
        meeting.timer = SimTimer::from_seconds(meeting_timer.max(0.1), TimerMode::Once);
        meeting.options = vote_options
            .into_iter()
            .map(|(player_id, name, dead)| VoteOption {
                player_id,
                name,
                dead,
            })
            .collect();
        meeting.result_text = result_text;
        if let Some(p) = &private {
            meeting.local_voted = p.voted;
            meeting.tallies = p.vote_tallies.clone();
        }
    }

    if let Some(p) = private {
        world.resource_mut::<LocalRole>().0 = Some(p.role);

        let local = {
            let mut query = world.query_filtered::<
                (Entity, Has<KillCooldownLeft>, Has<EmergenciesLeft>),
                With<LocalPlayer>,
            >();
            query.single(world).ok()
        };
        if let Some((entity, has_cd, has_em)) = local {
            if has_cd {
                if let Some(mut cd) = world.get_mut::<KillCooldownLeft>(entity) {
                    cd.0 = p.kill_cooldown;
                }
            } else if p.kill_cooldown > 0.0 {
                world
                    .entity_mut(entity)
                    .insert(KillCooldownLeft(p.kill_cooldown));
            }
            if has_em {
                if let Some(mut em) = world.get_mut::<EmergenciesLeft>(entity) {
                    em.0 = p.emergencies_left;
                }
            } else {
                world
                    .entity_mut(entity)
                    .insert(EmergenciesLeft(p.emergencies_left));
            }
            world.entity_mut(entity).insert(p.role);
        }
    }

    let now = world.resource::<FrameTime>().elapsed;
    let config_speed = world.resource::<MatchConfig>().player_speed;
    let mut seen = Vec::with_capacity(players.len());
    let mut player_q = world.query::<(Entity, &Player)>();

    for state in players {
        seen.push(state.player_id);
        let position = Vec2::new(state.position[0], state.position[1]);
        let existing = player_q
            .iter(world)
            .find(|(_, p)| p.id == state.player_id)
            .map(|(entity, _)| entity);

        let Some(entity) = existing else {
            let mut spawned = world.spawn((
                MatchCleanup,
                ReplicaPlayer {
                    player_id: state.player_id,
                },
                ReplicaInterpolation::with_initial(now, position),
                Player {
                    id: state.player_id,
                    name: state.name.clone(),
                    color_index: state.color_index,
                    speed: config_speed,
                },
                PlayerIntent::default(),
                Position(position),
            ));
            if state.alive {
                spawned.insert(Alive);
            } else {
                spawned.insert(Ghost);
            }
            if my_id == Some(state.player_id) {
                spawned.insert(LocalPlayer);
                world.insert_resource(LocalPlayerId(Some(state.player_id)));
            }
            continue;
        };

        if my_id == Some(state.player_id) {
            if let Some(mut interp) = world.get_mut::<ReplicaInterpolation>(entity) {
                interp.samples.clear();
            }
        } else {
            match world.get_mut::<ReplicaInterpolation>(entity) {
                Some(mut interp) => interp.push_sample(now, position),
                None => {
                    world
                        .entity_mut(entity)
                        .insert(ReplicaInterpolation::with_initial(now, position));
                }
            }
        }

        let (has_alive, has_ghost) = {
            let e = world.entity(entity);
            (e.contains::<Alive>(), e.contains::<Ghost>())
        };
        if state.alive && !has_alive {
            world.entity_mut(entity).insert(Alive);
        } else if !state.alive && has_alive {
            world.entity_mut(entity).remove::<Alive>();
        }
        if !state.alive && !has_ghost {
            world.entity_mut(entity).insert(Ghost);
        } else if state.alive && has_ghost {
            world.entity_mut(entity).remove::<Ghost>();
        }
    }

    let to_despawn: Vec<Entity> = player_q
        .iter(world)
        .filter(|(_, p)| !seen.contains(&p.id))
        .map(|(entity, _)| entity)
        .collect();
    for entity in to_despawn {
        let _ = world.despawn(entity);
    }

    let mut seen_bodies = Vec::with_capacity(bodies.len());
    let mut body_q = world.query::<(Entity, &ReplicaBody)>();
    for body in bodies {
        seen_bodies.push(body.body_id);
        if body_q
            .iter(world)
            .any(|(_, marker)| marker.body_id == body.body_id)
        {
            continue;
        }
        world.spawn((
            MatchCleanup,
            ReplicaBody {
                body_id: body.body_id,
            },
            Body {
                player_id: body.player_id,
                name: body.name.clone(),
                reported: body.reported,
            },
            Position(Vec2::new(body.position[0], body.position[1])),
        ));
    }
    let stale_bodies: Vec<Entity> = body_q
        .iter(world)
        .filter(|(_, marker)| !seen_bodies.contains(&marker.body_id))
        .map(|(entity, _)| entity)
        .collect();
    for entity in stale_bodies {
        let _ = world.despawn(entity);
    }
}

pub fn interpolate_replicas(world: &mut World) {
    if !matches!(*world.resource::<RuntimeMode>(), RuntimeMode::Client) {
        return;
    }
    let now = world.resource::<FrameTime>().elapsed;
    let render_time = now - INTERPOLATION_DELAY;
    let mut query =
        world.query_filtered::<(&mut Position, &ReplicaInterpolation), Without<LocalPlayer>>();
    for (mut position, interp) in query.iter_mut(world) {
        if let Some(sampled) = sample_position(&interp.samples, render_time) {
            position.0 = sampled;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy_ecs::schedule::Schedule;

    use super::*;
    use crate::game::{Role, SabotageKind};

    fn base_world() -> World {
        let mut world = World::new();
        world.insert_resource(AppState::InGame);
        world.insert_resource(GamePhase::None);
        world.init_resource::<ActiveSabotage>();
        world.init_resource::<TaskBoard>();
        world.init_resource::<MeetingState>();
        world.init_resource::<ClientSnapshotSequence>();
        world.init_resource::<ClientReconciliation>();
        world.init_resource::<LocalRole>();
        world.init_resource::<MatchConfig>();
        world.insert_resource(FrameTime {
            delta: std::time::Duration::ZERO,
            elapsed: 9.55,
        });
        world.insert_resource(NetworkIdentity {
            my_player_id: Some(1),
            hello_sent: true,
            input_sequence: 0,
        });
        world.spawn((
            LocalPlayer,
            Player {
                id: 1,
                name: "Me".into(),
                color_index: 0,
                speed: 220.0,
            },
            Position(Vec2::ZERO),
            Alive,
            EmergenciesLeft(2),
        ));
        world.spawn((
            Player {
                id: 2,
                name: "Other".into(),
                color_index: 1,
                speed: 220.0,
            },
            Position(Vec2::ZERO),
            Alive,
        ));
        world.spawn((
            SabotageFixStation {
                id: 0,
                kind: SabotageKind::Lights,
                progress: 0.0,
            },
            Position(Vec2::ZERO),
        ));
        world
    }

    fn snapshot(sequence: u32, phase: GamePhase) -> ServerPacket {
        ServerPacket::WorldSnapshot {
            sequence,
            players: vec![
                NetPlayerState {
                    player_id: 1,
                    name: "Me".into(),
                    color_index: 0,
                    position: [5.0, 6.0],
                    alive: false,
                },
                NetPlayerState {
                    player_id: 2,
                    name: "Other".into(),
                    color_index: 1,
                    position: [10.0, 20.0],
                    alive: true,
                },
            ],
            bodies: vec![NetBodyState {
                body_id: 1,
                player_id: 3,
                name: "Fallen".into(),
                position: [4.0, 4.0],
                reported: false,
            }],
            phase,
            sabotage: Some(NetSabotageState {
                kind: SabotageKind::Oxygen,
                remaining: 12.0,
                fixes_needed: 2,
                fixes_done: 1,
            }),
            tasks_completed: 3,
            tasks_total: 4,
            task_states: vec![],
            fix_station_states: vec![NetFixStationState {
                id: 0,
                kind: SabotageKind::Oxygen,
                progress: 0.5,
            }],
            meeting_prompt: "Who sus?".into(),
            meeting_timer: 18.0,
            vote_options: vec![(1, "Me".into(), false)],
            result_text: String::new(),
            private: Some(PrivatePlayerState {
                kill_cooldown: 5.0,
                emergencies_left: 1,
                role: Role::Impostor,
                voted: true,
                vote_tallies: vec![("Me".into(), 2)],
                acknowledged_input_sequence: Some(3),
            }),
        }
    }

    fn find_player(world: &mut World, id: u64) -> Entity {
        let mut query = world.query::<(Entity, &Player)>();
        query
            .iter(world)
            .find(|(_, p)| p.id == id)
            .map(|(e, _)| e)
            .unwrap()
    }

    #[test]
    fn snapshot_mirrors_state_onto_world() {
        let mut world = base_world();
        apply_world_snapshot(&mut world, snapshot(7, GamePhase::Meeting));

        assert!(matches!(*world.resource::<GamePhase>(), GamePhase::Meeting));
        assert_eq!(
            world.resource::<ClientSnapshotSequence>().last_applied,
            Some(7)
        );
        assert_eq!(world.resource::<LocalRole>().0, Some(Role::Impostor));
        assert_eq!(world.resource::<TaskBoard>().completed, 3);
        {
            let active = world.resource::<ActiveSabotage>();
            assert_eq!(active.kind, Some(SabotageKind::Oxygen));
            assert_eq!(active.fixes_done, 1);
        }
        {
            let meeting = world.resource::<MeetingState>();
            assert_eq!(meeting.prompt, "Who sus?");
            assert!(meeting.local_voted);
            assert_eq!(meeting.tallies, vec![("Me".into(), 2)]);
            assert_eq!(meeting.options.len(), 1);
        }

        let local = find_player(&mut world, 1);
        assert!(!world.get::<Alive>(local).is_some());
        assert!(world.get::<Ghost>(local).is_some());
        assert_eq!(world.get::<KillCooldownLeft>(local).unwrap().0, 5.0);
        assert_eq!(world.get::<EmergenciesLeft>(local).unwrap().0, 1);
        assert!(matches!(*world.get::<Role>(local).unwrap(), Role::Impostor));

        let sample = world
            .resource::<ClientReconciliation>()
            .pending
            .expect("local reconciliation sample");
        assert_eq!(sample.authoritative_position, Vec2::new(5.0, 6.0));
        assert_eq!(sample.acknowledged_input_sequence, Some(3));

        let other = find_player(&mut world, 2);
        let interp = world.get::<ReplicaInterpolation>(other).expect("interp");
        assert_eq!(interp.samples.len(), 1);
        assert_eq!(
            interp.samples.back().unwrap().position,
            Vec2::new(10.0, 20.0)
        );
        assert_eq!(world.get::<Position>(other).unwrap().0, Vec2::ZERO);

        let bodies = world
            .query_filtered::<&Body, With<ReplicaBody>>()
            .iter(&world)
            .count();
        assert_eq!(bodies, 1);

        let fix = world
            .query::<&SabotageFixStation>()
            .iter(&world)
            .next()
            .expect("fix station");
        assert_eq!(fix.progress, 0.5);
    }

    #[test]
    fn stale_snapshot_sequence_is_rejected() {
        let mut world = base_world();
        apply_world_snapshot(&mut world, snapshot(9, GamePhase::Playing));
        apply_world_snapshot(
            &mut world,
            snapshot(
                5,
                GamePhase::GameOver {
                    crew_win: true,
                    reason: crate::game::WinReason::Tasks,
                },
            ),
        );

        assert!(matches!(*world.resource::<GamePhase>(), GamePhase::Playing));
        assert_eq!(
            world.resource::<ClientSnapshotSequence>().last_applied,
            Some(9)
        );
    }

    #[test]
    fn reconcile_rolls_back_and_replays_pending() {
        let mut world = World::new();
        world.insert_resource(GamePhase::Playing);
        world.init_resource::<MatchConfig>();
        world.insert_resource(repame_sim::SimTime {
            elapsed_secs: 0.0,
            delta_secs: 1.0 / 60.0,
        });
        world.insert_resource(ClientPredictionState::default());
        world.insert_resource(ClientReconciliation {
            pending: Some(ReconciliationSample {
                authoritative_position: Vec2::new(50.0, 10.0),
                acknowledged_input_sequence: Some(2),
            }),
        });
        world.spawn((
            LocalPlayer,
            Player {
                id: 1,
                name: "Me".into(),
                color_index: 0,
                speed: 200.0,
            },
            Position(Vec2::ZERO),
            Alive,
        ));
        {
            let mut prediction = world.resource_mut::<ClientPredictionState>();
            for sequence in 1..=3 {
                prediction.push(NetInputCommand {
                    sequence,
                    movement: [1.0, 0.0],
                    interact: false,
                });
            }
        }

        let mut schedule = Schedule::default();
        schedule.add_systems(reconcile_local_prediction);
        schedule.run(&mut world);

        let position = world
            .query_filtered::<&Position, With<LocalPlayer>>()
            .single(&world)
            .unwrap()
            .0;
        assert!((position.x - 53.33).abs() < 0.1);
        assert_eq!(position.y, 10.0);
        assert_eq!(
            world
                .resource::<ClientPredictionState>()
                .pending
                .iter()
                .map(|c| c.sequence)
                .collect::<Vec<_>>(),
            vec![3]
        );
        assert!(world.resource::<ClientReconciliation>().pending.is_none());
    }

    #[test]
    fn interpolate_lerps_remote_between_samples() {
        let mut world = base_world();
        world.insert_resource(RuntimeMode::Client);
        let other = find_player(&mut world, 2);
        world
            .entity_mut(other)
            .insert(ReplicaInterpolation::default());
        {
            let mut interp = world.get_mut::<ReplicaInterpolation>(other).unwrap();
            interp.samples.clear();
            interp.push_sample(9.0, Vec2::ZERO);
            interp.push_sample(9.5, Vec2::new(100.0, 0.0));
        }

        interpolate_replicas(&mut world);

        let position = world.get::<Position>(other).unwrap().0;
        assert!((position.x - 90.0).abs() < 0.5);
        assert_eq!(position.y, 0.0);

        let local = find_player(&mut world, 1);
        assert_eq!(world.get::<Position>(local).unwrap().0, Vec2::ZERO);
    }
}
