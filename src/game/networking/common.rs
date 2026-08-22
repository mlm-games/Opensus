use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, SystemTime};

use bevy::prelude::*;
use renet2::{ClientId, RenetClient, RenetServer};
use renet2_netcode::{NetcodeClientTransport, NetcodeServerTransport};

use super::protocol::NetInputCommand;

#[derive(Resource)]
pub struct NetServerRes(pub RenetServer);

#[derive(Resource)]
pub struct NetServerTransportRes(pub NetcodeServerTransport);

#[derive(Resource)]
pub struct NetClientRes(pub RenetClient);

#[derive(Resource)]
pub struct NetClientTransportRes(pub NetcodeClientTransport);

#[derive(Resource, Default)]
pub struct NetworkIdentity {
    pub my_player_id: Option<u64>,
    pub hello_sent: bool,
    pub input_sequence: u32,
}

#[derive(Resource, Default)]
pub struct NetworkMappings {
    pub client_to_player: HashMap<ClientId, u64>,
    pub player_to_client: HashMap<u64, ClientId>,

    pub body_entities: HashMap<u64, Entity>,
    pub next_body_id: u64,

    pub authenticated_clients: HashSet<ClientId>,

    /// Commands accepted from each client but not yet simulated.
    pub pending_inputs: HashMap<ClientId, VecDeque<NetInputCommand>>,

    /// Highest sequence accepted into each client's pending queue.
    pub last_enqueued_input_sequence: HashMap<ClientId, u32>,

    /// Highest sequence actually simulated by the server.
    pub last_processed_input_sequence: HashMap<ClientId, u32>,

    pub handshake_deadline: HashMap<ClientId, Duration>,
    pub reliable_buckets: HashMap<ClientId, TokenBucket>,
    pub chat_buckets: HashMap<ClientId, TokenBucket>,
    pub action_buckets: HashMap<ClientId, TokenBucket>,
}

pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

pub const RELIABLE_TOKENS_PER_SEC: f32 = 40.0;
pub const RELIABLE_BURST: f32 = 64.0;
pub const CHAT_TOKENS_PER_SEC: f32 = 0.5;
pub const CHAT_BURST: f32 = 3.0;
pub const ACTION_TOKENS_PER_SEC: f32 = 5.0;
pub const ACTION_BURST: f32 = 10.0;

#[derive(Clone, Debug)]
pub struct TokenBucket {
    pub tokens: f32,
    pub last_refill: Duration,
    pub capacity: f32,
    pub refill_per_sec: f32,
}

impl TokenBucket {
    pub fn new(capacity: f32, refill_per_sec: f32, now: Duration) -> Self {
        Self {
            tokens: capacity,
            last_refill: now,
            capacity,
            refill_per_sec,
        }
    }

    pub fn refill(&mut self, now: Duration) {
        let dt = now.saturating_sub(self.last_refill).as_secs_f32().max(0.0);
        if dt > 0.0 {
            self.tokens = (self.tokens + dt * self.refill_per_sec).min(self.capacity);
            self.last_refill = now;
        }
    }

    pub fn try_consume(&mut self, now: Duration, cost: f32) -> bool {
        self.refill(now);
        if self.tokens >= cost {
            self.tokens -= cost;
            true
        } else {
            false
        }
    }
}

#[derive(Resource, Default)]
pub struct ServerSnapshotSequence(pub u32);

#[derive(Resource, Default)]
pub struct ClientSnapshotSequence {
    pub last_applied: Option<u32>,
}

pub fn sequence_is_newer(sequence: u32, previous: u32) -> bool {
    sequence != previous && sequence.wrapping_sub(previous) < (u32::MAX / 2)
}

#[derive(Resource)]
pub struct LobbyBroadcastTimer(pub Timer);

#[derive(Resource)]
pub struct SnapshotTimer(pub Timer);

#[derive(Component)]
pub struct ReplicaPlayer {
    pub player_id: u64,
}

