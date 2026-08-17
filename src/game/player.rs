use bevy::prelude::*;
use rand::seq::SliceRandom;

use super::{
    Alive, Body, CHARACTER_HEIGHT, EmergenciesLeft, GameAssets, GamePhase, Ghost, KillCooldownLeft,
    KillRequest, LocalRole, MatchCleanup, MatchConfig, MatchStats, PlayerLayer, ReportBody, Role,
    SabotageFixContribution, TaskStation, bake_body_tint,
};
use crate::app::{AppState, Paused};
use crate::game::RuntimeMode;
use crate::game::lobby::LobbyState;
use crate::save::SaveData;
use game_utils_bevy::juice::Juice;
use game_utils_bevy::screen_effects::CameraBase;
use game_utils_bevy::transitions::Transition;

#[derive(Component)]
pub struct Player {
    pub id: u64,
    pub name: String,
    #[allow(dead_code, reason = "Reserved for the network protocol")]
    pub color_index: u8,
    pub speed: f32,
}

#[derive(Component)]
pub struct LocalPlayer;

#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct LocalPlayerId(pub Option<u64>);

#[derive(Component, Default, Clone, Copy, Debug)]
pub struct PlayerIntent {
    pub movement: Vec2,
    pub interact: bool,
}

#[derive(Component)]
pub struct AiPlayer {
    pub repath: Timer,
    pub action: Timer,
    pub target_task: Option<Entity>,
    pub dir: Vec2,
}

pub struct PlayerPlugin;
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::InGame),
            spawn_players_from_lobby
                .after(super::setup_match)
                .run_if(super::has_authority),
        )
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
            (local_intent_and_move, camera_follow)
                .chain()
                .in_set(super::GameSimSet::Input)
                .run_if(in_state(AppState::InGame))
                .run_if(|p: Res<Paused>| !p.0)
                .run_if(|t: Res<Transition<AppState>>| !t.block_input)
                .run_if(|ph: Res<GamePhase>| matches!(*ph, GamePhase::Playing)),
        )
        .add_systems(
            Update,
            (ai_brain, ai_ghost_brain, apply_intent_movement)
                .chain()
                .in_set(super::ResolveStep::Ai)
                .run_if(in_state(AppState::InGame))
                .run_if(|p: Res<Paused>| !p.0)
                .run_if(|t: Res<Transition<AppState>>| !t.block_input)
                .run_if(|ph: Res<GamePhase>| matches!(*ph, GamePhase::Playing))
                .run_if(super::has_authority),
        );
    }
}

fn spawn_players_from_lobby(
    mut commands: Commands,
    lobby: Res<LobbyState>,
    cfg: Res<MatchConfig>,
    save: Res<SaveData>,
    mode: Res<RuntimeMode>,
    assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut local_role: ResMut<LocalRole>,
    mut local_player_id: ResMut<LocalPlayerId>,
    mut stats: ResMut<MatchStats>,
) {
    local_player_id.0 = None;
    local_role.0 = None;

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

    stats.players_spawned = slots.len() as u32;
    stats.impostors_spawned = impostor_ids.len() as u32;

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
            local_player_id.0 = Some(slot.id);
        }
        let color = PLAYER_COLORS[slot.color_index as usize % PLAYER_COLORS.len()];
        let pos = start_positions[i % start_positions.len()];

        let body_handle = bake_body_tint(&mut images, &assets.body_for(slot.color_index), color)
            .unwrap_or_else(|| assets.body_for(slot.color_index));
        let clothes_handle = assets.clothes_for(slot.color_index);

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
            PlayerIntent::default(),
            EmergenciesLeft(cfg.emergency_meetings),
            SabotageFixContribution::default(),
            // Invisible root hit/logic anchor (layers are children).
            Sprite {
                color: Color::NONE,
                custom_size: Some(Vec2::splat(1.0)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 10.0),
        ));

        if matches!(role, Role::Impostor) {
            // Start locked like Among Us.
            e.insert(KillCooldownLeft(cfg.kill_cooldown));
        }

        e.with_children(|c| {
            c.spawn((
                PlayerLayer,
                Sprite {
                    image: body_handle,
                    custom_size: Some(Vec2::new(CHARACTER_HEIGHT * 0.75, CHARACTER_HEIGHT)),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, 0.1),
            ));
            c.spawn((
                PlayerLayer,
                Sprite {
                    image: clothes_handle,
                    custom_size: Some(Vec2::new(CHARACTER_HEIGHT * 0.75, CHARACTER_HEIGHT)),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, 0.2),
            ));
            // Name tag above the head.
            c.spawn((
                Text2d::new(slot.name.clone()),
                TextFont::from_font_size(14.0),
                TextColor(Color::srgb(0.95, 0.95, 0.95)),
                Transform::from_xyz(0.0, 22.0, 0.3),
            ));
        });

        let id = e.id();
        if slot.is_local {
            e.insert(LocalPlayer);
        } else if matches!(*mode, RuntimeMode::Local | RuntimeMode::Host) {
            e.insert(AiPlayer {
                repath: Timer::from_seconds(0.4, TimerMode::Repeating),
                action: Timer::from_seconds(0.2, TimerMode::Repeating),
                target_task: None,
                dir: Vec2::ZERO,
            });
        }
        Juice::pop_in(&mut commands, id, 0.35);
    }
}

