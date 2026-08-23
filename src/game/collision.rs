use bevy::prelude::*;

use super::{MAP_BOUNDS, Player};

pub const PLAYER_RADIUS: f32 = 14.0;

/// Maximum movement performed before another collision test.
///
/// This prevents a player from crossing an entire thin wall during one
/// unusually long frame without requiring a full physics engine.
const MAX_SUBSTEP_DISTANCE: f32 = 5.0;

const CONTACT_EPSILON: f32 = 0.001;
const MAX_DEPENETRATION_PASSES: usize = 6;

#[derive(Component, Clone, Copy, Debug)]
pub struct SolidAabb {
    pub half_extents: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Axis {
    X,
    Y,
}

pub(crate) fn solid_boxes(
    solids: &Query<(&Transform, &SolidAabb), Without<Player>>,
) -> Vec<(Vec2, Vec2)> {
    solids
        .iter()
        .map(|(transform, solid)| (transform.translation.truncate(), solid.half_extents))
        .collect()
}

/// Move one player through the static collision world.
///
/// Features:
/// - sanitizes non-finite client input;
/// - clamps input to unit length;
/// - substeps long frames to prevent tunneling;
/// - separates X/Y movement to provide natural wall sliding;
/// - resolves initial overlaps;
/// - keeps living players fully inside map bounds;
/// - lets ghosts ignore walls while still respecting map bounds.
///
/// This function is shared by:
/// - offline/local movement;
/// - bot movement;
/// - authoritative remote-player movement;
/// - client prediction and replay.
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

    let speed = if speed.is_finite() {
        speed.max(0.0)
    } else {
        0.0
    };

    let delta_seconds = if delta_seconds.is_finite() {
        delta_seconds.max(0.0)
    } else {
        0.0
    };

    let displacement = movement * speed * delta_seconds;

    if !collides_with_walls {
        return clamp_to_map(position + displacement, 0.0);
    }

    let mut position = depenetrate_circle(position, PLAYER_RADIUS, boxes);

    let longest_axis = displacement.x.abs().max(displacement.y.abs());
    let substeps = (longest_axis / MAX_SUBSTEP_DISTANCE)
        .ceil()
        .clamp(1.0, 128.0) as usize;

    let step = displacement / substeps as f32;

    for _ in 0..substeps {
        position = move_axis(position, step.x, Axis::X, boxes);
        position = move_axis(position, step.y, Axis::Y, boxes);
        position = clamp_to_map(position, PLAYER_RADIUS);
    }

    position
}

fn move_axis(mut position: Vec2, amount: f32, axis: Axis, boxes: &[(Vec2, Vec2)]) -> Vec2 {
    if amount.abs() <= f32::EPSILON {
        return position;
    }

    match axis {
        Axis::X => position.x += amount,
        Axis::Y => position.y += amount,
    }

    // A second pass handles connected/overlapping wall segments at corners.
    for _ in 0..2 {
        let mut corrected = false;

        for &(center, half_extents) in boxes {
            let expanded = half_extents + Vec2::splat(PLAYER_RADIUS);
            let delta = position - center;

            if delta.x.abs() >= expanded.x || delta.y.abs() >= expanded.y {
                continue;
            }

            match axis {
                Axis::X => {
                    position.x = if amount > 0.0 {
                        center.x - expanded.x - CONTACT_EPSILON
                    } else {
                        center.x + expanded.x + CONTACT_EPSILON
                    };
                }
                Axis::Y => {
                    position.y = if amount > 0.0 {
                        center.y - expanded.y - CONTACT_EPSILON
                    } else {
                        center.y + expanded.y + CONTACT_EPSILON
                    };
                }
            }

            corrected = true;
        }

        if !corrected {
            break;
        }
    }

    position
}

