use std::time::Duration;

use bevy_ecs::prelude::*;
use log::warn;

use super::common::{
    FrameTime, NetClientRes, NetClientTransportRes, NetServerRes, NetServerTransportRes,
};

pub fn update_transports(world: &mut World) {
    let Some(delta) = world.get_resource::<FrameTime>().map(|clock| clock.delta) else {
        return;
    };
    update_server_transport(world, delta);
    update_client_transport(world, delta);
}

fn update_server_transport(world: &mut World, delta: Duration) {
    let Some(mut server) = world.remove_resource::<NetServerRes>() else {
        return;
    };
    server.0.update(delta);
    if let Some(mut transport) = world.get_resource_mut::<NetServerTransportRes>()
        && let Err(err) = transport.0.update(delta, &mut server.0)
    {
        warn!("server transport update failed: {err:?}");
    }
    world.insert_resource(server);
}

fn update_client_transport(world: &mut World, delta: Duration) {
    let Some(mut client) = world.remove_resource::<NetClientRes>() else {
        return;
    };
    client.0.update(delta);
    if let Some(mut transport) = world.get_resource_mut::<NetClientTransportRes>()
        && let Err(err) = transport.0.update(delta, &mut client.0)
    {
        warn!("client transport update failed: {err}");
    }
    world.insert_resource(client);
}

pub fn flush(world: &mut World) {
    if let Some(mut server) = world.remove_resource::<NetServerRes>()
        && let Some(mut transport) = world.get_resource_mut::<NetServerTransportRes>()
    {
        transport.0.send_packets(&mut server.0);
        world.insert_resource(server);
    }
    if let Some(mut client) = world.remove_resource::<NetClientRes>()
        && let Some(mut transport) = world.get_resource_mut::<NetClientTransportRes>()
    {
        if let Err(err) = transport.0.send_packets(&mut client.0) {
            warn!("client transport send failed: {err}");
        }
        world.insert_resource(client);
    }
}
