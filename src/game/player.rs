use bevy::prelude::*;
use rand::seq::SliceRandom;

use super::{Alive, GamePhase, LocalRole, MatchCleanup, MatchConfig, Role};
use crate::app::{AppState, Paused};
use crate::game::lobby::LobbyState;
use crate::save::SaveData;
use game_utils_bevy::juice::Juice;
use game_utils_bevy::screen_effects::CameraBase;
use game_utils_bevy::transitions::Transition;

#[derive(Component)]
pub struct Player {
    pub id: u64,
    pub name: String,
    #[expect(dead_code)]
    pub color_index: u8,
    pub speed: f32,
}

#[derive(Component)]
pub struct LocalPlayer;

#[derive(Component)]
pub struct AiPlayer {
    pub dir_timer: Timer,
    pub dir: Vec2,
}

pub struct PlayerPlugin;
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), spawn_players_from_lobby)
            .add_systems(
                OnExit(AppState::InGame),
                |mut cam: Query<&mut CameraBase, With<Camera2d>>| {
                    if let Ok(mut base) = cam.single_mut() {
                        base.translation = Vec3::new(0.0, 0.0, 1000.0);
                    }
                },
            )
            .add_systems(
                Update,
                (local_movement, ai_movement, camera_follow)
                    .run_if(in_state(AppState::InGame))
                    .run_if(|p: Res<Paused>| !p.0)
                    .run_if(|t: Res<Transition<AppState>>| !t.block_input)
                    .run_if(|ph: Res<GamePhase>| matches!(*ph, GamePhase::Playing)),
            );
    }
}

fn spawn_players_from_lobby(
    mut commands: Commands,
    lobby: Res<LobbyState>,
    cfg: Res<MatchConfig>,
    save: Res<SaveData>,
    mut local_role: ResMut<LocalRole>,
) {
    let mut slots: Vec<_> = lobby.slots.to_vec();
    if slots.is_empty() {
        slots.push(super::lobby::LobbySlot {
            id: 1,
            name: save.player_name.clone(),
            color_index: save.preferred_color_index,
            ready: true,
            is_local: true,
            is_host: true,
        });
        // bot crewmates for sandbox
        for i in 0..3 {
            slots.push(super::lobby::LobbySlot {
                id: 10 + i,
                name: format!("Agent-{}", i + 2),
                color_index: ((save.preferred_color_index as u64 + 1 + i)
                    % PLAYER_COLORS.len() as u64) as u8,
                ready: true,
                is_local: false,
                is_host: false,
            });
        }
    }

    let n = slots.len();
    let imp_count = (cfg.impostor_count as usize)
        .min(n.saturating_sub(1))
        .max(if n >= 2 { 1 } else { 0 });
    let mut indices: Vec<usize> = (0..n).collect();
    indices.shuffle(&mut rand::rng());
    let impostor_ids: std::collections::HashSet<u64> = indices
        .into_iter()
        .take(imp_count)
        .map(|i| slots[i].id)
        .collect();

    let start_positions = [
        Vec2::new(-80.0, 0.0),
        Vec2::new(80.0, 0.0),
        Vec2::new(0.0, 80.0),
        Vec2::new(0.0, -80.0),
        Vec2::new(-120.0, 60.0),
        Vec2::new(120.0, -60.0),
        Vec2::new(-60.0, -120.0),
        Vec2::new(60.0, 120.0),
    ];

    for (i, slot) in slots.iter().enumerate() {
        let role = if impostor_ids.contains(&slot.id) {
            Role::Impostor
        } else {
            Role::Crewmate
        };
        if slot.is_local {
            local_role.0 = Some(role);
        }
        let color = PLAYER_COLORS[slot.color_index as usize % PLAYER_COLORS.len()];
        let pos = start_positions[i % start_positions.len()];
        let mut e = commands.spawn((
            MatchCleanup,
            Player {
                id: slot.id,
                name: slot.name.clone(),
                color_index: slot.color_index,
                speed: cfg.player_speed,
            },
            role,
            Alive,
            Sprite {
                color,
                custom_size: Some(Vec2::splat(28.0)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 10.0),
        ));
        let id = e.id();
        if slot.is_local {
            e.insert(LocalPlayer);
        } else {
            e.insert(AiPlayer {
                dir_timer: Timer::from_seconds(1.5, TimerMode::Repeating),
                dir: Vec2::ZERO,
            });
        }
        Juice::pop_in(&mut commands, id, 0.35);
    }
}

fn local_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut q: Query<(&Player, &mut Transform), (With<LocalPlayer>, With<Alive>)>,
) {
    let Ok((p, mut tf)) = q.single_mut() else {
        return;
    };
    let mut d = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        d.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        d.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        d.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        d.x += 1.0;
    }
    if d != Vec2::ZERO {
        tf.translation += (d.normalize() * p.speed * time.delta_secs()).extend(0.0);
        // soft map bounds (map plugin can refine)
        tf.translation.x = tf.translation.x.clamp(-520.0, 520.0);
        tf.translation.y = tf.translation.y.clamp(-300.0, 300.0);
    }
}

fn ai_movement(
    time: Res<Time>,
    mut q: Query<(&Player, &mut AiPlayer, &mut Transform), With<Alive>>,
) {
    for (p, mut ai, mut tf) in &mut q {
        ai.dir_timer.tick(time.delta());
        if ai.dir_timer.just_finished() {
            let angle = rand::random::<f32>() * std::f32::consts::TAU;
            ai.dir = Vec2::new(angle.cos(), angle.sin());
        }
        if ai.dir != Vec2::ZERO {
            tf.translation += (ai.dir * p.speed * 0.55 * time.delta_secs()).extend(0.0);
            tf.translation.x = tf.translation.x.clamp(-520.0, 520.0);
            tf.translation.y = tf.translation.y.clamp(-300.0, 300.0);
        }
    }
}

fn camera_follow(
    mut camera: Query<&mut CameraBase, With<Camera2d>>,
    local: Query<&Transform, (With<LocalPlayer>, Without<Camera2d>)>,
) {
    let Ok(mut base) = camera.single_mut() else {
        return;
    };
    let Ok(tf) = local.single() else { return };
    let target = Vec3::new(
        tf.translation.x.clamp(-320.0, 320.0),
        tf.translation.y.clamp(-160.0, 160.0),
        1000.0,
    );
    base.translation = base.translation.lerp(target, 0.12);
}

pub const PLAYER_COLORS: [Color; 12] = [
    Color::srgb(0.9, 0.2, 0.2),
    Color::srgb(0.2, 0.45, 0.95),
    Color::srgb(0.2, 0.75, 0.35),
    Color::srgb(0.95, 0.85, 0.2),
    Color::srgb(0.85, 0.35, 0.85),
    Color::srgb(0.95, 0.55, 0.15),
    Color::srgb(0.3, 0.9, 0.9),
    Color::srgb(0.55, 0.25, 0.75),
    Color::srgb(0.45, 0.3, 0.2),
    Color::srgb(0.95, 0.95, 0.95),
    Color::srgb(0.15, 0.15, 0.18),
    Color::srgb(0.5, 0.8, 0.3),
];
