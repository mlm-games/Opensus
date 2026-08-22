use bevy::prelude::*;

use crate::app::AppState;

use super::common::{
    ClientSnapshotSequence, NetClientRes, NetClientTransportRes, NetServerRes,
    NetServerTransportRes, NetworkIdentity, NetworkMappings, ServerSnapshotSequence,
};

pub fn cleanup_network_on_title(
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

    commands.insert_resource(NetworkIdentity::default());
    commands.insert_resource(NetworkMappings::default());
    commands.insert_resource(ServerSnapshotSequence::default());
    commands.insert_resource(ClientSnapshotSequence::default());
}
