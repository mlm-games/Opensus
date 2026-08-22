use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime};

use bevy::prelude::*;
use renet2::{ClientId, RenetClient, RenetServer};
use renet2_netcode::{NetcodeClientTransport, NetcodeServerTransport};

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
    pub authenticated_clients: HashSet<ClientId>,
}

#[derive(Resource, Default)]
pub struct ServerSnapshotSequence(pub u32);

#[derive(Resource, Default)]
pub struct ClientSnapshotSequence {
    pub last_applied: Option<u32>,
}

pub fn sequence_is_newer(sequence: u32, previous: u32) -> bool {
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

pub fn now_duration() -> Duration {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
}
