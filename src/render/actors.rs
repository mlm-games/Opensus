use std::collections::HashSet;

use bevy_ecs::prelude::*;
use glam::Vec2;

use super::{DrawItem, RenderState, push_rect, rgba};
use crate::game::{
    Body, Ghost, LocalPlayer, MAP_BOUNDS, PLAYER_COLORS, Player, PlayerIntent, Position,
};

const CHARACTER_HEIGHT: f32 = 56.0;
const CAMERA_FOLLOW_RATE: f32 = 9.0;
const CAMERA_LOOK_AHEAD: f32 = 72.0;

struct ActorPose {
    position: Vec2,
    bob: f32,
    lean: f32,
    squash: f32,
    facing: f32,
    color_index: u8,
    name: String,
    ghost: bool,
}

pub(crate) fn push_corpses(world: &mut World, items: &mut Vec<DrawItem>) {
    let mut query = world.query::<(&Body, &Position)>();
    for (_, position) in query.iter(world) {
        push_rect(
            items,
            position.0,
            Vec2::new(CHARACTER_HEIGHT * 0.7, CHARACTER_HEIGHT * 0.35),
            rgba(0.55, 0.1, 0.12, 0.95),
            6.0,
        );
    }
}

pub(crate) fn push_players(
    world: &mut World,
    state: &mut RenderState,
    dt: f32,
    items: &mut Vec<DrawItem>,
) {
    let mut query = world.query::<(&Player, &Position, Option<&Ghost>, Option<&PlayerIntent>)>();
    let mut poses: Vec<ActorPose> = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();
    for (player, position, ghost, intent) in query.iter(world) {
        let movement = intent.map(|intent| intent.movement).unwrap_or(Vec2::ZERO);
        let moving = movement.length().clamp(0.0, 1.0);
        let anim = state.walk.entry(player.id).or_default();
        anim.blend += (moving - anim.blend) * (1.0 - (-14.0 * dt).exp());
        anim.phase += dt * (8.5 + moving * 2.5) * moving;
        if movement.x.abs() >= 0.01 {
            anim.facing = if movement.x >= 0.0 { 1.0 } else { -1.0 };
        }
        let step = anim.phase.sin();
        seen.insert(player.id);
        poses.push(ActorPose {
            position: position.0,
            bob: step.abs() * 2.4 * anim.blend,
            lean: step * 0.025 * anim.blend,
            squash: step.abs() * 0.035 * anim.blend,
            facing: anim.facing,
            color_index: player.color_index,
            name: player.name.clone(),
            ghost: ghost.is_some(),
        });
    }

    poses.sort_by(|a, b| b.position.y.total_cmp(&a.position.y));

    for pose in poses {
        let alpha = if pose.ghost { 0.35 } else { 1.0 };
        let body_center = pose.position + Vec2::new(0.0, pose.bob);
        let body_color =
            PLAYER_COLORS[pose.color_index as usize % PLAYER_COLORS.len()].with_alpha_f32(alpha);
        items.push(DrawItem::Rect {
            center: body_center,
            size: Vec2::new(
                CHARACTER_HEIGHT * 0.75 * (1.0 - pose.squash * 0.35),
                CHARACTER_HEIGHT * (1.0 + pose.squash),
            ),
            color: body_color,
            radius: 14.0,
            rotation: pose.lean,
        });
        items.push(DrawItem::Ellipse {
            center: body_center + Vec2::new(pose.facing * 10.0, 14.0),
            radii: Vec2::new(9.0, 6.0),
            color: rgba(0.78, 0.90, 0.94, alpha),
        });
        items.push(DrawItem::Text {
            center: pose.position + Vec2::new(0.0, 22.0),
            text: pose.name,
            color: rgba(0.95, 0.95, 0.95, alpha),
            size: 14.0,
        });
    }

    state.walk.retain(|id, _| seen.contains(id));
}

pub(crate) fn follow_camera(world: &mut World, state: &mut RenderState, dt: f32) {
    let mut query = world.query_filtered::<(&Position, &PlayerIntent), With<LocalPlayer>>();
    let Ok((position, intent)) = query.single(world) else {
        return;
    };
    let desired = (position.0 + intent.movement.normalize_or_zero() * CAMERA_LOOK_AHEAD)
        .clamp(-MAP_BOUNDS, MAP_BOUNDS);
    let blend = 1.0 - (-CAMERA_FOLLOW_RATE * dt).exp();
    state.camera = state.camera.lerp(desired, blend);
}
