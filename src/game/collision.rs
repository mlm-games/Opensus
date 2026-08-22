use bevy::prelude::*;

use super::{Alive, LocalPlayer, MAP_BOUNDS, Player};

const PLAYER_RADIUS: f32 = 14.0;

#[derive(Component, Clone, Copy, Debug)]
pub struct SolidAabb {
    pub half_extents: Vec2,
}

pub fn resolve_local_solids(
    solids: Query<(&Transform, &SolidAabb), Without<Player>>,
    mut bodies: Query<
        &mut Transform,
        (
            With<Player>,
            With<LocalPlayer>,
            With<Alive>,
            Without<SolidAabb>,
        ),
    >,
) {
    let boxes: Vec<(Vec2, Vec2)> = solids
        .iter()
        .map(|(t, s)| (t.translation.truncate(), s.half_extents))
        .collect();
    for mut tf in &mut bodies {
        *tf = resolve(&tf, &boxes);
    }
}

pub fn resolve_solids(
    solids: Query<(&Transform, &SolidAabb), Without<Player>>,
    mut bodies: Query<
        &mut Transform,
        (
            With<Player>,
            With<Alive>,
            Without<LocalPlayer>,
            Without<SolidAabb>,
        ),
    >,
) {
    let boxes: Vec<(Vec2, Vec2)> = solids
        .iter()
        .map(|(t, s)| (t.translation.truncate(), s.half_extents))
        .collect();
    for mut tf in &mut bodies {
        *tf = resolve(&tf, &boxes);
    }
}

fn resolve(tf: &Transform, boxes: &[(Vec2, Vec2)]) -> Transform {
    let mut p = tf.translation.truncate();
    for &(c, h) in boxes {
        p = push_out_circle_aabb(p, PLAYER_RADIUS, c, h);
    }
    p.x = p.x.clamp(-MAP_BOUNDS.x, MAP_BOUNDS.x);
    p.y = p.y.clamp(-MAP_BOUNDS.y, MAP_BOUNDS.y);
    Transform::from_xyz(p.x, p.y, tf.translation.z)
}

fn push_out_circle_aabb(mut p: Vec2, r: f32, center: Vec2, half: Vec2) -> Vec2 {
    let expanded = half + Vec2::splat(r);
    let delta = p - center;
    let clamped = delta.clamp(-expanded, expanded);
    // Outside the expanded AABB: no push needed.
    if (delta - clamped).length_squared() > 0.0 {
        return p;
    }
    // Push to the nearest face.
    let pen = expanded - delta.abs();
    let sx = if delta.x >= 0.0 { 1.0 } else { -1.0 };
    let sy = if delta.y >= 0.0 { 1.0 } else { -1.0 };
    if pen.x < pen.y {
        p.x = center.x + expanded.x * sx;
    } else {
        p.y = center.y + expanded.y * sy;
    }
    p
}
