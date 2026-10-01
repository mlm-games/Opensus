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
    squash_x: f32,
    squash_y: f32,
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

// Legacy actor_z(y) = 30 - y * 0.01 with bodies drawn at z - 0.05: a corpse sorts as y + 5.
const CORPSE_Y_SORT_BIAS: f32 = 5.0;

pub(crate) fn push_actors(
    world: &mut World,
    images: &GameImages,
    colors: &HashMap<u64, u8>,
    state: &mut RenderState,
    dt: f32,
    items: &mut Vec<DrawItem>,
) {
    state.anim_time += dt;
    let mut layers: Vec<(f32, Vec<DrawItem>)> = Vec::new();

    let mut query = world.query::<(&Body, &Position)>();
    for (body, position) in query.iter(world) {
        let color_index = colors.get(&body.player_id).copied().unwrap_or(0);
        let size = Vec2::new(CHARACTER_HEIGHT * 0.7, CHARACTER_HEIGHT * 0.35);
        let tint = rgba(0.55, 0.1, 0.12, 0.95);
        let mut group = Vec::new();
        match images.clothes[color_index as usize % images.clothes.len()] {
            Some(handle) => push_image(
                &mut group,
                position.0,
                size,
                handle,
                tint,
                std::f32::consts::FRAC_PI_2,
                false,
                ImageFilter::Linear,
            ),
            None => push_rect(&mut group, position.0, size, tint, 6.0),
        }
        layers.push((position.0.y + CORPSE_Y_SORT_BIAS, group));
    }

    let mut query = world.query::<(&Player, &Position, Option<&Ghost>, Option<&PlayerIntent>)>();
    let mut poses: Vec<ActorPose> = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();
    for (player, position, ghost, intent) in query.iter(world) {
        let movement = intent.map(|intent| intent.movement).unwrap_or(Vec2::ZERO);
        let blend_target = movement.length().clamp(0.0, 1.0);
        let anim = state.walk.entry(player.id).or_default();
        anim.blend += (blend_target - anim.blend) * (1.0 - (-14.0 * dt).exp());
        anim.phase += dt * (8.5 + blend_target * 2.5) * blend_target;
        if movement.x.abs() >= 0.01 {
            anim.facing = if movement.x >= 0.0 { 1.0 } else { -1.0 };
        }
        let step = anim.phase.sin();
        seen.insert(player.id);
        // Legacy split: player_bob_and_y_sort owned layer y/scale.y, animate_player_layers owned rotation/scale.x.
        let moving = movement.length_squared() > 0.01;
        let t = state.anim_time * if moving { 13.5 } else { 2.0 };
        let bob = if moving {
            t.sin().abs() * 2.2
        } else {
            t.sin() * 0.25
        };
        let squash_y = if moving { 1.0 + t.sin() * 0.025 } else { 1.0 };
        poses.push(ActorPose {
            position: position.0,
            bob,
            lean: step * 0.025 * anim.blend,
            squash_x: step.abs() * 0.035 * anim.blend,
            squash_y,
            facing: anim.facing,
            color_index: player.color_index,
            name: player.name.clone(),
            ghost: ghost.is_some(),
        });
    }

    for pose in poses {
        let mut group = Vec::new();
        let alpha = if pose.ghost { 0.35 } else { 1.0 };
        let body_center = pose.position + Vec2::new(0.0, pose.bob);
        let size = Vec2::new(
            CHARACTER_HEIGHT * 0.75 * (1.0 - pose.squash_x * 0.35),
            CHARACTER_HEIGHT * pose.squash_y,
        );
        let color_index = pose.color_index as usize % PLAYER_COLORS.len();
        let tint = Color::WHITE.with_alpha_f32(alpha);
        let mirror = pose.facing < 0.0;
        if let Some(handle) = images.bodies[color_index] {
            push_image(
                &mut group,
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
                    &mut group,
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
            group.push(DrawItem::Rect {
                center: body_center,
                size,
                color: body_color,
                radius: 14.0,
                rotation: pose.lean,
            });
            group.push(DrawItem::Ellipse {
                center: body_center + Vec2::new(pose.facing * 10.0, 14.0),
                radii: Vec2::new(9.0, 6.0),
                color: rgba(0.78, 0.90, 0.94, alpha),
            });
        }
        group.push(DrawItem::Text {
            center: pose.position + Vec2::new(0.0, 22.0),
            text: pose.name,
            color: rgba(0.95, 0.95, 0.95, alpha),
            size: 14.0,
        });
        layers.push((pose.position.y, group));
    }

    state.walk.retain(|id, _| seen.contains(id));

    layers.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (_, mut group) in layers {
        items.append(&mut group);
    }
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
