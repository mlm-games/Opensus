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
mod client;
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
    client::interpolate_replicas(world);
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
        gameplay_active, has_authority, local_intent_and_move, read_local_action_edges,
    };
    use bevy_ecs::schedule::ApplyDeferred;

    fn remote_client(mode: Res<RuntimeMode>) -> bool {
        mode.is_remote_client()
    }

    schedule.configure_sets(
        GameSimSet::Receive
            .before(GameSimSet::Input)
            .before(GameSimSet::Send),
    );
    schedule.configure_sets(GameSimSet::Send.after(GameSimSet::Win));
    schedule.add_systems(
        (
            host::host_handle_connects_and_disconnects,
            client::client_send_hello_once,
            client::client_send_ready,
            host::host_receive_reliable_packets,
            host::host_receive_input_packets,
            client::client_receive_reliable,
            client::client_receive_snapshots,
        )
            .chain()
            .in_set(GameSimSet::Receive),
    );
    schedule.add_systems(
        ApplyDeferred
            .after(GameSimSet::Receive)
            .before(GameSimSet::Input),
    );
    schedule.add_systems(
        client::reconcile_local_prediction
            .in_set(GameSimSet::Input)
            .run_if(gameplay_active)
            .run_if(remote_client)
            .before(local_intent_and_move),
    );
    schedule.add_systems(
        client::predict_local_player
            .in_set(GameSimSet::Input)
            .run_if(gameplay_active)
            .run_if(remote_client)
            .after(local_intent_and_move),
    );
    schedule.add_systems(
        client::client_send_input_packets
            .in_set(GameSimSet::Input)
            .run_if(gameplay_active)
            .run_if(remote_client)
            .after(client::predict_local_player),
    );
    schedule.add_systems(
        client::client_send_actions
            .in_set(GameSimSet::Input)
            .run_if(gameplay_active)
            .run_if(remote_client)
            .after(read_local_action_edges),
    );
    schedule.add_systems(
        host::host_relay_local_chat
            .in_set(GameSimSet::Input)
            .run_if(gameplay_active)
            .after(capture_chat_text)
            .before(apply_authority_chat),
    );
    schedule.add_systems(
        client::client_send_chat
            .in_set(GameSimSet::Input)
            .run_if(gameplay_active)
            .run_if(remote_client)
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

#[cfg(all(test, feature = "networking-native", not(target_arch = "wasm32")))]
mod integration_tests {
    use std::time::Duration;

    use crate::app::{App, AppState, goto_state};
    use crate::game::{
        GamePhase, LobbySlot, LobbyState, LocalPlayer, LocalPlayerId, LocalRole,
        PendingNetworkStart, Player, Role, RuntimeMode,
    };

    use super::*;

    const ADDR: &str = "127.0.0.1:47311";

    fn frame(app: &mut App, dt: Duration) {
        pre_frame(&mut app.sim.world, dt);
        app.advance(dt);
        post_frame(&mut app.sim.world);
    }

    fn slot_names(app: &App) -> Vec<String> {
        app.sim
            .world
            .resource::<LobbyState>()
            .slots
            .iter()
            .map(|slot| format!("{}(ready={})", slot.name, slot.ready))
            .collect()
    }

    fn player_count(app: &mut App) -> usize {
        app.sim
            .world
            .query::<&Player>()
            .iter(&app.sim.world)
            .count()
    }

    #[test]
    fn host_and_client_handshake_match_start_and_snapshot_sync() {
        let dt = Duration::from_millis(17);

        let mut host = App::new();
        *host.sim.world.resource_mut::<RuntimeMode>() = RuntimeMode::Host;
        *host.sim.world.resource_mut::<PendingNetworkStart>() = PendingNetworkStart::HostLocal {
            bind_addr: ADDR.into(),
        };
        goto_state(&mut host.sim.world, AppState::Lobby);
        for i in 0..2u64 {
            host.sim
                .world
                .resource_mut::<LobbyState>()
                .slots
                .push(LobbySlot {
                    id: 10 + i,
                    name: format!("Bot-{i}"),
                    color_index: (2 + i) as u8,
                    ready: true,
                    is_local: false,
                    is_host: false,
                    is_bot: true,
                });
        }

        let mut client = App::new();
        *client.sim.world.resource_mut::<RuntimeMode>() = RuntimeMode::Client;
        *client.sim.world.resource_mut::<PendingNetworkStart>() = PendingNetworkStart::JoinLocal {
            server_addr: ADDR.into(),
        };
        goto_state(&mut client.sim.world, AppState::Lobby);

        let mut bootstrapped = false;
        for _ in 0..60 {
            frame(&mut host, dt);
            frame(&mut client, dt);
            if host.sim.world.get_resource::<NetServerRes>().is_some()
                && client.sim.world.get_resource::<NetClientRes>().is_some()
            {
                bootstrapped = true;
                break;
            }
        }
        assert!(
            bootstrapped,
            "network bootstrap failed (port {ADDR} busy or init error)"
        );

        let mut synced = false;
        for _ in 0..600 {
            frame(&mut host, dt);
            frame(&mut client, dt);
            synced = host.sim.world.resource::<LobbyState>().slots.len() >= 2
                && client.sim.world.resource::<LobbyState>().slots.len() >= 2;
            if synced {
                break;
            }
        }
        assert!(
            synced,
            "lobby slots never synced: host={:?} client={:?}",
            slot_names(&host),
            slot_names(&client)
        );

        host.sim.world.resource_mut::<LobbyState>().local_ready = true;
        client.sim.world.resource_mut::<LobbyState>().local_ready = true;

        let mut start_allowed = false;
        for _ in 0..600 {
            frame(&mut host, dt);
            frame(&mut client, dt);
            if crate::game::handle_start_match(&host.sim.world) {
                start_allowed = true;
                break;
            }
        }
        assert!(
            start_allowed,
            "host start predicate never satisfied: {:?}",
            slot_names(&host)
        );
        host.begin_to_state(AppState::InGame);

        let mut client_ingame = false;
        for _ in 0..600 {
            frame(&mut host, dt);
            frame(&mut client, dt);
            if *client.sim.world.resource::<AppState>() == AppState::InGame {
                client_ingame = true;
                break;
            }
        }
        assert!(
            client_ingame,
            "client never entered InGame via MatchStarted: host_state={:?} client_state={:?} my_id={:?} state_request={:?}",
            *host.sim.world.resource::<AppState>(),
            *client.sim.world.resource::<AppState>(),
            client.sim.world.resource::<NetworkIdentity>().my_player_id,
            client.sim.world.resource::<crate::game::StateRequest>().0,
        );

        let mut playing = false;
        for _ in 0..1500 {
            frame(&mut host, dt);
            frame(&mut client, dt);
            playing = matches!(*host.sim.world.resource::<GamePhase>(), GamePhase::Playing)
                && matches!(
                    *client.sim.world.resource::<GamePhase>(),
                    GamePhase::Playing
                );
            if playing {
                break;
            }
        }
        assert!(
            playing,
            "phases did not converge on Playing: host={:?} client={:?}",
            *host.sim.world.resource::<GamePhase>(),
            *client.sim.world.resource::<GamePhase>()
        );

        assert_eq!(player_count(&mut host), 4, "host player count");
        assert_eq!(player_count(&mut client), 4, "client player count");

        let client_id = client
            .sim
            .world
            .resource::<LocalPlayerId>()
            .0
            .expect("client local player id");
        assert_eq!(
            Some(client_id),
            client.sim.world.resource::<NetworkIdentity>().my_player_id
        );

        let mut local_marked = false;
        let mut remote_interp = false;
        let mut query =
            client
                .sim
                .world
                .query::<(&Player, Option<&LocalPlayer>, Option<&ReplicaInterpolation>)>();
        for (player, local, interp) in query.iter(&client.sim.world) {
            if player.id == client_id {
                local_marked = local.is_some();
            } else {
                remote_interp = interp.is_some();
            }
        }
        assert!(local_marked, "client own player lacks LocalPlayer");
        assert!(remote_interp, "client remote player lacks interpolation");

        let authoritative_role = {
            let mut query = host.sim.world.query::<(&Player, &Role)>();
            query
                .iter(&host.sim.world)
                .find(|(player, _)| player.id == client_id)
                .map(|(_, role)| *role)
        };
        assert_eq!(
            authoritative_role,
            client.sim.world.resource::<LocalRole>().0,
            "client private role out of sync with host"
        );
    }
}
