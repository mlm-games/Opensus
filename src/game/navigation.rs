use bevy::prelude::*;

/// Simple hub-based navigation to keep bots from pressing against walls
/// once collision is fixed. Full navmesh is overkill for this map; we use
/// room centers + doorway waypoints and A* across them.

#[derive(Clone, Copy, Debug)]
struct NavNode {
    pos: Vec2,
}

fn nodes() -> Vec<NavNode> {
    vec![
        NavNode {
            pos: Vec2::new(-280.0, 120.0),
        }, // 0 Archives
        NavNode {
            pos: Vec2::new(280.0, 120.0),
        }, // 1 Comms
        NavNode {
            pos: Vec2::new(-280.0, -120.0),
        }, // 2 Reactor
        NavNode {
            pos: Vec2::new(280.0, -120.0),
        }, // 3 Medbay
        NavNode {
            pos: Vec2::new(0.0, 0.0),
        }, // 4 Cafeteria (hub)
        // Doorway waypoints (approx gap centers)
        NavNode {
            pos: Vec2::new(-145.0, 55.0),
        }, // 5 Archives<->Cafeteria
        NavNode {
            pos: Vec2::new(145.0, 55.0),
        }, // 6 Comms<->Cafeteria
        NavNode {
            pos: Vec2::new(-290.0, 0.0),
        }, // 7 Archives<->Reactor
        NavNode {
            pos: Vec2::new(295.0, 0.0),
        }, // 8 Comms<->Medbay
        NavNode {
            pos: Vec2::new(-40.0, -80.0),
        }, // 9 Cafeteria south-west door
        NavNode {
            pos: Vec2::new(40.0, -80.0),
        }, // 10 Cafeteria south-east door
    ]
}

fn edges() -> Vec<(usize, usize)> {
    vec![
        (0, 5),
        (5, 4),
        (1, 6),
        (6, 4),
        (0, 7),
        (7, 2),
        (1, 8),
        (8, 3),
        (4, 9),
        (9, 2),
        (4, 10),
        (10, 3),
        (4, 7),
        (4, 8),
    ]
}