fn input_direction(keys: &ButtonInput<KeyCode>) -> Vec2 {
    let mut direction = Vec2::ZERO;

    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }

    direction.normalize_or_zero()
}

fn local_intent_and_move(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<GamePhase>,
    mode: Res<RuntimeMode>,
    cfg: Res<MatchConfig>,
    mut living: Query<
        (&Player, &mut PlayerIntent, &mut Transform),
        (With<LocalPlayer>, With<Alive>),
    >,
    mut ghosts: Query<
        (&Player, &mut PlayerIntent, &mut Transform),
        (With<LocalPlayer>, With<Ghost>, Without<Alive>),
    >,
) {
    if !matches!(*phase, GamePhase::Playing) {
        if let Ok((_, mut intent, _)) = living.single_mut() {
            intent.movement = Vec2::ZERO;
            intent.interact = false;
        }
        if let Ok((_, mut intent, _)) = ghosts.single_mut() {
            intent.movement = Vec2::ZERO;
            intent.interact = false;
        }
        return;
    }

    let dir = input_direction(&keys);

    if let Ok((player, mut intent, mut transform)) = living.single_mut() {
        intent.movement = dir;
        intent.interact = keys.pressed(KeyCode::KeyE);
        if !matches!(*mode, RuntimeMode::Client) {
            transform.translation +=
                (intent.movement * player.speed * time.delta_secs()).extend(0.0);
            clamp_pos(&mut transform);
        }
        return;
    }

    if let Ok((player, mut intent, mut transform)) = ghosts.single_mut() {
        intent.movement = dir;
        intent.interact = keys.pressed(KeyCode::KeyE);
        if matches!(*mode, RuntimeMode::Client) {
            return;
        }
        let speed = player.speed * cfg.ghost_speed_mul;
        transform.translation += (intent.movement * speed * time.delta_secs()).extend(0.0);
        clamp_pos(&mut transform);
    }
}

fn clamp_pos(tf: &mut Transform) {
    tf.translation.x = tf
        .translation
        .x
        .clamp(-super::MAP_BOUNDS.x, super::MAP_BOUNDS.x);
    tf.translation.y = tf
        .translation
        .y
        .clamp(-super::MAP_BOUNDS.y, super::MAP_BOUNDS.y);
}

fn apply_intent_movement(
    time: Res<Time>,
    mode: Res<RuntimeMode>,
    cfg: Res<MatchConfig>,
    mut living: Query<
        (&Player, &PlayerIntent, &mut Transform),
        (With<Alive>, Without<LocalPlayer>),
    >,
    mut ghosts: Query<
        (&Player, &PlayerIntent, &mut Transform),
        (With<Ghost>, Without<Alive>, Without<LocalPlayer>),
    >,
) {
    if matches!(*mode, RuntimeMode::Client) {
        return;
    }
    for (player, intent, mut tf) in &mut living {
        tf.translation += (intent.movement * player.speed * time.delta_secs()).extend(0.0);
        clamp_pos(&mut tf);
    }
    for (player, intent, mut tf) in &mut ghosts {
        let speed = player.speed * cfg.ghost_speed_mul;
        tf.translation += (intent.movement * speed * time.delta_secs()).extend(0.0);
        clamp_pos(&mut tf);
    }
}

