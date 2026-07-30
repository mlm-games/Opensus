use bevy::prelude::*;
use std::collections::VecDeque;

#[derive(Resource)]
pub struct EntityPool<M: Component + Default> {
    available: VecDeque<Entity>,
    active: Vec<Entity>,
    max_size: usize,
    _marker: std::marker::PhantomData<M>,
}

impl<M: Component + Default> EntityPool<M> {
    pub fn new(max_size: usize) -> Self {
        Self {
            available: VecDeque::new(),
            active: Vec::new(),
            max_size,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub fn available_count(&self) -> usize {
        self.available.len()
    }

    pub fn total_count(&self) -> usize {
        self.active.len() + self.available.len()
    }
}

pub struct ObjectPool;

impl ObjectPool {
    pub fn prewarm<M: Component + Default>(
        pool: &mut EntityPool<M>,
        commands: &mut Commands,
        count: usize,
        spawn: impl FnMut(&mut EntityCommands),
    ) {
        let room = pool
            .max_size
            .saturating_sub(pool.active.len() + pool.available.len());
        let actual = count.min(room);
        let mut spawn = spawn;
        for _ in 0..actual {
            let mut ec = commands.spawn((Visibility::Hidden, M::default()));
            spawn(&mut ec);
            pool.available.push_back(ec.id());
        }
    }

    pub fn acquire<M: Component + Default>(
        pool: &mut EntityPool<M>,
        commands: &mut Commands,
        spawn: impl FnOnce(&mut EntityCommands),
    ) -> Option<Entity> {
        if let Some(e) = pool.available.pop_front() {
            pool.active.push(e);
            commands
                .entity(e)
                .insert((Visibility::Visible, M::default()));
            return Some(e);
        }
        if pool.active.len() >= pool.max_size {
            return None;
        }
        let mut ec = commands.spawn((Visibility::Visible, M::default()));
        spawn(&mut ec);
        let e = ec.id();
        pool.active.push(e);
        Some(e)
    }

    pub fn release<M: Component + Default>(
        pool: &mut EntityPool<M>,
        entity: Entity,
        commands: &mut Commands,
    ) {
        if let Some(i) = pool.active.iter().position(|&e| e == entity) {
            pool.active.swap_remove(i);
            commands
                .entity(entity)
                .insert(Visibility::Hidden)
                .remove::<M>();
            pool.available.push_back(entity);
        }
    }
}
