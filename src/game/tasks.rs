use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{GameAssets, MatchCleanup, TASK_STATIONS};
use crate::app::AppState;
use game_utils_bevy::juice::Juice;

#[derive(Resource, Default)]
pub struct TaskBoard {
    pub completed: u32,
    pub total: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskKind {
    ShortHold,
    LongHold,
    TwoStage,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct TaskStation {
    pub id: u32,
    pub label: &'static str,
    pub kind: TaskKind,
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

fn task_kind_for_id(id: u32) -> TaskKind {
    match id % 3 {
        0 => TaskKind::LongHold,
        1 => TaskKind::ShortHold,
        _ => TaskKind::TwoStage,
    }
}

pub struct TasksPlugin;

impl Plugin for TasksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::InGame),
            spawn_task_stations.after(super::setup_match),
        );
    }
}

fn spawn_task_stations(mut commands: Commands, assets: Res<GameAssets>) {
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

    for ((id, label, position), image) in TASK_STATIONS.into_iter().zip(images) {
        let entity = commands
            .spawn((
                MatchCleanup,
                TaskStation {
                    id,
                    label,
                    kind: task_kind_for_id(id),
                },
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