fn ai_brain(
    time: Res<Time>,
    cfg: Res<MatchConfig>,
    phase: Res<GamePhase>,
    mut kill_tx: MessageWriter<KillRequest>,
    mut report_tx: MessageWriter<ReportBody>,
    tasks: Query<(Entity, &Transform, &TaskStation)>,
    bodies: Query<(&Transform, &Body)>,
    mut ais: Query<
        (
            Entity,
            &Player,
            &Role,
            &mut AiPlayer,
            &mut PlayerIntent,
            &Transform,
            Option<&mut KillCooldownLeft>,
        ),
        With<Alive>,
    >,
) {
    if !matches!(*phase, GamePhase::Playing) {
        for (_, _, _, _, mut intent, _, _) in &mut ais {
            intent.movement = Vec2::ZERO;
            intent.interact = false;
        }
        return;
    }

    for (_e, player, role, mut ai, mut intent, tf, kill_cd) in &mut ais {
        ai.repath.tick(time.delta());
        ai.action.tick(time.delta());
        let pos = tf.translation.truncate();

        // 1) Report a nearby body.
        let near_body = bodies
            .iter()
            .filter(|(_, b)| !b.reported)
            .any(|(bt, _)| pos.distance(bt.translation.truncate()) <= cfg.bot_report_range);
        if near_body {
            report_tx.write(ReportBody {
                reporter_id: player.id,
            });
            intent.movement = Vec2::ZERO;
            intent.interact = false;
            continue;
        }

        // 2) Impostor kill when cooldown is ready.
        if matches!(role, Role::Impostor) {
            let cd_ok = kill_cd.as_ref().map(|c| c.0 <= 0.0).unwrap_or(false);
            let p = cfg.bot_kill_aggression * time.delta_secs() * 2.0;
            if cd_ok && rand::random::<f32>() < p {
                kill_tx.write(KillRequest {
                    actor_id: player.id,
                });
            }
        }

        // 3) Walk to a targeted task (crewmates reliably, impostors fake).
        if ai.repath.just_finished() || ai.target_task.is_none() {
            let mut best: Option<(Entity, f32)> = None;
            for (te, tt, st) in &tasks {
                if st.done {
                    continue;
                }
                if matches!(role, Role::Impostor) && rand::random::<f32>() > 0.15 {
                    continue;
                }
                let d = pos.distance(tt.translation.truncate());
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((te, d));
                }
            }
            ai.target_task = best.map(|(e, _)| e);
            if ai.target_task.is_none() {
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                ai.dir = Vec2::new(angle.cos(), angle.sin());
            }
        }

        if let Some(tid) = ai.target_task {
            if let Ok((_, tt, st)) = tasks.get(tid) {
                if st.done {
                    ai.target_task = None;
                    intent.interact = false;
                    intent.movement = ai.dir;
                    continue;
                }
                let delta = tt.translation.truncate() - pos;
                let dist = delta.length();
                if dist <= cfg.interact_range * 0.85 {
                    intent.movement = Vec2::ZERO;
                    intent.interact = matches!(role, Role::Crewmate);
                } else {
                    intent.movement = delta.normalize_or_zero();
                    intent.interact = false;
                }
            } else {
                ai.target_task = None;
            }
        } else {
            intent.movement = ai.dir;
            intent.interact = false;
        }
    }
}

/// Dead crew AI still push the task bar (Among Us).
fn ai_ghost_brain(
    time: Res<Time>,
    cfg: Res<MatchConfig>,
    phase: Res<GamePhase>,
    tasks: Query<(Entity, &Transform, &TaskStation)>,
    mut ais: Query<
        (
            &Role,
            &mut AiPlayer,
            &mut PlayerIntent,
            &Transform,
        ),
        (With<Ghost>, With<AiPlayer>, Without<Alive>),
    >,
) {
    if !matches!(*phase, GamePhase::Playing) {
        for (_, _, mut intent, _) in &mut ais {
            intent.movement = Vec2::ZERO;
            intent.interact = false;
        }
        return;
    }

    for (role, mut ai, mut intent, tf) in &mut ais {
        if !matches!(role, Role::Crewmate) {
            intent.movement = Vec2::ZERO;
            intent.interact = false;
            continue;
        }

        ai.repath.tick(time.delta());
        let pos = tf.translation.truncate();

        if ai.repath.just_finished() || ai.target_task.is_none() {
            let mut best: Option<(Entity, f32)> = None;
            for (te, tt, st) in &tasks {
                if st.done {
                    continue;
                }
                let d = pos.distance(tt.translation.truncate());
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((te, d));
                }
            }
            ai.target_task = best.map(|(e, _)| e);
            if ai.target_task.is_none() {
                let angle = rand::random::<f32>() * std::f32::consts::TAU;
                ai.dir = Vec2::new(angle.cos(), angle.sin());
            }
        }

        if let Some(tid) = ai.target_task {
            if let Ok((_, tt, st)) = tasks.get(tid) {
                if st.done {
                    ai.target_task = None;
                    intent.interact = false;
                    intent.movement = ai.dir;
                    continue;
                }
                let delta = tt.translation.truncate() - pos;
                let dist = delta.length();
                if dist <= cfg.interact_range * 0.85 {
                    intent.movement = Vec2::ZERO;
                    intent.interact = true;
                } else {
                    intent.movement = delta.normalize_or_zero();
                    intent.interact = false;
                }
            } else {
                ai.target_task = None;
                intent.movement = ai.dir;
                intent.interact = false;
            }
        } else {
            intent.movement = ai.dir;
            intent.interact = false;
        }
    }
}

fn camera_follow(
    time: Res<Time>,
    config: Res<MatchConfig>,
    mut camera: Query<&mut CameraBase, With<Camera2d>>,
    local: Query<&Transform, (With<LocalPlayer>, Without<Camera2d>)>,
) {
    let Ok(mut base) = camera.single_mut() else {
        return;
    };
    let Ok(player) = local.single() else {
        return;
    };

    let target = Vec3::new(
        player.translation.x.clamp(-320.0, 320.0),
        player.translation.y.clamp(-160.0, 160.0),
        1000.0,
    );

    let alpha = 1.0 - (-config.camera_follow_sharpness * time.delta_secs()).exp();

    base.translation = base.translation.lerp(target, alpha);
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
