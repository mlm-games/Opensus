use bevy_ecs::prelude::*;

use super::{
    Alive, GamePhase, LIGHTS_STATION, LocalPlayer, MatchCleanup, MatchConfig, OXYGEN_STATIONS,
    Player, Position, REACTOR_STATIONS, Role, SimTimer, TimerMode, Trauma,
};
use crate::app::InputEdges;
use crate::save::SaveData;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SabotageKind {
    Lights,
    Oxygen,
    Reactor,
}

#[derive(Resource, Default)]
pub struct ActiveSabotage {
    pub kind: Option<SabotageKind>,
    pub timer: Option<SimTimer>,
    pub fixes_needed: u8,
    pub fixes_done: u8,
}

impl ActiveSabotage {
    pub fn is_active(&self) -> bool {
        self.kind.is_some()
    }

    pub fn is_critical(&self) -> bool {
        matches!(
            self.kind,
            Some(SabotageKind::Oxygen | SabotageKind::Reactor)
        )
    }

    pub fn critical_remaining(&self) -> f32 {
        self.timer
            .as_ref()
            .map(SimTimer::remaining_secs)
            .unwrap_or(0.0)
    }

    pub fn is_fixed(&self) -> bool {
        self.fixes_needed > 0 && self.fixes_done >= self.fixes_needed
    }

    pub fn clear(&mut self) {
        self.kind = None;
        self.timer = None;
        self.fixes_needed = 0;
        self.fixes_done = 0;
    }
}

#[derive(Component)]
pub struct SabotageFixStation {
    #[allow(dead_code, reason = "Only read by the networking replication code")]
    pub id: u8,
    pub kind: SabotageKind,
    pub progress: f32,
}

pub fn clear_sabotage_world(
    sabotage: &mut ActiveSabotage,
    stations: &mut Query<&mut SabotageFixStation>,
) {
    sabotage.clear();
    for mut station in stations.iter_mut() {
        station.progress = 0.0;
    }
}

#[derive(Resource, Default)]
pub struct SabotageCooldown {
    pub remaining: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SabotageRequest {
    pub actor_id: u64,
    pub kind: SabotageKind,
}

#[derive(Resource, Default)]
pub struct SabotageRequests(pub Vec<SabotageRequest>);

pub fn spawn_fix_stations(world: &mut World) {
    let stations = [
        (0u8, OXYGEN_STATIONS[0], SabotageKind::Oxygen),
        (1, OXYGEN_STATIONS[1], SabotageKind::Oxygen),
        (2, REACTOR_STATIONS[0], SabotageKind::Reactor),
        (3, REACTOR_STATIONS[1], SabotageKind::Reactor),
        (4, LIGHTS_STATION, SabotageKind::Lights),
    ];
    for (id, position, kind) in stations {
        world.spawn((
            MatchCleanup,
            SabotageFixStation {
                id,
                kind,
                progress: 0.0,
            },
            Position(position),
        ));
    }
}

pub fn read_local_sabotage_edges(
    mut edges: ResMut<InputEdges>,
    phase: Res<GamePhase>,
    sabotage: Res<ActiveSabotage>,
    cooldown: Res<SabotageCooldown>,
    local: Query<(&Player, &Role), (With<LocalPlayer>, With<Alive>)>,
    mut requests: ResMut<SabotageRequests>,
) {
    let (lights, oxygen, reactor) = (
        edges.sabotage_lights,
        edges.sabotage_oxygen,
        edges.sabotage_reactor,
    );
    edges.sabotage_lights = false;
    edges.sabotage_oxygen = false;
    edges.sabotage_reactor = false;

    if !lights && !oxygen && !reactor {
        return;
    }
    if !matches!(*phase, GamePhase::Playing) || sabotage.is_active() || cooldown.remaining > 0.0 {
        return;
    }
    let Ok((player, role)) = local.single() else {
        return;
    };
    if !matches!(role, Role::Impostor) {
        return;
    }

    let kind = if lights {
        SabotageKind::Lights
    } else if oxygen {
        SabotageKind::Oxygen
    } else {
        SabotageKind::Reactor
    };
    requests.0.push(SabotageRequest {
        actor_id: player.id,
        kind,
    });
}

pub fn apply_sabotage(
    mut requests: ResMut<SabotageRequests>,
    phase: Res<GamePhase>,
    config: Res<MatchConfig>,
    mut sabotage: ResMut<ActiveSabotage>,
    mut cooldown: ResMut<SabotageCooldown>,
    actors: Query<(&Player, &Role), With<Alive>>,
    mut stations: Query<&mut SabotageFixStation>,
    mut trauma: ResMut<Trauma>,
) {
    let requests = std::mem::take(&mut requests.0);
    if !matches!(*phase, GamePhase::Playing) {
        return;
    }

    for request in requests {
        if sabotage.is_active() || cooldown.remaining > 0.0 {
            continue;
        }

        let valid_actor = actors
            .iter()
            .any(|(player, role)| player.id == request.actor_id && matches!(role, Role::Impostor));
        if !valid_actor {
            continue;
        }

        let (timer, fixes_needed) = match request.kind {
            SabotageKind::Lights => (None, 1),
            SabotageKind::Oxygen => (
                Some(SimTimer::from_seconds(config.oxygen_time, TimerMode::Once)),
                2,
            ),
            SabotageKind::Reactor => (
                Some(SimTimer::from_seconds(config.reactor_time, TimerMode::Once)),
                2,
            ),
        };

        sabotage.kind = Some(request.kind);
        sabotage.timer = timer;
        sabotage.fixes_needed = fixes_needed;
        sabotage.fixes_done = 0;

        cooldown.remaining = config.sabotage_cooldown;

        for mut station in stations.iter_mut() {
            station.progress = 0.0;
        }

        trauma.add(0.6);
    }
}

pub fn tick_sabotage(
    time: Res<repame_sim::SimTime>,
    phase: Res<GamePhase>,
    mut sabotage: ResMut<ActiveSabotage>,
    mut cooldown: ResMut<SabotageCooldown>,
) {
    cooldown.remaining = (cooldown.remaining - time.delta_secs).max(0.0);

    if matches!(*phase, GamePhase::Playing)
        && let Some(timer) = sabotage.timer.as_mut()
    {
        timer.tick(time.delta_secs);
    }
}

pub fn check_sabotage_loss(
    mut phase: ResMut<GamePhase>,
    sabotage: Res<ActiveSabotage>,
    mut save: ResMut<SaveData>,
) {
    if !matches!(*phase, GamePhase::Playing) {
        return;
    }
    if !sabotage.is_critical() || sabotage.is_fixed() {
        return;
    }
    let expired = sabotage.timer.as_ref().is_some_and(SimTimer::is_finished);
    if !expired {
        return;
    }
    super::apply_game_over(&mut phase, super::WinReason::Sabotage, &mut save);
}

pub fn clear_fixed_sabotage(
    mut sabotage: ResMut<ActiveSabotage>,
    mut stations: Query<&mut SabotageFixStation>,
) {
    if !sabotage.is_fixed() {
        return;
    }
    clear_sabotage_world(&mut sabotage, &mut stations);
}
