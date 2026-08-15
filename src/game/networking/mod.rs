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
mod native;

use bevy::prelude::*;

#[cfg(all(feature = "networking-native", not(target_arch = "wasm32")))]
pub use native::*;

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
