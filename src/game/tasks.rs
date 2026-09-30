use bevy_ecs::prelude::*;

use super::{MatchCleanup, Position, TASK_STATIONS};

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

#[derive(Component, Clone, Debug, Default)]
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

pub fn spawn_task_stations(world: &mut World) {
    world.resource_mut::<super::TaskBoard>().completed = 0;
    // `total` is owned by spawn_players_from_lobby because total tasks
    // depend on living crewmate assignments, not global stations.

    for (id, label, position) in TASK_STATIONS {
        world.spawn((MatchCleanup, TaskStation { id, label }, Position(position)));
    }
}
