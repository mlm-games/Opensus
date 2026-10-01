mod actors;
mod map;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bevy_ecs::prelude::*;
use glam::Vec2;
use repose_canvas::DrawScope;
use repose_core::{
    Color, ImageFilter, ImageFit, ImageHandle, Px, Rect, Transform, Vec2 as PaintVec2,
    effective_density_scale,
};
use repose_text::shape_line_cached;

use crate::app::AppState;
use crate::assets::{GameImages, MASK_WORLD_SIZE};
use crate::game::{
    ActiveSabotage, GamePhase, Ghost, LocalPlayer, LocalRole, Position, Role, SabotageKind,
};

pub enum DrawItem {
    Rect {
        center: Vec2,
        size: Vec2,
        color: Color,
        radius: f32,
        rotation: f32,
    },
    Ellipse {
        center: Vec2,
        radii: Vec2,
        color: Color,
    },
    Image {
        center: Vec2,
        size: Vec2,
        handle: ImageHandle,
        tint: Color,
        rotation: f32,
        mirror: bool,
        filter: ImageFilter,
    },
    Text {
        center: Vec2,
        text: String,
        color: Color,
        size: f32,
    },
}

#[derive(Clone, Default)]
pub struct WorldRender {
    pub camera: Vec2,
    pub items: Arc<[DrawItem]>,
}

#[derive(Default)]
pub struct RenderState {
    pub camera: Vec2,
    shake_time: f32,
    walk: HashMap<u64, WalkAnim>,
}

pub struct WalkAnim {
    pub blend: f32,
    pub phase: f32,
    pub facing: f32,
}

impl Default for WalkAnim {
    fn default() -> Self {
        Self {
            blend: 0.0,
            phase: 0.0,
            facing: 1.0,
        }
    }
}

pub fn sync_world_render(
    world: &mut World,
    state: &mut RenderState,
    images: &GameImages,
    dt: Duration,
) -> WorldRender {
    if *world.resource::<AppState>() != AppState::InGame {
        state.walk.clear();
        state.shake_time = 0.0;
        return WorldRender::default();
    }
    let dt = dt.as_secs_f32();
    let mut items = Vec::with_capacity(320);
    let colors = actors::player_colors(world);
    map::push_map_ground(world, images, &mut items);
    actors::push_corpses(world, images, &colors, &mut items);
    map::push_map_fixtures(world, images, &mut items);
    actors::push_players(world, images, state, dt, &mut items);
    push_vision_mask(world, images, &mut items);
    actors::follow_camera(world, state, dt);
    state.shake_time += dt;
    let trauma = world
        .get_resource::<crate::game::Trauma>()
        .map_or(0.0, |trauma| trauma.value);
    let shake = game_utils_repame::feel::shake_offset(state.shake_time, trauma * trauma, 1.0);
    WorldRender {
        camera: state.camera + shake,
        items: items.into(),
    }
}

fn push_vision_mask(world: &mut World, images: &GameImages, items: &mut Vec<DrawItem>) {
    if !matches!(world.get_resource::<GamePhase>(), Some(GamePhase::Playing)) {
        return;
    }
    let role = world.get_resource::<LocalRole>().and_then(|role| role.0);
    let lights = world
        .get_resource::<ActiveSabotage>()
        .is_some_and(|sabotage| sabotage.kind == Some(SabotageKind::Lights));
    let mut query = world.query_filtered::<(&Position, Option<&Ghost>), With<LocalPlayer>>();
    let Ok((position, ghost)) = query.single(world) else {
        return;
    };
    if ghost.is_some() {
        return;
    }
    let handle = match role {
        Some(Role::Impostor) => images.vision_impostor,
        Some(Role::Crewmate) if lights => images.vision_lights,
        _ => images.vision_crew,
    };
    let Some(handle) = handle else {
        return;
    };
    push_image(
        items,
        position.0,
        Vec2::splat(MASK_WORLD_SIZE),
        handle,
        Color::WHITE,
        0.0,
        false,
        ImageFilter::Linear,
    );
}

