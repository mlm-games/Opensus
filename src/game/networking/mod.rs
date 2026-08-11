//! Networking stub. Local sandbox is authoritative for now.
//!
//! Lightyear integration plan (feature = "networking"):
//! - Server-authoritative: role assignment, kill validation, vote tally,
//!   sabotage triggers, task completion.
//! - Client-predicted: local Transform from movement input.
//! - Interpolated: remote player Transforms.
//! - Messages mirror the existing local ones: StartMatchRequest, KillRequest,
//!   ReportBody, MeetingCommand, TaskInteract, SabotageAction.

use bevy::prelude::*;

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, _app: &mut App) {
        // No-op in local sandbox mode.
    }
}
