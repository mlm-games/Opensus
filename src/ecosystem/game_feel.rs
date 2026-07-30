use bevy::input::gamepad::{Gamepad, GamepadRumbleIntensity, GamepadRumbleRequest};
use bevy::prelude::*;

#[derive(Component)]
pub struct Recoil {
    pub offset: Vec2,
    pub timer: Timer,
    pub original: Option<Vec3>,
}

#[derive(Resource)]
pub struct SlowMotion {
    pub active: bool,
    pub scale: f32,
    pub timer: Timer,
}

impl Default for SlowMotion {
    fn default() -> Self {
        Self {
            active: false,
            scale: 1.0,
            timer: Timer::from_seconds(0.0, TimerMode::Once),
        }
    }
}

pub struct GameFeel;

impl GameFeel {
    pub fn add_recoil(commands: &mut Commands, entity: Entity, dir: Vec2, strength: f32) {
        commands.entity(entity).insert(Recoil {
            offset: dir.normalize_or_zero() * strength,
            timer: Timer::from_seconds(0.2, TimerMode::Once),
            original: None,
        });
    }

    pub fn apply_knockback(velocity: &mut Vec2, dir: Vec2, force: f32) {
        *velocity = dir.normalize_or_zero() * force;
    }

    pub fn slow_motion(slow_mo: &mut SlowMotion, scale: f32, duration_real: f32) {
        slow_mo.scale = scale.clamp(0.01, 1.0);
        slow_mo.timer = Timer::from_seconds(duration_real, TimerMode::Once);
        slow_mo.active = true;
    }

    pub fn rumble_controller(
        rumble_writer: &mut MessageWriter<GamepadRumbleRequest>,
        gamepads: &Query<(Entity, &Gamepad)>,
        weak: f32,
        strong: f32,
        duration_secs: f32,
    ) {
        let intensity = GamepadRumbleIntensity {
            strong_motor: strong.clamp(0.0, 1.0),
            weak_motor: weak.clamp(0.0, 1.0),
        };
        let duration = std::time::Duration::from_secs_f32(duration_secs.max(0.0));
        for (entity, _) in gamepads.iter() {
            rumble_writer.write(GamepadRumbleRequest::Add {
                gamepad: entity,
                intensity,
                duration,
            });
        }
    }
}

pub struct GameFeelPlugin;
impl Plugin for GameFeelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SlowMotion>()
            .add_systems(Update, (apply_recoil, tick_slow_motion));
    }
}

fn apply_recoil(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Transform, &mut Recoil)>,
) {
    for (e, mut tf, mut recoil) in &mut q {
        if recoil.original.is_none() {
            recoil.original = Some(tf.translation);
            tf.translation += recoil.offset.extend(0.0);
        }
        recoil.timer.tick(time.delta());
        let t = recoil.timer.fraction();
        let ease = 1.0 - (1.0 - t).powi(4);
        if let Some(orig) = recoil.original {
            tf.translation = orig + recoil.offset.extend(0.0) * (1.0 - ease);
        }
        if recoil.timer.just_finished() {
            if let Some(orig) = recoil.original {
                tf.translation = orig;
            }
            commands.entity(e).remove::<Recoil>();
        }
    }
}

fn tick_slow_motion(
    real: Res<Time<Real>>,
    mut slow: ResMut<SlowMotion>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    if !slow.active {
        return;
    }
    virtual_time.set_relative_speed(slow.scale);
    slow.timer.tick(real.delta());
    if slow.timer.just_finished() {
        slow.active = false;
        virtual_time.set_relative_speed(1.0);
    }
}
