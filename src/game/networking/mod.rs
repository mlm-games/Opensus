use std::time::Duration;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;

#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
mod bootstrap;
#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
mod channels;
#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
mod cleanup;
#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
mod common;
#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
mod host;
#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
mod protocol;
#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
mod transport;

#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
pub use common::*;
#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
pub use protocol::*;

#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
pub fn init_resources(world: &mut World) {
    world.init_resource::<common::NetworkIdentity>();
    world.init_resource::<common::NetworkMappings>();
    world.init_resource::<common::ServerSnapshotSequence>();
    world.init_resource::<common::ClientSnapshotSequence>();
    world.init_resource::<common::ClientPredictionState>();
    world.init_resource::<common::ClientReconciliation>();
    world.init_resource::<common::FrameTime>();
    world.insert_resource(common::LobbyBroadcastTimer(
        crate::game::SimTimer::from_seconds(
            1.0 / common::LOBBY_BROADCAST_HZ,
            crate::game::TimerMode::Repeating,
        ),
    ));
    world.insert_resource(common::SnapshotTimer(crate::game::SimTimer::from_seconds(
        1.0 / common::SNAPSHOT_HZ,
        crate::game::TimerMode::Repeating,
    )));
}

#[cfg(not(all(feature = "networking-native", not(target_arch = "wasm32"))))]
pub fn init_resources(_world: &mut World) {}

#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
pub fn pre_frame(world: &mut World, dt: Duration) {
    {
        let mut clock = world.resource_mut::<common::FrameTime>();
        clock.delta = dt;
        clock.elapsed += dt.as_secs_f64();
    }
    bootstrap::bootstrap(world);
    transport::update_transports(world);
}

#[cfg(not(all(feature = "networking-native", not(target_arch = "wasm32"))))]
pub fn pre_frame(_world: &mut World, _dt: Duration) {}

#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
pub fn post_frame(world: &mut World) {
    transport::flush(world);
}

#[cfg(not(all(feature = "networking-native", not(target_arch = "wasm32"))))]
pub fn post_frame(_world: &mut World) {}

#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
pub fn on_enter_title(world: &mut World) {
    cleanup::on_enter_title(world);
}

#[cfg(not(all(feature = "networking-native", not(target_arch = "wasm32"))))]
pub fn on_enter_title(_world: &mut World) {}

#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
pub fn reset_match_state(world: &mut World) {
    world
        .resource_mut::<common::NetworkIdentity>()
        .input_sequence = 0;
    world
        .resource_mut::<common::ClientSnapshotSequence>()
        .last_applied = None;
    world
        .resource_mut::<common::ClientPredictionState>()
        .clear();
    world.resource_mut::<common::ClientReconciliation>().pending = None;
    let mut mappings = world.resource_mut::<common::NetworkMappings>();
    mappings.pending_inputs.clear();
    mappings.last_enqueued_input_sequence.clear();
    mappings.last_processed_input_sequence.clear();
}

#[cfg(not(all(feature = "networking-native", not(target_arch = "wasm32"))))]
pub fn reset_match_state(_world: &mut World) {}

#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
pub fn register_schedule(schedule: &mut Schedule) {
    use super::{
        GameSimSet, ResolveStep, RuntimeMode, ai_brain, apply_authority_chat, capture_chat_text,
        gameplay_active, has_authority,
    };

    schedule.configure_sets(
        GameSimSet::Receive
            .before(GameSimSet::Input)
            .before(GameSimSet::Send),
    );
    schedule.configure_sets(GameSimSet::Send.after(GameSimSet::Win));
    schedule.add_systems(
        (
            host::host_handle_connects_and_disconnects,
            host::host_receive_reliable_packets,
            host::host_receive_input_packets,
        )
            .chain()
            .in_set(GameSimSet::Receive),
    );
    schedule.add_systems(
        host::host_relay_local_chat
            .in_set(GameSimSet::Input)
            .run_if(gameplay_active)
            .after(capture_chat_text)
            .before(apply_authority_chat),
    );
    schedule.add_systems(
        host::apply_remote_input_commands
            .in_set(ResolveStep::Ai)
            .before(ai_brain)
            .run_if(gameplay_active)
            .run_if(has_authority)
            .run_if(|mode: Res<RuntimeMode>| matches!(*mode, RuntimeMode::Host)),
    );
    schedule.add_systems(
        (
            host::host_broadcast_lobby_snapshot,
            host::host_send_match_started,
            host::host_send_world_snapshots,
        )
            .chain()
            .in_set(GameSimSet::Send),
    );
}

#[cfg(not(all(feature = "networking-native", not(target_arch = "wasm32"))))]
pub fn register_schedule(_schedule: &mut Schedule) {}
