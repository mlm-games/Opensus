use bevy::input::gamepad::{Gamepad, GamepadRumbleRequest};
use bevy::prelude::*;
use rand::RngExt;

use crate::app::{AppState, Paused};
use crate::ecosystem::game_feel::GameFeel;
use crate::ecosystem::juice::Juice;
use crate::ecosystem::save::{SaveData, SaveManager};
use crate::ecosystem::screen_effects::{
    ChromaticAberration, FlashWhite, FreezeFrame, ScreenEffects, Trauma,
};
use crate::ecosystem::transitions::Transition;
use crate::ecosystem::vfx::VfxSpawner;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PowerupKind {
    Chromatic,
    Flash,
    Freeze,
    Trauma,
    DamageAll,
    CircleWipe,
}

#[derive(Component)]
struct PowerupDrop {
    kind: PowerupKind,
    speed: f32,
}

#[derive(Resource, Default)]
pub struct Score(pub u32);

#[derive(Component)]
struct Player {
    speed: f32,
    cooldown: Timer,
}

#[derive(Component)]
struct Bullet {
    vel: Vec2,
    life: Timer,
}

#[derive(Component)]
struct Enemy {
    speed: f32,
}

#[derive(Component)]
struct DemoCleanup;

pub struct DemoPlugin;
impl Plugin for DemoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Score>()
            .add_systems(OnEnter(AppState::InGame), setup_demo)
            .add_systems(OnExit(AppState::InGame), cleanup_demo)
            .add_systems(
                Update,
                (
                    player_move,
                    player_shoot,
                    move_bullets,
                    spawn_enemies,
                    move_enemies,
                    bullet_enemy_collision,
                    move_powerups,
                    collect_powerups,
                )
                    .run_if(in_state(AppState::InGame))
                    .run_if(|p: Res<Paused>| !p.0)
                    .run_if(|t: Res<Transition>| !t.block_input),
            );
    }
}

fn setup_demo(mut commands: Commands, mut score: ResMut<Score>) {
    score.0 = 0;
    let player = commands
        .spawn((
            DemoCleanup,
            Player {
                speed: 320.0,
                cooldown: Timer::from_seconds(0.18, TimerMode::Repeating),
            },
            Sprite {
                color: Color::srgb(0.3, 0.75, 1.0),
                custom_size: Some(Vec2::splat(28.0)),
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, 10.0),
        ))
        .id();
    Juice::pop_in(&mut commands, player, 0.35);
}

fn cleanup_demo(mut commands: Commands, q: Query<Entity, With<DemoCleanup>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn player_move(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut q: Query<(&Player, &mut Transform)>,
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
        tf.translation.x = tf.translation.x.clamp(-600.0, 600.0);
        tf.translation.y = tf.translation.y.clamp(-320.0, 320.0);
    }
}

fn player_shoot(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Player, &Transform)>,
) {
    let Ok((e, mut p, tf)) = q.single_mut() else {
        return;
    };
    p.cooldown.tick(time.delta());
    let fire = mouse.pressed(MouseButton::Left) || keys.pressed(KeyCode::Space);
    if fire && p.cooldown.just_finished() {
        GameFeel::add_recoil(&mut commands, e, Vec2::NEG_Y, 6.0);
        commands.spawn((
            DemoCleanup,
            Bullet {
                vel: Vec2::Y * 520.0,
                life: Timer::from_seconds(1.2, TimerMode::Once),
            },
            Sprite {
                color: Color::srgb(1.0, 0.9, 0.3),
                custom_size: Some(Vec2::new(6.0, 14.0)),
                ..default()
            },
            Transform::from_translation(tf.translation + Vec3::Y * 20.0),
        ));
    }
}

fn move_bullets(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Bullet, &mut Transform)>,
) {
    for (e, mut b, mut tf) in &mut q {
        b.life.tick(time.delta());
        tf.translation += (b.vel * time.delta_secs()).extend(0.0);
        if b.life.just_finished() {
            commands.entity(e).despawn();
        }
    }
}

fn spawn_enemies(mut commands: Commands, time: Res<Time>, mut timer: Local<f32>) {
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = 0.8;
    let mut rng = rand::rng();
    let x = rng.random_range(-550.0..550.0);
    let e = commands
        .spawn((
            DemoCleanup,
            Enemy {
                speed: rng.random_range(60.0..140.0),
            },
            Sprite {
                color: Color::srgb(1.0, 0.35, 0.35),
                custom_size: Some(Vec2::splat(24.0)),
                ..default()
            },
            Transform::from_xyz(x, 360.0, 5.0),
        ))
        .id();
    Juice::pop_in(&mut commands, e, 0.25);
}

fn move_enemies(time: Res<Time>, mut q: Query<(&Enemy, &mut Transform)>) {
    for (en, mut tf) in &mut q {
        tf.translation.y -= en.speed * time.delta_secs();
    }
}

