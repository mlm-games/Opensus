use std::net::{SocketAddr, UdpSocket};

use bevy::prelude::*;
use renet2::{RenetClient, RenetServer};
use renet2_netcode::{
    ClientAuthentication, NativeSocket, NetcodeClientTransport, NetcodeServerTransport,
    ServerAuthentication, ServerSetupConfig,
};

use crate::game::RuntimeMode;

use super::PendingNetworkStart;
use super::channels::connection_config;
use super::common::{
    NetClientRes, NetClientTransportRes, NetServerRes, NetServerTransportRes, now_duration,
};
use super::protocol::PROTOCOL_ID;

pub fn bootstrap_network(
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
