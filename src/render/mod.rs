mod actors;
mod map;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bevy_ecs::prelude::*;
use glam::Vec2;
use repose_canvas::DrawScope;
use repose_core::{Color, Px, Rect, Vec2 as PaintVec2, effective_density_scale};
use repose_text::shape_line_cached;

use crate::app::AppState;

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

pub fn sync_world_render(world: &mut World, state: &mut RenderState, dt: Duration) -> WorldRender {
    if *world.resource::<AppState>() != AppState::InGame {
        state.walk.clear();
        return WorldRender::default();
    }
    let dt = dt.as_secs_f32();
    let mut items = Vec::with_capacity(256);
    map::push_map_ground(world, &mut items);
    actors::push_corpses(world, &mut items);
    map::push_map_fixtures(world, &mut items);
    actors::push_players(world, state, dt, &mut items);
    actors::follow_camera(world, state, dt);
    WorldRender {
        camera: state.camera,
        items: items.into(),
    }
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
        let render = sync_world_render(&mut world, &mut state, Duration::from_millis(16));
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
        let cleared = sync_world_render(&mut world, &mut state, Duration::from_millis(16));
        assert!(cleared.items.is_empty());
    }
}
