use bevy_ecs::prelude::World;

use super::common::{
    ClientSnapshotSequence, NetClientRes, NetClientTransportRes, NetServerRes,
    NetServerTransportRes, NetworkIdentity, NetworkMappings, ServerSnapshotSequence,
};

pub fn on_enter_title(world: &mut World) {
    world.remove_resource::<NetServerRes>();
    world.remove_resource::<NetServerTransportRes>();
    world.remove_resource::<NetClientRes>();
    world.remove_resource::<NetClientTransportRes>();

    *world.resource_mut::<NetworkIdentity>() = NetworkIdentity::default();
    *world.resource_mut::<NetworkMappings>() = NetworkMappings::default();
    *world.resource_mut::<ServerSnapshotSequence>() = ServerSnapshotSequence::default();
    *world.resource_mut::<ClientSnapshotSequence>() = ClientSnapshotSequence::default();
}