#[derive(Component)]
pub struct ReplicaBody {
    pub body_id: u64,
}

pub const SNAPSHOT_HZ: f32 = 20.0;

/// Explicitly matches Bevy's fixed-step default while documenting that the
/// network prediction protocol depends on this frequency.
pub const PREDICTION_HZ: f64 = 64.0;

/// Up to one second of 64 Hz commands fits comfortably below the existing
/// 4 KiB input-packet limit.
pub const INPUT_BATCH_SIZE: usize = 64;

/// Client-side memory bound if snapshots/acknowledgements stop arriving.
pub const MAX_CLIENT_PENDING_INPUTS: usize = 256;

/// Server-side bound against an authenticated client flooding future inputs.
pub const MAX_SERVER_PENDING_INPUTS: usize = 128;

#[derive(Resource, Default)]
pub struct ClientPredictionState {
    pub pending: VecDeque<NetInputCommand>,
}

impl ClientPredictionState {
    pub fn push(&mut self, command: NetInputCommand) {
        self.pending.push_back(command);

        while self.pending.len() > MAX_CLIENT_PENDING_INPUTS {
            self.pending.pop_front();
        }
    }

    pub fn acknowledge(&mut self, acknowledged: Option<u32>) {
        let Some(acknowledged) = acknowledged else {
            return;
        };

        self.pending
            .retain(|command| sequence_is_newer(command.sequence, acknowledged));
    }

    /// Oldest unacknowledged commands first.
    pub fn send_batch(&self) -> Vec<NetInputCommand> {
        self.pending
            .iter()
            .take(INPUT_BATCH_SIZE)
            .copied()
            .collect()
    }