/// Resolve an initial circle/AABB overlap using the actual circle shape.
///
/// The swept movement uses expanded AABBs for stable sliding. This path is
/// specifically for spawn/teleport/network corrections where a player may
/// already be embedded in geometry.
fn depenetrate_circle(mut position: Vec2, radius: f32, boxes: &[(Vec2, Vec2)]) -> Vec2 {
    for _ in 0..MAX_DEPENETRATION_PASSES {
        let mut changed = false;

        for &(center, half_extents) in boxes {
            let min = center - half_extents;
            let max = center + half_extents;
            let closest = position.clamp(min, max);
            let offset = position - closest;
            let distance_squared = offset.length_squared();

            if distance_squared >= radius * radius {
                continue;
            }

            if distance_squared > 0.000_001 {
                let distance = distance_squared.sqrt();
                let normal = offset / distance;
                position += normal * (radius - distance + CONTACT_EPSILON);
                changed = true;
                continue;
            }

            // Circle center is inside the AABB. Exit through the nearest side.
            let to_left = position.x - min.x;
            let to_right = max.x - position.x;
            let to_bottom = position.y - min.y;
            let to_top = max.y - position.y;

            let nearest = to_left.min(to_right).min(to_bottom).min(to_top);

            if nearest == to_left {
                position.x = min.x - radius - CONTACT_EPSILON;
            } else if nearest == to_right {
                position.x = max.x + radius + CONTACT_EPSILON;
            } else if nearest == to_bottom {
                position.y = min.y - radius - CONTACT_EPSILON;
            } else {
                position.y = max.y + radius + CONTACT_EPSILON;
            }

            changed = true;
        }

        if !changed {
            break;
        }
    }

    clamp_to_map(position, radius)
}

pub(crate) fn clamp_to_map(mut position: Vec2, margin: f32) -> Vec2 {
    let max_x = (MAP_BOUNDS.x - margin).max(0.0);
    let max_y = (MAP_BOUNDS.y - margin).max(0.0);

    position.x = position.x.clamp(-max_x, max_x);
    position.y = position.y.clamp(-max_y, max_y);
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
    fn non_finite_input_does_not_poison_position() {
        let result = step_player_position(
            Vec2::new(4.0, 8.0),
            Vec2::new(f32::NAN, 1.0),
            100.0,
            1.0,
            true,
            &[],
        );

        assert_eq!(result, Vec2::new(4.0, 8.0));
    }

    #[test]
    fn large_frame_cannot_tunnel_through_wall() {
        let walls = [(Vec2::ZERO, Vec2::new(5.0, 100.0))];

        let result =
            step_player_position(Vec2::new(-100.0, 0.0), Vec2::X, 1_000.0, 0.25, true, &walls);

        assert!(result.x <= -PLAYER_RADIUS - 5.0);
    }

    #[test]
    fn diagonal_movement_slides_along_wall() {
        let walls = [(Vec2::ZERO, Vec2::new(5.0, 100.0))];

        let result = step_player_position(
            Vec2::new(-30.0, -60.0),
            Vec2::new(1.0, 1.0),
            100.0,
            0.5,
            true,
            &walls,
        );

        assert!(result.x <= -PLAYER_RADIUS - 5.0);
        assert!(result.y > -60.0);
    }

    #[test]
    fn living_player_center_keeps_radius_inside_bounds() {
        let result = step_player_position(
            Vec2::new(MAP_BOUNDS.x - 20.0, 0.0),
            Vec2::X,
            100.0,
            1.0,
            true,
            &[],
        );

        assert_eq!(result.x, MAP_BOUNDS.x - PLAYER_RADIUS);
    }

    #[test]
    fn ghost_ignores_walls_but_respects_map_bounds() {
        let walls = [(Vec2::ZERO, Vec2::new(100.0, 100.0))];

        let result =
            step_player_position(Vec2::new(-150.0, 0.0), Vec2::X, 1_000.0, 1.0, false, &walls);

        assert_eq!(result.x, MAP_BOUNDS.x);
    }
}
