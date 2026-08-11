use bevy::prelude::*;

#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeMode {
    /// Offline sandbox. The current process owns all game state.
    #[default]
    Local,

    /// Listen server. The current process owns all authoritative game state
    /// and also renders/controls the host player.
    #[allow(dead_code, reason = "Reserved for the network protocol")]
    Host,

    /// Remote client. Authoritative game state comes from the server.
    #[allow(dead_code, reason = "Reserved for the network protocol")]
    Client,
}

impl RuntimeMode {
    pub const fn has_authority(self) -> bool {
        matches!(self, Self::Local | Self::Host)
    }

    #[allow(dead_code, reason = "Reserved for the network protocol")]
    pub const fn is_remote_client(self) -> bool {
        matches!(self, Self::Client)
    }
}

pub fn has_authority(mode: Res<RuntimeMode>) -> bool {
    mode.has_authority()
}

#[allow(dead_code, reason = "Reserved for the network protocol")]
pub fn is_remote_client(mode: Res<RuntimeMode>) -> bool {
    mode.is_remote_client()
}
