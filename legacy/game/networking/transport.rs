use std::time::Duration;

use bevy::prelude::*;

use super::common::{NetClientRes, NetClientTransportRes, NetServerRes, NetServerTransportRes};

pub fn update_server_transport(
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

pub fn send_server_packets(
    server: Option<ResMut<NetServerRes>>,
    transport: Option<ResMut<NetServerTransportRes>>,
) {
    let (Some(mut server), Some(mut transport)) = (server, transport) else {
        return;
    };
    transport.0.send_packets(&mut server.0);
}

pub fn update_client_transport(
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

pub fn send_client_packets(
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
