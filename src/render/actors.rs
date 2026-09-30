use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::*;
use glam::Vec2;
use repose_core::{Color, ImageFilter};

use super::{DrawItem, RenderState, push_image, push_rect, rgba};
use crate::assets::GameImages;
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

pub(crate) fn player_colors(world: &mut World) -> HashMap<u64, u8> {
    let mut query = world.query::<&Player>();
    query
        .iter(world)
        .map(|player| (player.id, player.color_index))
        .collect()
}

pub(crate) fn push_corpses(
    world: &mut World,
    images: &GameImages,
    colors: &HashMap<u64, u8>,
    items: &mut Vec<DrawItem>,
) {
    let mut query = world.query::<(&Body, &Position)>();
    for (body, position) in query.iter(world) {
        let color_index = colors.get(&body.player_id).copied().unwrap_or(0);
        let size = Vec2::new(CHARACTER_HEIGHT * 0.7, CHARACTER_HEIGHT * 0.35);
        let tint = rgba(0.55, 0.1, 0.12, 0.95);
        match images.clothes[color_index as usize % images.clothes.len()] {
            Some(handle) => push_image(
                items,
                position.0,
                size,
                handle,
                tint,
                std::f32::consts::FRAC_PI_2,
                false,
                ImageFilter::Linear,
            ),
            None => push_rect(items, position.0, size, tint, 6.0),
        }
    }
}

pub(crate) fn push_players(
    world: &mut World,
    images: &GameImages,
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
        let size = Vec2::new(
            CHARACTER_HEIGHT * 0.75 * (1.0 - pose.squash * 0.35),
            CHARACTER_HEIGHT * (1.0 + pose.squash),
        );
        let color_index = pose.color_index as usize % PLAYER_COLORS.len();
        let tint = Color::WHITE.with_alpha_f32(alpha);
        let mirror = pose.facing < 0.0;
        if let Some(handle) = images.bodies[color_index] {
            push_image(
                items,
                body_center,
                size,
                handle,
                tint,
                pose.lean,
                mirror,
                ImageFilter::Nearest,
            );
            if let Some(clothes) = images.clothes[color_index] {
                push_image(
                    items,
                    body_center,
                    size,
                    clothes,
                    tint,
                    pose.lean,
                    mirror,
                    ImageFilter::Linear,
                );
            }
        } else {
            let body_color = PLAYER_COLORS[color_index].with_alpha_f32(alpha);
            items.push(DrawItem::Rect {
                center: body_center,
                size,
                color: body_color,
                radius: 14.0,
                rotation: pose.lean,
            });
            items.push(DrawItem::Ellipse {
                center: body_center + Vec2::new(pose.facing * 10.0, 14.0),
                radii: Vec2::new(9.0, 6.0),
                color: rgba(0.78, 0.90, 0.94, alpha),
            });
        }
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
