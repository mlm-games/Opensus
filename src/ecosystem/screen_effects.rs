use bevy::prelude::*;
use rand::RngExt;

#[derive(Resource, Default)]
pub struct Trauma(pub f32);

#[derive(Resource)]
pub struct FlashWhite {
    pub amount: f32,
    pub timer: Timer,
}

impl Default for FlashWhite {
    fn default() -> Self {
        Self {
            amount: 0.0,
            timer: Timer::from_seconds(0.0, TimerMode::Once),
        }
    }
}

#[derive(Resource)]
pub struct FreezeFrame {
    pub active: bool,
    pub timer: Timer,
}

impl Default for FreezeFrame {
    fn default() -> Self {
        Self {
            active: false,
            timer: Timer::from_seconds(0.0, TimerMode::Once),
        }
    }
}

#[derive(Resource, Default)]
pub struct ChromaticAberration(pub f32);

#[derive(Component, Clone, Copy)]
pub struct CameraBase {
    pub translation: Vec3,
    pub rotation: f32,
}

pub struct ScreenEffects;

impl ScreenEffects {
    pub fn add_trauma(trauma: &mut Trauma, amount: f32) {
        trauma.0 = (trauma.0 + amount).clamp(0.0, 1.0);
    }

    pub fn freeze_frame(freeze: &mut FreezeFrame, duration: f32) {
        freeze.timer = Timer::from_seconds(duration, TimerMode::Once);
        freeze.active = true;
    }

    pub fn flash_white(flash: &mut FlashWhite, duration: f32) {
        flash.amount = 1.0;
        flash.timer = Timer::from_seconds(duration, TimerMode::Once);
    }

    pub fn chromatic_pulse(chrom: &mut ChromaticAberration, strength: f32) {
        chrom.0 = chrom.0.max(strength);
    }
}

pub struct ScreenEffectsPlugin;
impl Plugin for ScreenEffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Trauma>()
            .init_resource::<FlashWhite>()
            .init_resource::<FreezeFrame>()
            .init_resource::<ChromaticAberration>()
            .add_systems(
                Update,
                (apply_trauma_shake, tick_flash, tick_freeze, tick_chromatic).chain(),
            );
    }
}

fn tick_chromatic(time: Res<Time>, mut chrom: ResMut<ChromaticAberration>) {
    chrom.0 = (chrom.0 - 2.0 * time.delta_secs()).max(0.0);
}

fn apply_trauma_shake(
    time: Res<Time>,
    mut trauma: ResMut<Trauma>,
    mut q: Query<(&mut Transform, &CameraBase), With<Camera2d>>,
) {
    let mut rng = rand::rng();
    let t = trauma.0;
    let shake_pow = t * t;
    for (mut tf, base) in &mut q {
        if shake_pow > 0.001 {
            let mag = shake_pow * 12.0;
            let ox = rng.random_range(-mag..mag);
            let oy = rng.random_range(-mag..mag);
            let rot = rng.random_range(-0.05..0.05) * shake_pow;
            tf.translation = base.translation + Vec3::new(ox, oy, 0.0);
            tf.rotation = Quat::from_rotation_z(base.rotation + rot);
        } else {
            tf.translation = base.translation;
            tf.rotation = Quat::from_rotation_z(base.rotation);
        }
    }
    trauma.0 = (trauma.0 - 1.5 * time.delta_secs()).max(0.0);
}

fn tick_flash(real: Res<Time<Real>>, mut flash: ResMut<FlashWhite>) {
    if flash.amount <= 0.0 {
        return;
    }
    flash.timer.tick(real.delta());
    let t = flash.timer.fraction();
    flash.amount = 1.0 - t;
    if flash.timer.just_finished()
        || flash.timer.elapsed_secs() >= flash.timer.duration().as_secs_f32()
    {
        flash.amount = 0.0;
    }
}

fn tick_freeze(
    real: Res<Time<Real>>,
    mut freeze: ResMut<FreezeFrame>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    if !freeze.active {
        return;
    }
    virtual_time.pause();
    freeze.timer.tick(real.delta());
    if freeze.timer.just_finished() {
        freeze.active = false;
        virtual_time.unpause();
    }
}
