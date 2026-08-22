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
    let boxes = solid_boxes(&solids);

    for mut transform in &mut bodies {
        let position = resolve_position(transform.translation.truncate(), &boxes);
        transform.translation.x = position.x;
        transform.translation.y = position.y;
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
    let boxes = solid_boxes(&solids);

    for mut transform in &mut bodies {
        let position = resolve_position(transform.translation.truncate(), &boxes);
        transform.translation.x = position.x;
        transform.translation.y = position.y;
    }
}

pub(crate) fn solid_boxes(
    solids: &Query<(&Transform, &SolidAabb), Without<Player>>,
) -> Vec<(Vec2, Vec2)> {
    solids
        .iter()
        .map(|(transform, solid)| (transform.translation.truncate(), solid.half_extents))
        .collect()
}

#[allow(dead_code)]
pub(crate) fn step_player_position(
    position: Vec2,
    movement: Vec2,
    speed: f32,
    delta_seconds: f32,
    collides_with_walls: bool,
    boxes: &[(Vec2, Vec2)],
) -> Vec2 {
    let movement = if movement.is_finite() {
        movement.clamp_length_max(1.0)
    } else {
        Vec2::ZERO
    };

    let next = position + movement * speed.max(0.0) * delta_seconds.max(0.0);

    if collides_with_walls {
        resolve_position(next, boxes)
    } else {
        clamp_to_map(next)
    }
}

pub(crate) fn resolve_position(mut position: Vec2, boxes: &[(Vec2, Vec2)]) -> Vec2 {
    for &(center, half_extents) in boxes {
        position = push_out_circle_aabb(position, PLAYER_RADIUS, center, half_extents);
    }

    clamp_to_map(position)
}

pub(crate) fn clamp_to_map(mut position: Vec2) -> Vec2 {
    position.x = position.x.clamp(-MAP_BOUNDS.x, MAP_BOUNDS.x);
    position.y = position.y.clamp(-MAP_BOUNDS.y, MAP_BOUNDS.y);
    position
}

fn push_out_circle_aabb(mut position: Vec2, radius: f32, center: Vec2, half_extents: Vec2) -> Vec2 {
    let expanded = half_extents + Vec2::splat(radius);
    let delta = position - center;
    let clamped = delta.clamp(-expanded, expanded);

    if (delta - clamped).length_squared() > 0.0 {
        return position;
    }

    let penetration = expanded - delta.abs();
    let sign_x = if delta.x >= 0.0 { 1.0 } else { -1.0 };
    let sign_y = if delta.y >= 0.0 { 1.0 } else { -1.0 };

    if penetration.x < penetration.y {
        position.x = center.x + expanded.x * sign_x;
    } else {
        position.y = center.y + expanded.y * sign_y;
    }

    position
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_is_clamped_to_unit_input() {
        let result = step_player_position(Vec2::ZERO, Vec2::new(10.0, 0.0), 100.0, 0.1, false, &[]);

        assert!((result.x - 10.0).abs() < 0.001);
    }

    #[test]
    fn ghost_movement_still_respects_map_bounds() {
        let result = step_player_position(
            Vec2::new(MAP_BOUNDS.x - 1.0, 0.0),
            Vec2::X,
            100.0,
            1.0,
            false,
            &[],
        );

        assert_eq!(result.x, MAP_BOUNDS.x);
    }
}
