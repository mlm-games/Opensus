use bevy::prelude::*;
use rand::seq::{IndexedRandom, SliceRandom};

use super::{
    ActiveSabotage, Alive, Body, CHARACTER_HEIGHT, EmergenciesLeft, EmergencyButton,
    EmergencyCooldownLeft, GameAssets, GamePhase, Ghost, KillCooldownLeft, KillRequest, LocalRole,
    MAP_BOUNDS, MatchCleanup, MatchConfig, MatchStats, PlayerLayer, ReportBody, Role,
    SabotageFixContribution, SabotageFixStation, TaskStation, assign_tasks, bake_body_tint,
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

#[derive(Resource, Default, Clone)]
pub struct LocalPrompt(pub String);

#[derive(Component, Default, Clone, Copy, Debug)]
pub struct PlayerIntent {
    pub movement: Vec2,
    pub interact: bool,
}

#[derive(Component, Default)]
pub struct PlayerAnimation {
    phase: f32,
    blend: f32,
}

#[derive(Component, Clone, Copy)]
struct PlayerLayerRest {
    translation: Vec3,
    scale: Vec3,
}

#[derive(Component)]
pub struct AiPlayer {
    pub repath: Timer,
    pub action: Timer,
    pub target_task: Option<Entity>,
    pub dir: Vec2,
    pub reported_this_body: bool,
}

/// A human-controlled remote client represented in the authoritative host world.
///
/// Its movement is applied from fixed-step network commands, not from the
/// normal latest-intent movement system.
#[derive(Component, Default)]
pub struct RemoteNetworkPlayer;

pub struct PlayerPlugin;
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LocalPrompt>()
            .add_systems(
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
                (local_intent_and_move, update_local_prompt, camera_follow)
                    .chain()
                    .in_set(super::GameSimSet::Input)
                    .run_if(in_state(AppState::InGame))
                    .run_if(|p: Res<Paused>| !p.0)
                    .run_if(|t: Res<Transition<AppState>>| !t.block_input)
                    .run_if(|ph: Res<GamePhase>| matches!(*ph, GamePhase::Playing)),
            )
            .add_systems(
                Update,
                (
                    ai_brain,
                    ai_ghost_brain,
                    apply_intent_movement,
                    capture_player_layer_rest,
                    face_movement,
                    animate_player_layers,
                )
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
    assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    mut local_role: ResMut<LocalRole>,
    mut local_player_id: ResMut<LocalPlayerId>,
    mut stats: ResMut<MatchStats>,
    mut task_board: ResMut<super::TaskBoard>,
    mode: Res<RuntimeMode>,
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
            is_bot: false,
        });
        for i in 0..cfg.bot_count as u64 {
            slots.push(super::lobby::LobbySlot {
                id: 10 + i,
                name: format!("Agent-{}", i + 2),
                color_index: ((save.preferred_color_index as u64 + 1 + i)
                    % PLAYER_COLORS.len() as u64) as u8,
                ready: true,
                is_local: false,
                is_host: false,
                is_bot: true,
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

    let crewmate_count = slots
        .iter()
        .filter(|slot| !impostor_ids.contains(&slot.id))
        .count() as u32;
    task_board.completed = 0;
    task_board.total = crewmate_count.saturating_mul(cfg.tasks_per_crewmate as u32);

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
        let pos = super::PLAYER_SPAWNS[i % super::PLAYER_SPAWNS.len()];

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
            PlayerAnimation::default(),
            EmergenciesLeft(cfg.emergency_meetings),
            EmergencyCooldownLeft(cfg.emergency_cooldown),
            SabotageFixContribution::default(),
            Sprite {
                color: Color::NONE,
                custom_size: Some(Vec2::splat(1.0)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 10.0),
        ));

        if matches!(role, Role::Impostor) {
            e.insert(KillCooldownLeft(cfg.kill_cooldown));
        }

        if matches!(role, Role::Crewmate) {
            e.insert(assign_tasks(slot.id, cfg.tasks_per_crewmate as usize));
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
        } else if slot.is_bot {
            e.insert(AiPlayer {
                repath: Timer::from_seconds(0.4, TimerMode::Repeating),
                action: Timer::from_seconds(0.2, TimerMode::Repeating),
                target_task: None,
                dir: Vec2::ZERO,
                reported_this_body: false,
            });
        } else if matches!(*mode, RuntimeMode::Host) {
            e.insert(RemoteNetworkPlayer);
        }
        Juice::pop_in(&mut commands, id, 0.35);
    }
}

