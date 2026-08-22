//! Networking. Native `renet2` / `renet2_netcode` integration behind the
//! `networking-native` feature (disabled on wasm).
//!
//! Server-authoritative: role assignment, kill validation, vote tally,
//! sabotage triggers, task completion all live on the host. Clients send only
//! intent on an unreliable channel and render world snapshots.
//!
//! `renet2` / `renet2_netcode` are used without their `bevy` features so the
//! dependency graph does not pull in a second (0.19) Bevy.

#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
pub mod channels;

#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
pub mod protocol;

#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
mod bootstrap;
#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
mod cleanup;
#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
mod client;
#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
mod common;
#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
mod host;
#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
mod transport;

use bevy::prelude::*;

#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
#[allow(unused_imports)]
pub use bootstrap::*;
#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
#[allow(unused_imports)]
pub use cleanup::*;
#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
#[allow(unused_imports)]
pub use client::*;
#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
#[allow(unused_imports)]
pub use common::*;
#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
#[allow(unused_imports)]
pub use host::*;
#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
#[allow(unused_imports)]
pub use transport::*;

#[derive(Resource, Default, Clone, Debug)]
#[allow(
    dead_code,
    reason = "Fields are consumed by the networking-native systems"
)]
pub enum PendingNetworkStart {
    #[default]
    None,
    HostLocal {
        bind_addr: String,
    },
    JoinLocal {
        server_addr: String,
    },
}

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingNetworkStart>();

        #[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
        app.add_plugins(NativeNetworkingPlugin);
    }
}

#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum NativeNetSet {
    Bootstrap,
    ReceiveTransport,
    ReceivePackets,
    SendPackets,
    FlushTransport,
}

#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
pub struct NativeNetworkingPlugin;

#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
impl Plugin for NativeNetworkingPlugin {
    fn build(&self, app: &mut App) {
        use crate::app::AppState;
        use crate::game::RuntimeMode;
        use crate::game::networking::common::{
            ClientPredictionState, ClientReconciliation, ClientSnapshotSequence, NetworkIdentity,
            NetworkMappings, ServerSnapshotSequence,
        };
        use crate::game::networking::common::{LobbyBroadcastTimer, SnapshotTimer};

        const SNAPSHOT_HZ: f32 = 20.0;
        const LOBBY_BROADCAST_HZ: f32 = 5.0;

        app.init_resource::<NetworkIdentity>()
            .init_resource::<NetworkMappings>()
            .init_resource::<ServerSnapshotSequence>()
            .init_resource::<ClientSnapshotSequence>()
            .init_resource::<ClientPredictionState>()
            .init_resource::<ClientReconciliation>()
            .insert_resource(Time::<Fixed>::from_hz(common::PREDICTION_HZ))
            .insert_resource(LobbyBroadcastTimer(Timer::from_seconds(
                1.0 / LOBBY_BROADCAST_HZ,
                TimerMode::Repeating,
            )))
            .insert_resource(SnapshotTimer(Timer::from_seconds(
                1.0 / SNAPSHOT_HZ,
                TimerMode::Repeating,
            )))
            .configure_sets(
                PreUpdate,
                (
                    NativeNetSet::Bootstrap,
                    NativeNetSet::ReceiveTransport,
                    NativeNetSet::ReceivePackets,
                )
                    .chain(),
            )
            .configure_sets(
                PostUpdate,
                (NativeNetSet::SendPackets, NativeNetSet::FlushTransport).chain(),
            )
            .add_systems(
                PreUpdate,
                bootstrap::bootstrap_network.in_set(NativeNetSet::Bootstrap),
            )
            .add_systems(
                PreUpdate,
                (
                    transport::update_server_transport,
                    transport::update_client_transport,
                    host::host_handle_connects_and_disconnects,
                )
                    .chain()
                    .in_set(NativeNetSet::ReceiveTransport),
            )
            .add_systems(
                PreUpdate,
                (
                    client::client_send_hello_once,
                    client::client_send_ready,
                    host::host_receive_reliable_packets,
                    host::host_receive_input_packets,
                    client::client_receive_packets,
                )
                    .chain()
                    .in_set(NativeNetSet::ReceivePackets),
            )
            .add_systems(
                PostUpdate,
                (
                    host::host_broadcast_lobby_snapshot,
                    host::host_send_match_started,
                    host::host_send_world_snapshots,
                    host::host_relay_local_chat,
                    client::client_send_input_packets,
                    client::client_send_actions,
                    client::client_send_chat,
                )
                    .chain()
                    .in_set(NativeNetSet::SendPackets),
            )
            .add_systems(
                PostUpdate,
                (
                    transport::send_server_packets,
                    transport::send_client_packets,
                )
                    .in_set(NativeNetSet::FlushTransport),
            )
            .add_systems(Update, cleanup::cleanup_network_on_title)
            .add_systems(
                Update,
                client::interpolate_replicas
                    .run_if(|mode: Res<RuntimeMode>| mode.is_remote_client())
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                FixedUpdate,
                host::apply_remote_input_commands
                    .run_if(|mode: Res<RuntimeMode>| matches!(*mode, RuntimeMode::Host))
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                FixedUpdate,
                (
                    client::reconcile_local_prediction,
                    client::predict_local_player,
                )
                    .chain()
                    .run_if(|mode: Res<RuntimeMode>| mode.is_remote_client())
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(OnEnter(AppState::InGame), reset_network_match_state)
            .add_systems(OnExit(AppState::InGame), reset_network_match_state);
    }
}

#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
fn reset_network_match_state(
    mut identity: ResMut<crate::game::networking::common::NetworkIdentity>,
    mut snapshot: ResMut<crate::game::networking::common::ClientSnapshotSequence>,
    mut prediction: ResMut<crate::game::networking::common::ClientPredictionState>,
    mut reconciliation: ResMut<crate::game::networking::common::ClientReconciliation>,
    mut mappings: ResMut<crate::game::networking::common::NetworkMappings>,
) {
    identity.input_sequence = 0;
    snapshot.last_applied = None;
    prediction.clear();
    reconciliation.pending = None;

    mappings.pending_inputs.clear();
    mappings.last_enqueued_input_sequence.clear();
    mappings.last_processed_input_sequence.clear();
}