fn nearest_node(pos: Vec2) -> usize {
    let ns = nodes();
    let mut best = 0;
    let mut best_d = f32::MAX;
    for (i, n) in ns.iter().enumerate() {
        let d = pos.distance_squared(n.pos);
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best
}

fn a_star(start: usize, goal: usize) -> Option<Vec<usize>> {
    let ns = nodes();
    let es = edges();
    let n = ns.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (a, b) in es {
        adj[a].push(b);
        adj[b].push(a);
    }

    let mut open = vec![start];
    let mut came_from: Vec<Option<usize>> = vec![None; n];
    let mut g_score = vec![f32::INFINITY; n];
    let mut f_score = vec![f32::INFINITY; n];
    g_score[start] = 0.0;
    f_score[start] = ns[start].pos.distance(ns[goal].pos);

    let mut closed = vec![false; n];

    while let Some(current) = {
        let mut best: Option<usize> = None;
        let mut best_f = f32::INFINITY;
        for &node in &open {
            if f_score[node] < best_f {
                best_f = f_score[node];
                best = Some(node);
            }
        }
        best
    } {
        if current == goal {
            let mut path = vec![current];
            let mut cur = current;
            while let Some(prev) = came_from[cur] {
                path.push(prev);
                cur = prev;
            }
            path.reverse();
            return Some(path);
        }

        open.retain(|&x| x != current);
        closed[current] = true;

        for &neighbor in &adj[current] {
            if closed[neighbor] {
                continue;
            }
            let tentative = g_score[current] + ns[current].pos.distance(ns[neighbor].pos);
            if tentative < g_score[neighbor] {
                came_from[neighbor] = Some(current);
                g_score[neighbor] = tentative;
                f_score[neighbor] = tentative + ns[neighbor].pos.distance(ns[goal].pos);
                if !open.contains(&neighbor) {
                    open.push(neighbor);
                }
            }
        }
    }
    None
}

/// Returns the next waypoint towards target, or target itself if direct path is clear.
/// If direct segment is blocked by any solid, returns a graph waypoint.
pub fn next_waypoint(pos: Vec2, target: Vec2, solids: &[(Vec2, Vec2)]) -> Vec2 {
    if !is_blocked(pos, target, solids) {
        return target;
    }
    // Use graph path via nearest nodes.
    let ns = nodes();
    let start = nearest_node(pos);
    let goal = nearest_node(target);
    if let Some(path) = a_star(start, goal) {
        // path[0] is start node, so next is path[1] if exists.
        if path.len() >= 2 {
            let wp = ns[path[1]].pos;
            // If we are already very close to wp, skip to next.
            if pos.distance(wp) < 18.0 && path.len() >= 3 {
                return ns[path[2]].pos;
            }
            return wp;
        }
    }
    // Fallback: go to cafeteria hub.
    Vec2::ZERO
}

fn is_blocked(a: Vec2, b: Vec2, solids: &[(Vec2, Vec2)]) -> bool {
    for &(center, half) in solids {
        if segment_intersects_aabb(a, b, center, half) {
            return true;
        }
    }
    false
}

fn segment_intersects_aabb(p1: Vec2, p2: Vec2, center: Vec2, half: Vec2) -> bool {
    let min = center - half;
    let max = center + half;

    // Quick reject: segment bbox vs aabb
    let seg_min = p1.min(p2);
    let seg_max = p1.max(p2);
    if seg_max.x < min.x || seg_min.x > max.x || seg_max.y < min.y || seg_min.y > max.y {
        return false;
    }

    // Check if either endpoint inside
    if point_in_aabb(p1, min, max) || point_in_aabb(p2, min, max) {
        return true;
    }

    // Check intersection with 4 edges
    let corners = [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)];
    for i in 0..4 {
        let a1 = corners[i];
        let a2 = corners[(i + 1) % 4];
        if segments_intersect(p1, p2, a1, a2) {
            return true;
        }
    }

    // Sample along segment as extra safety for thick walls
    for t in [0.25, 0.5, 0.75] {
        let p = p1.lerp(p2, t);
        if point_in_aabb(p, min, max) {
            return true;
        }
    }

    false
}

fn point_in_aabb(p: Vec2, min: Vec2, max: Vec2) -> bool {
    p.x >= min.x && p.x <= max.x && p.y >= min.y && p.y <= max.y
}

fn segments_intersect(p1: Vec2, p2: Vec2, q1: Vec2, q2: Vec2) -> bool {
    let o1 = orientation(p1, p2, q1);
    let o2 = orientation(p1, p2, q2);
    let o3 = orientation(q1, q2, p1);
    let o4 = orientation(q1, q2, p2);
    o1 != o2 && o3 != o4
}

fn orientation(a: Vec2, b: Vec2, c: Vec2) -> i32 {
    let v = (b.y - a.y) * (c.x - b.x) - (b.x - a.x) * (c.y - b.y);
    if v.abs() < 1e-6 {
        0
    } else if v > 0.0 {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_unblocked() {
        let solids = vec![(Vec2::new(500.0, 500.0), Vec2::new(10.0, 10.0))];
        assert_eq!(
            next_waypoint(Vec2::ZERO, Vec2::new(10.0, 0.0), &solids),
            Vec2::new(10.0, 0.0)
        );
    }

    #[test]
    fn blocked_uses_waypoint() {
        // Wall between (0,0) and (100,0)
        let solids = vec![(Vec2::new(50.0, 0.0), Vec2::new(20.0, 40.0))];
        let wp = next_waypoint(Vec2::ZERO, Vec2::new(100.0, 0.0), &solids);
        // Should not be direct target
        assert_ne!(wp, Vec2::new(100.0, 0.0));
    }
}