pub(crate) fn input_direction(keys: &ButtonInput<KeyCode>) -> Vec2 {
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
    solids: Query<(&Transform, &super::SolidAabb), Without<Player>>,
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

    let direction = input_direction(&keys);
    let interact = keys.pressed(KeyCode::KeyE);

    if matches!(*mode, RuntimeMode::Client) {
        if let Ok((_, mut intent, _)) = living.single_mut() {
            intent.movement = direction;
            intent.interact = interact;
        } else if let Ok((_, mut intent, _)) = ghosts.single_mut() {
            intent.movement = direction;
            intent.interact = interact;
        }

        return;
    }

    let boxes = super::collision::solid_boxes(&solids);

    if let Ok((player, mut intent, mut transform)) = living.single_mut() {
        intent.movement = direction;
        intent.interact = interact;

        let position = super::collision::step_player_position(
            transform.translation.truncate(),
            direction,
            player.speed,
            time.delta_secs(),
            true,
            &boxes,
        );

        transform.translation.x = position.x;
        transform.translation.y = position.y;
        return;
    }

    if let Ok((player, mut intent, mut transform)) = ghosts.single_mut() {
        intent.movement = direction;
        intent.interact = interact;

        let position = super::collision::step_player_position(
            transform.translation.truncate(),
            direction,
            player.speed * cfg.ghost_speed_mul,
            time.delta_secs(),
            false,
            &boxes,
        );

        transform.translation.x = position.x;
        transform.translation.y = position.y;
    }
}

fn apply_intent_movement(
    time: Res<Time>,
    mode: Res<RuntimeMode>,
    cfg: Res<MatchConfig>,
    solids: Query<(&Transform, &super::SolidAabb), Without<Player>>,
    mut living: Query<
        (&Player, &PlayerIntent, &mut Transform),
        (
            With<Alive>,
            Without<LocalPlayer>,
            Without<RemoteNetworkPlayer>,
        ),
    >,
    mut ghosts: Query<
        (&Player, &PlayerIntent, &mut Transform),
        (
            With<Ghost>,
            Without<Alive>,
            Without<LocalPlayer>,
            Without<RemoteNetworkPlayer>,
        ),
    >,
) {
    if matches!(*mode, RuntimeMode::Client) {
        return;
    }

    let boxes = super::collision::solid_boxes(&solids);

    for (player, intent, mut transform) in &mut living {
        let position = super::collision::step_player_position(
            transform.translation.truncate(),
            intent.movement,
            player.speed,
            time.delta_secs(),
            true,
            &boxes,
        );

        transform.translation.x = position.x;
        transform.translation.y = position.y;
    }

    for (player, intent, mut transform) in &mut ghosts {
        let position = super::collision::step_player_position(
            transform.translation.truncate(),
            intent.movement,
            player.speed * cfg.ghost_speed_mul,
            time.delta_secs(),
            false,
            &boxes,
        );

        transform.translation.x = position.x;
        transform.translation.y = position.y;
    }
}

