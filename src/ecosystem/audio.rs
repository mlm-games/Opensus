use bevy::prelude::*;
use rand::RngExt;

#[derive(Component, Default)]
pub struct SfxChannel;

#[derive(Component, Default)]
pub struct MusicChannel;

#[derive(Component, Default)]
pub struct UiChannel;

#[derive(Resource)]
pub struct AudioChannels {
    pub master: f32,
    pub sfx: f32,
    pub music: f32,
    pub ui: f32,
}

impl Default for AudioChannels {
    fn default() -> Self {
        Self {
            master: 1.0,
            sfx: 1.0,
            music: 0.8,
            ui: 1.0,
        }
    }
}

impl AudioChannels {
    pub fn sfx_volume(&self) -> f32 {
        self.master * self.sfx
    }

    pub fn music_volume(&self) -> f32 {
        self.master * self.music
    }

    pub fn ui_volume(&self) -> f32 {
        self.master * self.ui
    }
}

pub struct AudioM;

impl AudioM {
    pub fn play_sfx(commands: &mut Commands, handle: Handle<AudioSource>, volume: f32) {
        commands.spawn((
            AudioPlayer::new(handle),
            PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(volume)),
            SfxChannel,
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
            SfxChannel,
        ));
    }

    pub fn play_music(
        commands: &mut Commands,
        handle: Handle<AudioSource>,
        volume: f32,
        music_q: &Query<Entity, With<MusicChannel>>,
    ) {
        for e in music_q.iter() {
            commands.entity(e).despawn();
        }
        commands.spawn((
            AudioPlayer::new(handle),
            PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(volume)),
            MusicChannel,
        ));
    }

    pub fn stop_music(commands: &mut Commands, music_q: &Query<Entity, With<MusicChannel>>) {
        for e in music_q.iter() {
            commands.entity(e).despawn();
        }
    }

    pub fn play_ui(commands: &mut Commands, handle: Handle<AudioSource>, volume: f32) {
        commands.spawn((
            AudioPlayer::new(handle),
            PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(volume)),
            UiChannel,
        ));
    }
}

fn sync_channel_volumes(
    channels: Res<AudioChannels>,
    mut q: Query<(
        &mut AudioSink,
        Option<&SfxChannel>,
        Option<&MusicChannel>,
        Option<&UiChannel>,
    )>,
) {
    for (mut sink, sfx, music, ui) in &mut q {
        let vol = if sfx.is_some() {
            channels.sfx_volume()
        } else if music.is_some() {
            channels.music_volume()
        } else if ui.is_some() {
            channels.ui_volume()
        } else {
            channels.master
        };
        sink.set_volume(bevy::audio::Volume::Linear(vol));
    }
}

pub struct AudioPlugin;
impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AudioChannels>()
            .add_systems(Update, sync_channel_volumes);
    }
}