    pub fn clear(&mut self) {
        self.pending.clear();
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ReconciliationSample {
    pub authoritative_position: Vec2,
    pub acknowledged_input_sequence: Option<u32>,
}

#[derive(Resource, Default)]
pub struct ClientReconciliation {
    pub pending: Option<ReconciliationSample>,
}

/// Interpolation delay in seconds. Two snapshot intervals at SNAPSHOT_HZ (20Hz)
/// tolerates one dropped snapshot without stalling.
pub const INTERPOLATION_DELAY: f64 = 2.0 / SNAPSHOT_HZ as f64;

/// A jump larger than this between consecutive samples is treated as a
/// teleport (meeting seat, respawn): clear the buffer and snap.
pub const SNAP_DISTANCE: f32 = 150.0;

pub const MAX_INTERPOLATION_SAMPLES: usize = 12;

#[derive(Clone, Copy, Debug)]
pub struct PositionSample {
    /// Client-side arrival time (Time<Real> elapsed seconds).
    pub time: f64,
    pub position: Vec2,
}

#[derive(Component, Default)]
pub struct ReplicaInterpolation {
    pub samples: VecDeque<PositionSample>,
}

impl ReplicaInterpolation {
    pub fn with_initial(time: f64, position: Vec2) -> Self {
        let mut samples = VecDeque::new();
        samples.push_back(PositionSample { time, position });
        Self { samples }
    }

    pub fn push_sample(&mut self, time: f64, position: Vec2) {
        if let Some(last) = self.samples.back() {
            // Ignore out-of-order pushes (shouldn't happen post-sequencing, but cheap).
            if time <= last.time {
                return;
            }
            if last.position.distance(position) > SNAP_DISTANCE {
                self.samples.clear();
            }
        }
        self.samples.push_back(PositionSample { time, position });
        while self.samples.len() > MAX_INTERPOLATION_SAMPLES {
            self.samples.pop_front();
        }
    }
}

/// Pure sampling: lerp between the two samples bracketing `render_time`.
/// Clamps to the oldest/newest sample outside the buffered range.
pub fn sample_position(samples: &VecDeque<PositionSample>, render_time: f64) -> Option<Vec2> {
    let first = samples.front()?;
    if render_time <= first.time {
        return Some(first.position);
    }

    let mut previous = *first;
    for sample in samples.iter().skip(1) {
        if render_time <= sample.time {
            let span = sample.time - previous.time;
            if span <= f64::EPSILON {
                return Some(sample.position);
            }
            let t = ((render_time - previous.time) / span) as f32;
            return Some(previous.position.lerp(sample.position, t.clamp(0.0, 1.0)));
        }
        previous = *sample;
    }

    // Render time is ahead of newest data (packet loss): hold, don't extrapolate.
    Some(previous.position)
}

pub fn now_duration() -> Duration {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
}

#[cfg(test)]
mod interpolation_tests {
    use super::*;

    fn buffer(samples: &[(f64, Vec2)]) -> VecDeque<PositionSample> {
        samples
            .iter()
            .map(|&(time, position)| PositionSample { time, position })
            .collect()
    }

    #[test]
    fn lerps_between_bracketing_samples() {
        let samples = buffer(&[(0.0, Vec2::ZERO), (1.0, Vec2::new(10.0, 0.0))]);
        let position = sample_position(&samples, 0.5).unwrap();
        assert!((position.x - 5.0).abs() < 1e-4);
    }

    #[test]
    fn clamps_before_first_and_after_last() {
        let samples = buffer(&[(1.0, Vec2::new(3.0, 3.0)), (2.0, Vec2::new(7.0, 7.0))]);
        assert_eq!(sample_position(&samples, 0.0).unwrap(), Vec2::new(3.0, 3.0));
        assert_eq!(sample_position(&samples, 5.0).unwrap(), Vec2::new(7.0, 7.0));
    }

    #[test]
    fn empty_buffer_yields_none() {
        assert!(sample_position(&VecDeque::new(), 1.0).is_none());
    }

    #[test]
    fn teleport_clears_buffer_and_snaps() {
        let mut interp = ReplicaInterpolation::with_initial(0.0, Vec2::ZERO);
        interp.push_sample(0.05, Vec2::new(5.0, 0.0));
        interp.push_sample(0.10, Vec2::new(500.0, 0.0)); // meeting teleport
        assert_eq!(interp.samples.len(), 1);
        assert_eq!(
            interp.samples.back().unwrap().position,
            Vec2::new(500.0, 0.0)
        );
    }

    #[test]
    fn rejects_out_of_order_samples() {
        let mut interp = ReplicaInterpolation::with_initial(1.0, Vec2::ZERO);
        interp.push_sample(0.5, Vec2::new(9.0, 9.0));
        assert_eq!(interp.samples.len(), 1);
    }

    fn input(sequence: u32) -> NetInputCommand {
        NetInputCommand {
            sequence,
            movement: [1.0, 0.0],
            interact: false,
        }
    }

    #[test]
    fn prediction_ack_removes_processed_commands() {
        let mut prediction = ClientPredictionState::default();

        prediction.push(input(1));
        prediction.push(input(2));
        prediction.push(input(3));

        prediction.acknowledge(Some(2));

        let remaining: Vec<u32> = prediction
            .pending
            .iter()
            .map(|command| command.sequence)
            .collect();

        assert_eq!(remaining, vec![3]);
    }

    #[test]
    fn prediction_batch_preserves_oldest_first_order() {
        let mut prediction = ClientPredictionState::default();

        prediction.push(input(10));
        prediction.push(input(11));
        prediction.push(input(12));

        let sequences: Vec<u32> = prediction
            .send_batch()
            .iter()
            .map(|command| command.sequence)
            .collect();

        assert_eq!(sequences, vec![10, 11, 12]);
    }

    #[test]
    fn prediction_ack_handles_sequence_wraparound() {
        let mut prediction = ClientPredictionState::default();

        prediction.push(input(u32::MAX));
        prediction.push(input(0));
        prediction.push(input(1));

        prediction.acknowledge(Some(u32::MAX));

        let remaining: Vec<u32> = prediction
            .pending
            .iter()
            .map(|command| command.sequence)
            .collect();

        assert_eq!(remaining, vec![0, 1]);
    }
}