fn ai_brain(
    time: Res<Time>,
    cfg: Res<MatchConfig>,
    phase: Res<GamePhase>,
    mut kill_tx: MessageWriter<KillRequest>,
    mut report_tx: MessageWriter<ReportBody>,
    mut sabo_tx: MessageWriter<super::SabotageAction>,
    sabotage: Res<super::ActiveSabotage>,
    cooldown: Res<super::SabotageCooldown>,
    tasks: Query<(Entity, &Transform, &TaskStation)>,
    bodies: Query<(&Transform, &Body)>,
    fix_stations: Query<(&super::SabotageFixStation, &Transform), Without<TaskStation>>,
    solids: Query<(&Transform, &super::SolidAabb)>,
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

    let solid_boxes: Vec<(Vec2, Vec2)> = solids
        .iter()
        .map(|(t, s)| (t.translation.truncate(), s.half_extents))
        .collect();

    for (_e, player, role, mut ai, mut intent, tf, kill_cd) in &mut ais {
        ai.repath.tick(time.delta());
        ai.action.tick(time.delta());
        let pos = tf.translation.truncate();

        let near_body = bodies
            .iter()
            .filter(|(_, b)| !b.reported)
            .any(|(bt, _)| pos.distance(bt.translation.truncate()) <= cfg.bot_report_range);
        if near_body {
            if !ai.reported_this_body {
                report_tx.write(ReportBody {
                    reporter_id: player.id,
                });
                ai.reported_this_body = true;
            }
            intent.movement = Vec2::ZERO;
            intent.interact = false;
            continue;
        }
        ai.reported_this_body = false;

        if matches!(role, Role::Impostor) {
            let cd_ok = kill_cd.as_ref().map(|c| c.0 <= 0.0).unwrap_or(false);
            let p = cfg.bot_kill_aggression * time.delta_secs() * 2.0;
            if cd_ok && rand::random::<f32>() < p {
                kill_tx.write(KillRequest {
                    actor_id: player.id,
                });
            }
        }

        if matches!(role, Role::Impostor)
            && !sabotage.is_active()
            && cooldown.remaining <= 0.0
            && ai.action.just_finished()
            && rand::random::<f32>() < 0.08
        {
            let kind = *[
                super::SabotageKind::Lights,
                super::SabotageKind::Oxygen,
                super::SabotageKind::Reactor,
            ]
            .choose(&mut rand::rng())
            .unwrap();
            sabo_tx.write(super::SabotageAction {
                actor_id: player.id,
                kind,
            });
        }

        if matches!(role, Role::Crewmate)
            && let Some(kind) = sabotage.kind
        {
            let mut best: Option<(f32, Vec2)> = None;
            for (st, tt) in &fix_stations {
                if st.kind != kind || st.progress >= 1.0 {
                    continue;
                }
                let d = pos.distance(tt.translation.truncate());
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, tt.translation.truncate()));
                }
            }
            if let Some((dist, target)) = best {
                if dist <= cfg.interact_range * 0.85 {
                    intent.movement = Vec2::ZERO;
                    intent.interact = true;
                } else {
                    let wp = crate::game::navigation::next_waypoint(pos, target, &solid_boxes);
                    intent.movement = (wp - pos).normalize_or_zero();
                    intent.interact = false;
                }
                continue;
            }
        }

        if ai.repath.just_finished() || ai.target_task.is_none() {
            let mut best: Option<(Entity, f32)> = None;
            for (te, tt, st) in &tasks {
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
            if let Ok((_, tt, _)) = tasks.get(tid) {
                let target = tt.translation.truncate();
                let dist = pos.distance(target);
                if dist <= cfg.interact_range * 0.85 {
                    intent.movement = Vec2::ZERO;
                    intent.interact = matches!(role, Role::Crewmate);
                } else {
                    let wp = crate::game::navigation::next_waypoint(pos, target, &solid_boxes);
                    intent.movement = (wp - pos).normalize_or_zero();
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
        (&Role, &mut AiPlayer, &mut PlayerIntent, &Transform),
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
            for (te, tt, _st) in &tasks {
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
            if let Ok((_, tt, _st)) = tasks.get(tid) {
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

fn update_local_prompt(
    cfg: Res<MatchConfig>,
    phase: Res<GamePhase>,
    sabotage: Res<ActiveSabotage>,
    local: Query<(&Transform, Option<&Alive>), With<LocalPlayer>>,
    bodies: Query<(&Transform, &Body)>,
    tasks: Query<(&Transform, &TaskStation)>,
    buttons: Query<&Transform, (With<EmergencyButton>, Without<Player>)>,
    fix: Query<(&Transform, &SabotageFixStation)>,
    mut prompt: ResMut<LocalPrompt>,
) {
    prompt.0.clear();
    if !matches!(*phase, GamePhase::Playing) {
        return;
    }
    let Ok((tf, alive)) = local.single() else {
        return;
    };
    let pos = tf.translation.truncate();

    if alive.is_some() {
        if bodies.iter().any(|(bt, b)| {
            !b.reported && pos.distance(bt.translation.truncate()) <= cfg.report_range
        }) {
            prompt.0 = "R - Report body".into();
            return;
        }
        if let Some(kind) = sabotage.kind
            && fix.iter().any(|(ft, s)| {
                s.kind == kind
                    && s.progress < 1.0
                    && pos.distance(ft.translation.truncate()) <= cfg.interact_range
            })
        {
            prompt.0 = "E - Hold to fix".into();
            return;
        }
        if buttons
            .iter()
            .any(|bt| pos.distance(bt.translation.truncate()) <= cfg.interact_range)
        {
            prompt.0 = "F - Emergency meeting".into();
            return;
        }
    }
    if tasks
        .iter()
        .any(|(tt, _)| pos.distance(tt.translation.truncate()) <= cfg.interact_range)
    {
        prompt.0 = "E - Hold to work".into();
    }
}

fn capture_player_layer_rest(
    mut commands: Commands,
    layers: Query<(Entity, &Transform), Added<PlayerLayer>>,
) {
    for (entity, transform) in &layers {
        commands.entity(entity).insert(PlayerLayerRest {
            translation: transform.translation,
            scale: transform.scale,
        });
    }
}

fn face_movement(
    q: Query<(&PlayerIntent, &Children), Or<(With<Alive>, With<Ghost>)>>,
    mut layers: Query<&mut Transform, With<PlayerLayer>>,
) {
    for (intent, children) in &q {
        if intent.movement.x.abs() < 0.01 {
            continue;
        }
        let sx = if intent.movement.x >= 0.0 { 1.0 } else { -1.0 };
        for child in children.iter() {
            if let Ok(mut tf) = layers.get_mut(child) {
                tf.scale.x = sx * tf.scale.x.abs();
            }
        }
    }
}

fn animate_player_layers(
    time: Res<Time>,
    mut players: Query<(&PlayerIntent, &mut PlayerAnimation, &Children)>,
    mut layers: Query<(&mut Transform, &PlayerLayerRest), With<PlayerLayer>>,
) {
    for (intent, mut animation, children) in &mut players {
        let moving = intent.movement.length().clamp(0.0, 1.0);
        let blend_target = moving;
        let blend_alpha = 1.0 - (-14.0 * time.delta_secs()).exp();

        animation.blend += (blend_target - animation.blend) * blend_alpha;
        animation.phase += time.delta_secs() * (8.5 + moving * 2.5) * moving;

        let step = animation.phase.sin();
        let bob = step.abs() * 2.4 * animation.blend;
        let lean = step * 0.025 * animation.blend;
        let squash = step.abs() * 0.035 * animation.blend;

        for child in children.iter() {
            let Ok((mut transform, rest)) = layers.get_mut(child) else {
                continue;
            };

            let facing = transform.scale.x.signum();

            transform.translation = rest.translation + Vec3::new(0.0, bob, 0.0);
            transform.rotation = Quat::from_rotation_z(lean);
            transform.scale.x = rest.scale.x.abs() * facing * (1.0 - squash * 0.35);
            transform.scale.y = rest.scale.y * (1.0 + squash);
        }
    }
}

const CAMERA_FOLLOW_RATE: f32 = 9.0;
const CAMERA_LOOK_AHEAD: f32 = 72.0;

pub fn camera_follow(
    time: Res<Time>,
    player: Query<(&Transform, &PlayerIntent), With<LocalPlayer>>,
    mut camera: Query<&mut CameraBase, With<Camera2d>>,
) {
    let Ok((player_transform, intent)) = player.single() else {
        return;
    };
    let Ok(mut camera) = camera.single_mut() else {
        return;
    };

    let player_position = player_transform.translation.truncate();
    let look_ahead = intent.movement.normalize_or_zero() * CAMERA_LOOK_AHEAD;

    let desired = Vec2::new(
        (player_position.x + look_ahead.x).clamp(-MAP_BOUNDS.x, MAP_BOUNDS.x),
        (player_position.y + look_ahead.y).clamp(-MAP_BOUNDS.y, MAP_BOUNDS.y),
    );

    let current = camera.translation.truncate();
    let blend = 1.0 - (-CAMERA_FOLLOW_RATE * time.delta_secs()).exp();
    let next = current.lerp(desired, blend);

    camera.translation.x = next.x;
    camera.translation.y = next.y;
    camera.translation.z = 1000.0;
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