fn spawn_powerup(commands: &mut Commands, pos: Vec2) {
    const KINDS: [PowerupKind; 6] = [
        PowerupKind::Chromatic,
        PowerupKind::Flash,
        PowerupKind::Freeze,
        PowerupKind::Trauma,
        PowerupKind::DamageAll,
        PowerupKind::CircleWipe,
    ];
    let kind = KINDS[rand::rng().random_range(0..KINDS.len())];
    let color = match kind {
        PowerupKind::Chromatic => Color::srgb(0.2, 0.6, 1.0),
        PowerupKind::Flash => Color::srgb(1.0, 1.0, 0.8),
        PowerupKind::Freeze => Color::srgb(0.5, 0.9, 1.0),
        PowerupKind::Trauma => Color::srgb(1.0, 0.5, 0.0),
        PowerupKind::DamageAll => Color::srgb(1.0, 0.1, 0.1),
        PowerupKind::CircleWipe => Color::srgb(0.8, 0.3, 1.0),
    };
    let e = commands
        .spawn((
            DemoCleanup,
            PowerupDrop {
                kind,
                speed: rand::rng().random_range(30.0..60.0),
            },
            Sprite {
                color,
                custom_size: Some(Vec2::splat(18.0)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 3.0),
        ))
        .id();
    Juice::pop_in(commands, e, 0.2);
}

fn bullet_enemy_collision(
    mut commands: Commands,
    mut score: ResMut<Score>,
    mut trauma: ResMut<Trauma>,
    mut save: ResMut<SaveData>,
    bullets: Query<(Entity, &Transform), With<Bullet>>,
    enemies: Query<(Entity, &Transform), With<Enemy>>,
    gamepads: Query<(Entity, &Gamepad)>,
    mut rumble: MessageWriter<GamepadRumbleRequest>,
) {
    for (be, bt) in &bullets {
        for (ee, et) in &enemies {
            if bt
                .translation
                .truncate()
                .distance(et.translation.truncate())
                < 18.0
            {
                let pos = et.translation.truncate();
                commands.entity(be).despawn();
                commands.entity(ee).despawn();
                score.0 += 10;
                ScreenEffects::add_trauma(&mut trauma, 0.35);
                GameFeel::rumble_controller(&mut rumble, &gamepads, 0.3, 0.7, 0.15);
                VfxSpawner::spawn_damage_number(&mut commands, 10, pos, Color::srgb(1.0, 0.9, 0.2));
                VfxSpawner::spawn_burst(
                    &mut commands,
                    pos,
                    8,
                    Color::srgb(1.0, 0.4, 0.3),
                    (40.0, 100.0),
                );
                if rand::rng().random_range(0.0..1.0) < 0.35 {
                    spawn_powerup(&mut commands, pos);
                }
                if score.0 > save.high_score {
                    save.high_score = score.0;
                    let _ = SaveManager::save(&save);
                }
            }
        }
    }
}

fn move_powerups(time: Res<Time>, mut q: Query<(&PowerupDrop, &mut Transform)>) {
    for (p, mut tf) in &mut q {
        tf.translation.y -= p.speed * time.delta_secs();
        if tf.translation.y < -380.0 {
            tf.translation.y = 380.0;
            tf.translation.x = rand::rng().random_range(-550.0..550.0);
        }
    }
}

fn collect_powerups(
    mut commands: Commands,
    player: Query<&Transform, With<Player>>,
    powerups: Query<(Entity, &Transform, &PowerupDrop)>,
    mut trauma: ResMut<Trauma>,
    mut flash: ResMut<FlashWhite>,
    mut freeze: ResMut<FreezeFrame>,
    mut chroma: ResMut<ChromaticAberration>,
    mut transition: ResMut<Transition>,
) {
    let Ok(pt) = player.single() else {
        return;
    };
    let ppos = pt.translation.truncate();
    for (e, t, drop) in &powerups {
        if ppos.distance(t.translation.truncate()) > 28.0 {
            continue;
        }
        commands.entity(e).despawn();
        match drop.kind {
            PowerupKind::Chromatic => {
                ScreenEffects::chromatic_pulse(&mut chroma, 0.8);
            }
            PowerupKind::Flash => {
                ScreenEffects::flash_white(&mut flash, 0.4);
            }
            PowerupKind::Freeze => {
                ScreenEffects::freeze_frame(&mut freeze, 0.15);
            }
            PowerupKind::Trauma => {
                ScreenEffects::add_trauma(&mut trauma, 0.6);
            }
            PowerupKind::DamageAll => {
                ScreenEffects::add_trauma(&mut trauma, 0.8);
                ScreenEffects::flash_white(&mut flash, 0.3);
            }
            PowerupKind::CircleWipe => {
                transition.circle_progress = 1.0;
            }
        }
    }
}
