//! Networking stub. Local sandbox is authoritative for now.
//!
//! Raw `renet2` integration plan (feature = "networking-native"):
//! - Server-authoritative: role assignment, kill validation, vote tally,
//!   sabotage triggers, task completion.
//! - Client-predicted: local Transform from movement input.
//! - Interpolated: remote player Transforms.
//! - Client packets: Hello, Ready, Input, Kill, Report, Emergency, Vote,
//!   Sabotage, Chat. Server packets: Welcome, LobbySnapshot, MatchStarted,
//!   WorldSnapshot, Chat, Rejected.
//! - The server derives the actor from the connected renet2 `client_id` and
//!   never accepts actor ids or targets from client packets.
//!
//! `renet2` / `renet2_netcode` are used without their `bevy` features so the
//! dependency graph does not pull in a second (0.19) Bevy.

use bevy::prelude::*;

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, _app: &mut App) {
        // No-op in local sandbox mode.
    }
}