pub fn draw_world(scope: &mut DrawScope, render: &WorldRender) {
    let scale = effective_density_scale();
    let screen_center = PaintVec2 {
        x: scope.size.width * 0.5,
        y: scope.size.height * 0.5,
    };
    for item in render.items.iter() {
        match item {
            DrawItem::Rect {
                center,
                size,
                color,
                radius,
                rotation,
            } => {
                let p = to_paint(*center, render, screen_center, scale);
                let w = size.x * scale;
                let h = size.y * scale;
                if culled(p, w, h, scope) {
                    continue;
                }
                let rect = Rect {
                    x: p.x - w * 0.5,
                    y: p.y - h * 0.5,
                    w,
                    h,
                };
                if rotation.abs() > 1e-6 {
                    scope.draw_rect_rotated(rect, *color, Px(radius * scale), -*rotation, p);
                } else {
                    scope.draw_rect(rect, *color, Px(radius * scale));
                }
            }
            DrawItem::Ellipse {
                center,
                radii,
                color,
            } => {
                let p = to_paint(*center, render, screen_center, scale);
                let w = radii.x * 2.0 * scale;
                let h = radii.y * 2.0 * scale;
                if culled(p, w, h, scope) {
                    continue;
                }
                scope.draw_ellipse(p, radii.x * scale, radii.y * scale, *color);
            }
            DrawItem::Image {
                center,
                size,
                handle,
                tint,
                rotation,
                mirror,
                filter,
            } => {
                let p = to_paint(*center, render, screen_center, scale);
                let w = size.x * scale;
                let h = size.y * scale;
                if culled(p, w, h, scope) {
                    continue;
                }
                let rect = Rect {
                    x: p.x - w * 0.5,
                    y: p.y - h * 0.5,
                    w,
                    h,
                };
                let rotation = -*rotation;
                if rotation.abs() > 1e-6 || *mirror {
                    let mut spin = Transform::identity();
                    spin.rotate = rotation;
                    let mut mirror_scale = Transform::identity();
                    mirror_scale.scale_x = if *mirror { -1.0 } else { 1.0 };
                    let mut t = Transform::translate(p.x, p.y)
                        .combine(&spin)
                        .combine(&mirror_scale)
                        .combine(&Transform::translate(-p.x, -p.y));
                    t.origin_x = 0.0;
                    t.origin_y = 0.0;
                    scope.push_transform(t);
                    scope.draw_image_filtered(
                        rect,
                        *handle,
                        None,
                        *tint,
                        ImageFit::FillBounds,
                        *filter,
                    );
                    scope.pop_transform();
                } else {
                    scope.draw_image_filtered(
                        rect,
                        *handle,
                        None,
                        *tint,
                        ImageFit::FillBounds,
                        *filter,
                    );
                }
            }
            DrawItem::Text {
                center,
                text,
                color,
                size,
            } => {
                let px = size * scale;
                let width = text_width(text, px);
                let p = to_paint(*center, render, screen_center, scale);
                if culled(p, width, px, scope) {
                    continue;
                }
                scope.draw_text(
                    text,
                    PaintVec2 {
                        x: p.x - width * 0.5,
                        y: p.y - px * 0.5,
                    },
                    *color,
                    Px(px),
                );
            }
        }
    }
}

fn to_paint(p: Vec2, render: &WorldRender, screen_center: PaintVec2, scale: f32) -> PaintVec2 {
    PaintVec2 {
        x: screen_center.x + (p.x - render.camera.x) * scale,
        y: screen_center.y - (p.y - render.camera.y) * scale,
    }
}

fn culled(p: PaintVec2, w: f32, h: f32, scope: &DrawScope) -> bool {
    let margin_x = w * 0.5 + 4.0;
    let margin_y = h * 0.5 + 4.0;
    p.x + margin_x < 0.0
        || p.x - margin_x > scope.size.width
        || p.y + margin_y < 0.0
        || p.y - margin_y > scope.size.height
}

fn text_width(text: &str, px: f32) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let shaped = shape_line_cached(text, px, 1.0, None, 400, 0, 0.0, None);
    shaped
        .glyphs
        .iter()
        .map(|glyph| glyph.x + glyph.advance)
        .fold(0.0_f32, f32::max)
}

