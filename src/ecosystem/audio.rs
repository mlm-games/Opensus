use bevy::prelude::*;
use rand::RngExt;

#[derive(Resource, Default)]
pub struct AudioM;

impl AudioM {
    pub fn play_sfx(commands: &mut Commands, handle: Handle<AudioSource>, volume: f32) {
        commands.spawn((
            AudioPlayer::new(handle),
            PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(volume)),
        ));
    }

    pub fn play_sfx_varied(
        commands: &mut Commands,
        handle: Handle<AudioSource>,
        volume: f32,
        pitch_var: f32,
    ) {
        let mut rng = rand::rng();
        let pitch = 1.0 + rng.random_range(-pitch_var..pitch_var);
        commands.spawn((
            AudioPlayer::new(handle),
            PlaybackSettings::DESPAWN
                .with_volume(bevy::audio::Volume::Linear(volume))
                .with_speed(pitch),
        ));
    }
}

pub struct AudioPlugin;
impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AudioM>();
    }
}
