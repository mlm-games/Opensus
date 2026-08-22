use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, SystemTime};

use bevy::prelude::*;
use renet2::{ClientId, RenetClient, RenetServer};
use renet2_netcode::{NetcodeClientTransport, NetcodeServerTransport};

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
    pub last_input_sequence: HashMap<ClientId, u32>,
    pub authenticated_clients: HashSet<ClientId>,
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
}
