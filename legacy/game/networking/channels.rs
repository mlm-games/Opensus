use std::time::Duration;

use renet2::{ChannelConfig, ConnectionConfig, SendType};

pub const C2S_RELIABLE: u8 = 0;
pub const C2S_INPUT: u8 = 1;

pub const S2C_RELIABLE: u8 = 0;
pub const S2C_SNAPSHOT: u8 = 1;

pub fn connection_config() -> ConnectionConfig {
    let reliable = |channel_id| ChannelConfig {
        channel_id,
        max_memory_usage_bytes: 2 * 1024 * 1024,
        send_type: SendType::ReliableOrdered {
            resend_time: Duration::from_millis(200),
        },
    };

    let unreliable = |channel_id| ChannelConfig {
        channel_id,
        max_memory_usage_bytes: 512 * 1024,
        send_type: SendType::Unreliable {
            ordered_reliable_substrate: false,
        },
    };

    ConnectionConfig::from_channels(
        vec![reliable(S2C_RELIABLE), unreliable(S2C_SNAPSHOT)],
        vec![reliable(C2S_RELIABLE), unreliable(C2S_INPUT)],
    )
}
