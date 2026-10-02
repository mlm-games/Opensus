use bevy_ecs::prelude::*;
use glam::Vec2;
use repame_fx::{EaseKind, EffectDef, Gradient, Jittered, step_particles_secs};

fn burst_def(color: [f32; 4], speed_lo: f32, speed_hi: f32) -> EffectDef {
    EffectDef {
        speed_pps: Jittered {
            base: (speed_lo + speed_hi) * 0.5,
            range: (speed_hi - speed_lo) * 0.5,
        },
        lifetime_secs: Jittered {
            base: 0.65,
            range: 0.25,
        },
        size_px: Jittered {
            base: 5.0,
            range: 2.0,
        },
        gravity_pps2: -60.0,
        drag_per_sec: 0.0,
        gradient: Gradient::fade_out(color),
        ease: EaseKind::Linear,
        ..EffectDef::default()
    }
}

fn kill_burst() -> EffectDef {
    burst_def([0.9, 0.15, 0.1, 1.0], 50.0, 120.0)
}

fn sabotage_burst() -> EffectDef {
    burst_def([0.8, 0.3, 0.1, 1.0], 40.0, 90.0)
}

fn task_burst() -> EffectDef {
    burst_def([0.4, 0.9, 0.5, 1.0], 30.0, 80.0)
}

fn spawn(commands: &mut Commands, pos: Vec2, count: usize, def: EffectDef) {
    repame_fx::burst(commands, pos.x, pos.y, &def, count, &mut rand::rng());
}

pub fn spawn_kill_burst(commands: &mut Commands, pos: Vec2) {
    spawn(commands, pos, 14, kill_burst());
}

pub fn spawn_sabotage_burst(commands: &mut Commands, pos: Vec2) {
    spawn(commands, pos, 8, sabotage_burst());
}

pub fn spawn_task_burst(commands: &mut Commands, pos: Vec2) {
    spawn(commands, pos, 10, task_burst());
}

pub fn tick_particles(
    time: Res<repame_sim::SimTime>,
    mut commands: Commands,
    mut particles: Query<(Entity, &mut repame_fx::Particle)>,
) {
    step_particles_secs(&mut commands, &mut particles, time.delta_secs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bursts_match_legacy_vfx_parameters() {
        let kill = kill_burst();
        assert_eq!(kill.speed_pps.base, 85.0);
        assert_eq!(kill.speed_pps.range, 35.0);
        assert_eq!(kill.lifetime_secs.base, 0.65);
        assert_eq!(kill.lifetime_secs.range, 0.25);
        assert_eq!(kill.size_px.base, 5.0);
        assert_eq!(kill.size_px.range, 2.0);
        assert_eq!(kill.gravity_pps2, -60.0);
        assert_eq!(kill.drag_per_sec, 0.0);
        assert!(matches!(kill.ease, EaseKind::Linear));
        assert_eq!(
            kill.gradient.keys,
            vec![(0.0, [0.9, 0.15, 0.1, 1.0]), (1.0, [0.9, 0.15, 0.1, 0.0]),]
        );

        let sabotage = sabotage_burst();
        assert_eq!(sabotage.speed_pps.base, 65.0);
        assert_eq!(sabotage.speed_pps.range, 25.0);
        assert_eq!(sabotage.gradient.keys[0].1, [0.8, 0.3, 0.1, 1.0]);

        let task = task_burst();
        assert_eq!(task.speed_pps.base, 55.0);
        assert_eq!(task.speed_pps.range, 25.0);
        assert_eq!(task.gradient.keys[0].1, [0.4, 0.9, 0.5, 1.0]);
    }
}
