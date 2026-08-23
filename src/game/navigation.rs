use bevy::prelude::*;

use super::{NAV_EDGES, NAV_NODES, PLAYER_RADIUS};

const NAV_CLEARANCE: f32 = PLAYER_RADIUS + 2.0;

fn nearest_node(position: Vec2) -> usize {
    NAV_NODES
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            position
                .distance_squared(**left)
                .partial_cmp(&position.distance_squared(**right))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn a_star(start: usize, goal: usize) -> Option<Vec<usize>> {
    let mut adjacency = vec![Vec::<usize>::new(); NAV_NODES.len()];

    for &(left, right) in &NAV_EDGES {
        adjacency[left].push(right);
        adjacency[right].push(left);
    }

    let mut open = vec![start];
    let mut came_from = vec![None::<usize>; NAV_NODES.len()];
    let mut closed = vec![false; NAV_NODES.len()];
    let mut g_score = vec![f32::INFINITY; NAV_NODES.len()];
    let mut f_score = vec![f32::INFINITY; NAV_NODES.len()];

    g_score[start] = 0.0;
    f_score[start] = NAV_NODES[start].distance(NAV_NODES[goal]);

    while !open.is_empty() {
        let current_index = open
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                f_score[**left]
                    .partial_cmp(&f_score[**right])
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(index, _)| index)?;

        let current = open.swap_remove(current_index);

        if current == goal {
            let mut path = vec![current];
            let mut cursor = current;

            while let Some(previous) = came_from[cursor] {
                path.push(previous);
                cursor = previous;
            }

            path.reverse();
            return Some(path);
        }

        closed[current] = true;

        for &neighbor in &adjacency[current] {
            if closed[neighbor] {
                continue;
            }

            let tentative = g_score[current] + NAV_NODES[current].distance(NAV_NODES[neighbor]);

            if tentative >= g_score[neighbor] {
                continue;
            }

            came_from[neighbor] = Some(current);
            g_score[neighbor] = tentative;
            f_score[neighbor] = tentative + NAV_NODES[neighbor].distance(NAV_NODES[goal]);

            if !open.contains(&neighbor) {
                open.push(neighbor);
            }
        }
    }

    None
}

/// Return the next collision-safe waypoint toward `target`.
///
/// A direct path is preferred. When a wall or prop blocks it, navigation uses
/// room centers and doorway/corridor nodes matching the generated map.
pub fn next_waypoint(position: Vec2, target: Vec2, solids: &[(Vec2, Vec2)]) -> Vec2 {
    if !is_blocked(position, target, solids) {
        return target;
    }

    let start = nearest_node(position);
    let goal = nearest_node(target);

    if let Some(path) = a_star(start, goal) {
        for node in path.into_iter().skip(1) {
            let waypoint = NAV_NODES[node];

            if position.distance(waypoint) >= 18.0 {
                return waypoint;
            }
        }
    }

    // A safe central fallback beside the meeting table.
    NAV_NODES[2]
}

fn is_blocked(start: Vec2, end: Vec2, solids: &[(Vec2, Vec2)]) -> bool {
    solids.iter().any(|&(center, half_extents)| {
        segment_intersects_aabb(
            start,
            end,
            center,
            half_extents + Vec2::splat(NAV_CLEARANCE),
        )
    })
}

fn segment_intersects_aabb(start: Vec2, end: Vec2, center: Vec2, half_extents: Vec2) -> bool {
    let minimum = center - half_extents;
    let maximum = center + half_extents;
    let direction = end - start;

    let mut entry = 0.0_f32;
    let mut exit = 1.0_f32;

    for (origin, delta, lower, upper) in [
        (start.x, direction.x, minimum.x, maximum.x),
        (start.y, direction.y, minimum.y, maximum.y),
    ] {
        if delta.abs() <= f32::EPSILON {
            if origin < lower || origin > upper {
                return false;
            }

            continue;
        }

        let inverse = 1.0 / delta;
        let mut near = (lower - origin) * inverse;
        let mut far = (upper - origin) * inverse;

        if near > far {
            std::mem::swap(&mut near, &mut far);
        }

        entry = entry.max(near);
        exit = exit.min(far);

        if entry > exit {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::super::{BRIEFING_CENTER, MAP_BOUNDS};
    use super::*;

    #[test]
    fn direct_path_is_returned_when_clear() {
        let solids = [(Vec2::new(500.0, 500.0), Vec2::splat(10.0))];

        let target = Vec2::new(10.0, 0.0);

        assert_eq!(next_waypoint(Vec2::ZERO, target, &solids), target,);
    }

    #[test]
    fn blocked_path_uses_graph() {
        let solids = [(Vec2::new(50.0, 0.0), Vec2::new(20.0, 40.0))];

        let target = Vec2::new(100.0, 0.0);
        let waypoint = next_waypoint(Vec2::ZERO, target, &solids);

        assert_ne!(waypoint, target);
    }

    #[test]
    fn navigation_nodes_stay_inside_map() {
        for node in NAV_NODES {
            assert!(node.x.abs() <= MAP_BOUNDS.x);
            assert!(node.y.abs() <= MAP_BOUNDS.y);
        }
    }

    #[test]
    fn graph_connects_every_node() {
        for goal in 0..NAV_NODES.len() {
            assert!(a_star(0, goal).is_some());
        }
    }

    #[test]
    fn central_navigation_avoids_table() {
        let table = [(
            BRIEFING_CENTER + Vec2::new(0.0, 10.0),
            Vec2::new(44.0, 24.0),
        )];

        for &(left, right) in &NAV_EDGES {
            assert!(
                !is_blocked(NAV_NODES[left], NAV_NODES[right], &table),
                "edge {left}->{right} crosses briefing table"
            );
        }
    }
}
