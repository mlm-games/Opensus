use bevy::prelude::*;

#[derive(Component)]
pub struct HoverScale {
    pub normal: Vec3,
    pub hovered: Vec3,
    pub speed: f32,
}

impl Default for HoverScale {
    fn default() -> Self {
        Self {
            normal: Vec3::ONE,
            hovered: Vec3::splat(1.08),
            speed: 14.0,
        }
    }
}

#[derive(Component)]
pub struct Typewriter {
    pub full_text: String,
    pub visible_chars: usize,
    pub timer: Timer,
    pub finished: bool,
}

#[derive(Component)]
pub struct NumberCounter {
    pub from: f32,
    pub to: f32,
    pub current: f32,
    pub timer: Timer,
    pub finished: bool,
}

pub struct UiEffectsPlugin;
impl Plugin for UiEffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (hover_scale_system, typewriter_system, number_counter_system),
        );
    }
}

fn hover_scale_system(time: Res<Time>, mut q: Query<(&Interaction, &HoverScale, &mut Transform)>) {
    for (interaction, hs, mut tf) in &mut q {
        let target = match *interaction {
            Interaction::Hovered | Interaction::Pressed => hs.hovered,
            _ => hs.normal,
        };
        if tf.scale != target {
            tf.scale = tf
                .scale
                .lerp(target, (hs.speed * time.delta_secs()).min(1.0));
        }
    }
}

fn typewriter_system(time: Res<Time>, mut q: Query<(&mut Text, &mut Typewriter)>) {
    for (mut text, mut tw) in &mut q {
        if tw.finished {
            continue;
        }
        tw.timer.tick(time.delta());
        if tw.timer.just_finished() {
            tw.visible_chars = (tw.visible_chars + 1).min(tw.full_text.len());
            let s: String = tw.full_text.chars().take(tw.visible_chars).collect();
            text.0 = s;
            if tw.visible_chars >= tw.full_text.len() {
                tw.finished = true;
            } else {
                tw.timer.reset();
            }
        }
    }
}

fn number_counter_system(time: Res<Time>, mut q: Query<(&mut Text, &mut NumberCounter)>) {
    for (mut text, mut nc) in &mut q {
        if nc.finished {
            continue;
        }
        nc.timer.tick(time.delta());
        let t = nc.timer.fraction().clamp(0.0, 1.0);
        nc.current = nc.from + (nc.to - nc.from) * t;
        text.0 = format!("{:.0}", nc.current);
        if nc.timer.just_finished() {
            nc.current = nc.to;
            nc.finished = true;
        }
    }
}