pub(crate) fn push_rect(
    items: &mut Vec<DrawItem>,
    center: Vec2,
    size: Vec2,
    color: Color,
    radius: f32,
) {
    items.push(DrawItem::Rect {
        center,
        size,
        color,
        radius,
        rotation: 0.0,
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_image(
    items: &mut Vec<DrawItem>,
    center: Vec2,
    size: Vec2,
    handle: ImageHandle,
    tint: Color,
    rotation: f32,
    mirror: bool,
    filter: ImageFilter,
) {
    items.push(DrawItem::Image {
        center,
        size,
        handle,
        tint,
        rotation,
        mirror,
        filter,
    });
}

pub(crate) fn push_surface(
    items: &mut Vec<DrawItem>,
    center: Vec2,
    size: Vec2,
    handle: Option<ImageHandle>,
    tint: Color,
    radius: f32,
) {
    match handle {
        Some(handle) => push_image(
            items,
            center,
            size,
            handle,
            tint,
            0.0,
            false,
            ImageFilter::Linear,
        ),
        None => push_rect(items, center, size, tint, radius),
    }
}

pub(crate) fn rgb(r: f32, g: f32, b: f32) -> Color {
    rgba(r, g, b, 1.0)
}

pub(crate) fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
    Color(
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
        (a * 255.0).round() as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{
        Body, LocalPlayer, MAP_BOUNDS, PLAYER_COLORS, Player, PlayerIntent, Position,
    };

    fn local_player(world: &mut World, position: Vec2, movement: Vec2) {
        world.spawn((
            Player {
                id: 1,
                name: "Agent".to_string(),
                color_index: 3,
                speed: 240.0,
            },
            LocalPlayer,
            Position(position),
            PlayerIntent {
                movement,
                interact: false,
            },
        ));
    }

    #[test]
    fn camera_eases_toward_local_player_with_look_ahead() {
        let mut world = World::new();
        local_player(&mut world, Vec2::new(500.0, 300.0), Vec2::X);
        let mut state = RenderState::default();
        for _ in 0..120 {
            actors::follow_camera(&mut world, &mut state, 0.05);
        }
        assert!((state.camera - Vec2::new(572.0, 300.0)).length() < 1.0);
    }

    #[test]
    fn camera_look_ahead_clamps_to_map_bounds() {
        let mut world = World::new();
        local_player(
            &mut world,
            MAP_BOUNDS + Vec2::splat(200.0),
            Vec2::new(1.0, 1.0),
        );
        let mut state = RenderState::default();
        for _ in 0..120 {
            actors::follow_camera(&mut world, &mut state, 0.05);
        }
        assert!((state.camera - MAP_BOUNDS).length() < 1.0);
    }

    #[test]
    fn sync_builds_map_and_actors_only_ingame() {
        let mut world = World::new();
        world.insert_resource(AppState::InGame);
        crate::game::map::spawn_map(&mut world);
        local_player(&mut world, Vec2::new(100.0, 50.0), Vec2::ZERO);
        world.spawn((
            Body {
                player_id: 2,
                name: "Bot".to_string(),
                reported: false,
            },
            Position(Vec2::new(-40.0, 20.0)),
        ));

        let mut state = RenderState::default();
        let render = sync_world_render(
            &mut world,
            &mut state,
            &GameImages::default(),
            Duration::from_millis(16),
        );
        assert!(render.items.len() > 50);
        assert!(
            render
                .items
                .iter()
                .any(|item| matches!(item, DrawItem::Text { text, .. } if text == "Agent"))
        );
        let body_color = PLAYER_COLORS[3].with_alpha_f32(1.0);
        assert!(
            render
                .items
                .iter()
                .any(|item| matches!(item, DrawItem::Rect { color, size, .. }
                if *color == body_color && (size.y - 56.0).abs() < 0.001))
        );
        assert!(
            render
                .items
                .iter()
                .any(|item| matches!(item, DrawItem::Rect { color, .. }
                if color.0 == 140 && color.1 == 26 && color.2 == 31))
        );

        world.insert_resource(AppState::Title);
        let cleared = sync_world_render(
            &mut world,
            &mut state,
            &GameImages::default(),
            Duration::from_millis(16),
        );
        assert!(cleared.items.is_empty());
    }

    #[test]
    fn sync_emits_images_for_loaded_surfaces_and_actors() {
        let mut world = World::new();
        world.insert_resource(AppState::InGame);
        crate::game::map::spawn_map(&mut world);
        local_player(&mut world, Vec2::new(100.0, 50.0), Vec2::ZERO);
        world.spawn((
            Player {
                id: 2,
                name: "Victim".to_string(),
                color_index: 2,
                speed: 240.0,
            },
            Position(Vec2::new(300.0, 100.0)),
        ));
        world.spawn((
            Body {
                player_id: 2,
                name: "Victim".to_string(),
                reported: false,
            },
            Position(Vec2::new(-40.0, 20.0)),
        ));

        let images = GameImages {
            floor_carpet: Some(7),
            door: Some(8),
            bodies: std::array::from_fn(|index| match index {
                3 => Some(11),
                2 => Some(14),
                _ => None,
            }),
            clothes: std::array::from_fn(|index| match index {
                3 => Some(12),
                2 => Some(13),
                _ => None,
            }),
            ..Default::default()
        };

        let mut state = RenderState::default();
        let render = sync_world_render(&mut world, &mut state, &images, Duration::from_millis(16));
        assert!(
            render
                .items
                .iter()
                .any(|item| matches!(item, DrawItem::Image { handle: 7, .. }))
        );
        assert!(
            render
                .items
                .iter()
                .any(|item| matches!(item, DrawItem::Image { handle: 8, .. }))
        );
        assert!(
            render
                .items
                .iter()
                .any(|item| matches!(item, DrawItem::Image { handle: 11, filter, .. } if *filter == ImageFilter::Nearest))
        );
        assert!(render.items.iter().any(
            |item| matches!(item, DrawItem::Image { handle: 12, rotation, .. } if *rotation == 0.0)
        ));
        assert!(
            render
                .items
                .iter()
                .any(|item| matches!(item, DrawItem::Image { handle: 13, rotation, .. } if (rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-6))
        );
        assert!(
            !render
                .items
                .iter()
                .any(|item| matches!(item, DrawItem::Ellipse { .. }))
        );
    }

    #[test]
    fn camera_shakes_only_while_trauma_active() {
        let mut world = World::new();
        world.insert_resource(AppState::InGame);
        local_player(&mut world, Vec2::new(100.0, 50.0), Vec2::ZERO);
        let mut state = RenderState::default();

        let calm = sync_world_render(
            &mut world,
            &mut state,
            &GameImages::default(),
            Duration::from_millis(16),
        );
        assert_eq!(calm.camera, state.camera);

        world.insert_resource(crate::game::Trauma { value: 0.8 });
        let shaken = sync_world_render(
            &mut world,
            &mut state,
            &GameImages::default(),
            Duration::from_millis(16),
        );
        let offset = shaken.camera - state.camera;
        assert!(
            offset.length() > 0.3 && offset.length() < 0.95,
            "offset {offset:?}"
        );
    }

    #[test]
    fn fix_stations_draw_only_for_active_kind() {
        use crate::game::{
            ActiveSabotage, OXYGEN_STATIONS, REACTOR_STATIONS, SabotageFixStation, SabotageKind,
        };

        let mut world = World::new();
        world.insert_resource(AppState::InGame);
        local_player(&mut world, Vec2::ZERO, Vec2::ZERO);
        world.spawn((
            SabotageFixStation {
                id: 0,
                kind: SabotageKind::Oxygen,
                progress: 0.0,
            },
            Position(OXYGEN_STATIONS[0]),
        ));
        world.spawn((
            SabotageFixStation {
                id: 2,
                kind: SabotageKind::Reactor,
                progress: 0.0,
            },
            Position(REACTOR_STATIONS[0]),
        ));
        let mut state = RenderState::default();

        let idle = sync_world_render(
            &mut world,
            &mut state,
            &GameImages::default(),
            Duration::from_millis(16),
        );
        assert!(!idle.items.iter().any(|item| matches!(item,
                DrawItem::Rect { color, .. }
                if *color == rgba(1.0, 0.6, 0.15, 1.0))));

        world.insert_resource(ActiveSabotage {
            kind: Some(SabotageKind::Oxygen),
            timer: None,
            fixes_needed: 2,
            fixes_done: 0,
        });
        let active = sync_world_render(
            &mut world,
            &mut state,
            &GameImages::default(),
            Duration::from_millis(16),
        );
        assert!(active.items.iter().any(|item| matches!(item,
                DrawItem::Rect { center, color, .. }
                if *center == OXYGEN_STATIONS[0] && *color == rgba(1.0, 0.6, 0.15, 1.0))));
        assert!(!active.items.iter().any(|item| matches!(item,
                DrawItem::Rect { center, color, .. }
                if *center == REACTOR_STATIONS[0] && *color == rgba(1.0, 0.6, 0.15, 1.0))));

        let mut q = world.query::<(&mut SabotageFixStation, &Position)>();
        for (mut station, position) in q.iter_mut(&mut world) {
            if position.0 == OXYGEN_STATIONS[0] {
                station.progress = 1.0;
            }
        }
        let fixed = sync_world_render(
            &mut world,
            &mut state,
            &GameImages::default(),
            Duration::from_millis(16),
        );
        assert!(fixed.items.iter().any(|item| matches!(item,
                DrawItem::Rect { center, color, .. }
                if *center == OXYGEN_STATIONS[0] && *color == rgba(0.45, 0.75, 0.5, 0.9))));
    }

    #[test]
    fn vision_mask_tracks_role_lights_and_phase() {
        let mut world = World::new();
        world.insert_resource(AppState::InGame);
        world.insert_resource(GamePhase::Playing);
        world.insert_resource(LocalRole(Some(Role::Crewmate)));
        local_player(&mut world, Vec2::new(40.0, -20.0), Vec2::ZERO);
        let images = GameImages {
            vision_crew: Some(21),
            vision_lights: Some(22),
            vision_impostor: Some(23),
            ..Default::default()
        };
        let mut state = RenderState::default();

        let crew = sync_world_render(&mut world, &mut state, &images, Duration::from_millis(16));
        assert!(crew.items.iter().any(|item| matches!(item,
                DrawItem::Image { handle: 21, center, size, filter, .. }
                if *center == Vec2::new(40.0, -20.0)
                    && *size == Vec2::splat(1800.0)
                    && *filter == ImageFilter::Linear)));

        world.insert_resource(ActiveSabotage {
            kind: Some(SabotageKind::Lights),
            ..Default::default()
        });
        let lights = sync_world_render(&mut world, &mut state, &images, Duration::from_millis(16));
        assert!(
            lights
                .items
                .iter()
                .any(|item| matches!(item, DrawItem::Image { handle: 22, .. }))
        );

        world.insert_resource(LocalRole(Some(Role::Impostor)));
        let impostor =
            sync_world_render(&mut world, &mut state, &images, Duration::from_millis(16));
        assert!(
            impostor
                .items
                .iter()
                .any(|item| matches!(item, DrawItem::Image { handle: 23, .. }))
        );

        let local = {
            let mut q = world.query_filtered::<Entity, With<LocalPlayer>>();
            q.single(&world).unwrap()
        };
        world.entity_mut(local).insert(Ghost);
        let ghost = sync_world_render(&mut world, &mut state, &images, Duration::from_millis(16));
        assert!(!ghost.items.iter().any(|item| matches!(
            item,
            DrawItem::Image {
                handle: 21..=23,
                ..
            }
        )));

        world.entity_mut(local).remove::<Ghost>();
        world.insert_resource(GamePhase::Meeting);
        let meeting = sync_world_render(&mut world, &mut state, &images, Duration::from_millis(16));
        assert!(!meeting.items.iter().any(|item| matches!(
            item,
            DrawItem::Image {
                handle: 21..=23,
                ..
            }
        )));
    }
}
