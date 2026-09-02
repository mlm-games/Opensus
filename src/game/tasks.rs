use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{GameAssets, LocalPlayer, MatchCleanup, Role, TASK_STATIONS};
use crate::app::AppState;
use game_utils_bevy::juice::Juice;

#[derive(Resource, Default)]
pub struct TaskBoard {
    pub completed: u32,
    pub total: u32,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct TaskStation {
    pub id: u32,
    pub label: &'static str,
}

#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct TaskAssignments {
    pub assigned: Vec<u32>,
    pub completed: Vec<u32>,
    pub active_task: Option<u32>,
    pub active_progress: f32,
}

impl TaskAssignments {
    pub fn new(assigned: Vec<u32>) -> Self {
        Self {
            assigned,
            completed: Vec::new(),
            active_task: None,
            active_progress: 0.0,
        }
    }

    #[inline]
    pub fn has(&self, id: u32) -> bool {
        self.assigned.contains(&id)
    }

    #[inline]
    pub fn is_done(&self, id: u32) -> bool {
        self.completed.contains(&id)
    }

    #[inline]
    pub fn remaining(&self) -> usize {
        self.assigned
            .iter()
            .filter(|id| !self.completed.contains(id))
            .count()
    }

    pub fn reset_hold_if_not(&mut self, id: u32) {
        if self.active_task != Some(id) {
            self.active_task = Some(id);
            self.active_progress = 0.0;
        }
    }

    pub fn clear_hold(&mut self) {
        self.active_task = None;
        self.active_progress = 0.0;
    }

    pub fn complete_active(&mut self) -> Option<u32> {
        let id = self.active_task?;
        if !self.has(id) || self.is_done(id) {
            self.clear_hold();
            return None;
        }

        self.completed.push(id);
        self.clear_hold();
        Some(id)
    }
}

pub fn deterministic_task_ids(player_id: u64, slot_index: usize, count: usize) -> Vec<u32> {
    let ids: Vec<u32> = TASK_STATIONS.iter().map(|(id, _, _)| *id).collect();
    if ids.is_empty() || count == 0 {
        return Vec::new();
    }

    let count = count.min(ids.len());
    let mut out = Vec::with_capacity(count);
    let start = (player_id as usize + slot_index * 3) % ids.len();

    for step in 0..ids.len() {
        if out.len() >= count {
            break;
        }

        let id = ids[(start + step * 2) % ids.len()];
        if !out.contains(&id) {
            out.push(id);
        }
    }

    out
}

// Compatibility shims for first PR code (PlayerTasks / assign_tasks / TaskKind)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskKind {
    ShortHold,
    LongHold,
    TwoStage,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerTask {
    pub station_id: u32,
    pub progress: f32,
    pub stage: u8,
    pub done: bool,
}

#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerTasks {
    pub items: Vec<PlayerTask>,
}

impl PlayerTasks {
    pub fn unfinished(&self) -> impl Iterator<Item = &PlayerTask> {
        self.items.iter().filter(|task| !task.done)
    }

    pub fn get_mut(&mut self, station_id: u32) -> Option<&mut PlayerTask> {
        self.items
            .iter_mut()
            .find(|task| task.station_id == station_id)
    }
}

pub fn assign_tasks(player_id: u64, count: usize) -> PlayerTasks {
    // Fallback wrapper using old logic for compatibility
    let station_count = TASK_STATIONS.len();
    let count = count.min(station_count);
    let start = (player_id as usize * 7) % station_count;
    let items = (0..count)
        .map(|index| {
            let station_index = (start + index * 3) % station_count;
            PlayerTask {
                station_id: TASK_STATIONS[station_index].0,
                progress: 0.0,
                stage: 0,
                done: false,
            }
        })
        .collect();
    PlayerTasks { items }
}

fn task_kind_for_id(_id: u32) -> TaskKind {
    TaskKind::ShortHold
}

pub struct TasksPlugin;

impl Plugin for TasksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::InGame),
            spawn_task_stations.after(super::setup_match),
        )
        .add_systems(
            Update,
            color_task_stations_for_local.run_if(in_state(AppState::InGame)),
        );
    }
}

fn spawn_task_stations(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut board: ResMut<TaskBoard>,
) {
    let images = [
        assets.task_beaker.clone(),
        assets.task_flask.clone(),
        assets.task_burner.clone(),
        assets.task_flask.clone(),
        assets.task_beaker.clone(),
        assets.task_burner.clone(),
        assets.task_flask.clone(),
        assets.task_beaker.clone(),
        assets.task_flask.clone(),
        assets.task_burner.clone(),
    ];

    board.completed = 0;
    // board.total is now owned by spawn_players_from_lobby because total tasks
    // depend on living crewmate assignments, not global stations.

    for ((id, label, position), image) in TASK_STATIONS.into_iter().zip(images) {
        let entity = commands
            .spawn((
                MatchCleanup,
                TaskStation { id, label },
                Sprite {
                    image,
                    custom_size: Some(Vec2::splat(28.0)),
                    ..default()
                },
                Transform::from_xyz(position.x, position.y, 4.0),
            ))
            .id();

        Juice::pop_in(&mut commands, entity, 0.2);
    }
}

fn color_task_stations_for_local(
    local: Query<(&TaskAssignments, Option<&Role>), With<LocalPlayer>>,
    mut stations: Query<(&TaskStation, &mut Sprite)>,
) {
    let Ok((assignments, role)) = local.single() else {
        return;
    };

    let impostor = matches!(role, Some(Role::Impostor));

    for (station, mut sprite) in &mut stations {
        if assignments.is_done(station.id) {
            sprite.color = Color::srgba(0.45, 0.48, 0.50, 0.75);
        } else if assignments.has(station.id) || impostor {
            sprite.color = Color::srgba(1.0, 1.0, 1.0, 1.0);
        } else {
            sprite.color = Color::srgba(0.55, 0.60, 0.64, 0.42);
        }
    }
}
