use bevy::prelude::*;

#[derive(Component)]
pub struct PopIn {
    pub timer: Timer,
    pub delay: Timer,
}

#[derive(Component)]
pub struct SquashStretch {
    pub timer: Timer,
    pub amount: Vec2,
    pub original: Option<Vec3>,
}

#[derive(Component)]
pub struct BounceScale {
    pub timer: Timer,
    pub peak: f32,
    pub original: Option<Vec3>,
}

#[derive(Component)]
pub struct Shake {
    pub timer: Timer,
    pub intensity: f32,
    pub original: Option<Vec3>,
}

#[derive(Component)]
pub struct Particle {
    pub lifetime: Timer,
    pub velocity: Vec2,
    pub start_color: Color,
}

pub struct Juice;

impl Juice {
    pub fn pop_in(commands: &mut Commands, entity: Entity, duration: f32) {
        commands.entity(entity).insert(PopIn {
            timer: Timer::from_seconds(duration, TimerMode::Once),
            delay: Timer::from_seconds(0.0, TimerMode::Once),
        });
    }

    pub fn squash_stretch(commands: &mut Commands, entity: Entity, amount: Vec2, duration: f32) {
        commands.entity(entity).insert(SquashStretch {
            timer: Timer::from_seconds(duration, TimerMode::Once),
            amount,
            original: None,
        });
    }

    pub fn bounce_scale(commands: &mut Commands, entity: Entity, peak: f32, duration: f32) {
        commands.entity(entity).insert(BounceScale {
            timer: Timer::from_seconds(duration, TimerMode::Once),
            peak,
            original: None,
        });
    }

    pub fn shake(commands: &mut Commands, entity: Entity, intensity: f32, duration: f32) {
        commands.entity(entity).insert(Shake {
            timer: Timer::from_seconds(duration, TimerMode::Once),
            intensity,
            original: None,
        });
    }
}

pub struct JuicePlugin;
impl Plugin for JuicePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, animate_juice);
    }
}

fn animate_juice(
    time: Res<Time>,
    mut commands: Commands,
    mut set: ParamSet<(
        Query<(Entity, &mut PopIn, &mut Transform)>,
        Query<(Entity, &mut SquashStretch, &mut Transform)>,
        Query<(Entity, &mut BounceScale, &mut Transform)>,
        Query<(Entity, &mut Shake, &mut Transform)>,
        Query<(Entity, &mut Particle, &mut Sprite, &mut Transform)>,
    )>,
) {
    let dt = time.delta();
    let elapsed = time.elapsed_secs();

    for (e, mut pop, mut tf) in set.p0().iter_mut() {
        pop.delay.tick(dt);
        if !pop.delay.just_finished()
            && pop.delay.elapsed_secs() < pop.delay.duration().as_secs_f32()
        {
            tf.scale = Vec3::splat(0.0);
            continue;
        }
        pop.timer.tick(dt);
        let t = pop.timer.fraction();
        let overshoot = 1.70158;
        let t2 = t - 1.0;
        let s = t2 * t2 * ((overshoot + 1.0) * t2 + overshoot) + 1.0;
        tf.scale = Vec3::splat(s);
        if pop.timer.just_finished() {
            tf.scale = Vec3::ONE;
            commands.entity(e).remove::<PopIn>();
        }
    }

    for (e, mut sq, mut tf) in set.p1().iter_mut() {
        if sq.original.is_none() {
            sq.original = Some(tf.scale);
        }
        sq.timer.tick(dt);
        let t = sq.timer.fraction();
        let orig = sq.original.unwrap_or(Vec3::ONE);
        if t < 0.5 {
            let u = t / 0.5;
            tf.scale = orig
                * Vec3::new(
                    1.0 + (sq.amount.x - 1.0) * u,
                    1.0 + (sq.amount.y - 1.0) * u,
                    1.0,
                );
        } else {
            let u = (t - 0.5) / 0.5;
            let a = Vec3::new(sq.amount.x, sq.amount.y, 1.0);
            tf.scale = orig * (a + (Vec3::ONE - a) * u);
        }
        if sq.timer.just_finished() {
            tf.scale = orig;
            commands.entity(e).remove::<SquashStretch>();
        }
    }

    for (e, mut b, mut tf) in set.p2().iter_mut() {
        if b.original.is_none() {
            b.original = Some(tf.scale);
        }
        b.timer.tick(dt);
        let t = b.timer.fraction();
        let orig = b.original.unwrap_or(Vec3::ONE);
        let wave = if t < 0.3 {
            let u = t / 0.3;
            1.0 + (b.peak - 1.0) * u
        } else {
            let u = (t - 0.3) / 0.7;
            b.peak + (1.0 - b.peak) * u
        };
        tf.scale = orig * wave;
        if b.timer.just_finished() {
            tf.scale = orig;
            commands.entity(e).remove::<BounceScale>();
        }
    }

    for (e, mut sh, mut tf) in set.p3().iter_mut() {
        if sh.original.is_none() {
            sh.original = Some(tf.translation);
        }
        sh.timer.tick(dt);
        let decay = 1.0 - sh.timer.fraction();
        let o = sh.original.unwrap_or_default();
        tf.translation = o + Vec3::new(
            (elapsed * 50.0).sin() * sh.intensity * decay,
            (elapsed * 47.0).cos() * sh.intensity * decay,
            0.0,
        );
        if sh.timer.just_finished() {
            tf.translation = o;
            commands.entity(e).remove::<Shake>();
        }
    }

    for (entity, mut p, mut sprite, mut tf) in set.p4().iter_mut() {
        p.lifetime.tick(dt);
        if p.lifetime.just_finished() {
            commands.entity(entity).despawn();
            continue;
        }
        tf.translation += (p.velocity * dt.as_secs_f32()).extend(0.0);
        p.velocity.y -= 60.0 * dt.as_secs_f32();
        let t = p.lifetime.fraction();
        sprite.color = Color::srgba(
            p.start_color.to_linear().red,
            p.start_color.to_linear().green,
            p.start_color.to_linear().blue,
            (1.0 - t).clamp(0.0, 1.0),
        );
    }
}
