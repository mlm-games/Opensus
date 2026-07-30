use bevy::prelude::*;

pub struct MathUtils;

impl MathUtils {
    pub fn smooth_damp(
        current: f32,
        target: f32,
        current_velocity: f32,
        smooth_time: f32,
        delta: f32,
    ) -> (f32, f32) {
        let smooth_time = smooth_time.max(0.0001);
        let omega = 2.0 / smooth_time;
        let x = omega * delta;
        let exp = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
        let change = current - target;
        let temp = (current_velocity + omega * change) * delta;
        let new_velocity = (current_velocity - omega * temp) * exp;
        let output = target + (change + temp) * exp;
        (output, new_velocity)
    }

    pub fn approach(current: f32, target: f32, rate: f32) -> f32 {
        if current < target {
            (current + rate).min(target)
        } else {
            (current - rate).max(target)
        }
    }

    pub fn wave(from: f32, to: f32, duration: f32, offset: f32, time_secs: f32) -> f32 {
        if duration == 0.0 {
            return from;
        }
        let t = (time_secs + offset) / duration;
        from + (to - from) * (t * std::f32::consts::TAU).sin().mul_add(0.5, 0.5)
    }

    pub fn smooth_damp_vec2(
        current: Vec2,
        target: Vec2,
        velocity: &mut Vec2,
        smooth_time: f32,
        delta: f32,
    ) -> Vec2 {
        let (x, vx) = Self::smooth_damp(current.x, target.x, velocity.x, smooth_time, delta);
        let (y, vy) = Self::smooth_damp(current.y, target.y, velocity.y, smooth_time, delta);
        *velocity = Vec2::new(vx, vy);
        Vec2::new(x, y)
    }

    pub fn approach_vec2(current: Vec2, target: Vec2, rate: f32) -> Vec2 {
        Vec2::new(
            Self::approach(current.x, target.x, rate),
            Self::approach(current.y, target.y, rate),
        )
    }

    pub fn smooth_damp_vec3(
        current: Vec3,
        target: Vec3,
        velocity: &mut Vec3,
        smooth_time: f32,
        delta: f32,
    ) -> Vec3 {
        let (x, vx) = Self::smooth_damp(current.x, target.x, velocity.x, smooth_time, delta);
        let (y, vy) = Self::smooth_damp(current.y, target.y, velocity.y, smooth_time, delta);
        let (z, vz) = Self::smooth_damp(current.z, target.z, velocity.z, smooth_time, delta);
        *velocity = Vec3::new(vx, vy, vz);
        Vec3::new(x, y, z)
    }
}
