use bevy_ecs::prelude::World;
use repame_audio::{AmplitudeEnvelope, Audio, AudioChannel, CueDef, CueSpec, Sweep, Waveform};

use crate::game::{PendingCues, TONE_CUES, ToneCue};

const AUDIO_RETRY_SECS: f32 = 1.0;
const RATE: u32 = 22_050;

/// The old table wrote its sine into 16-bit PCM at `20_000`, so the cue's gain
/// in full scale is that peak over the decoder's `i16 -> f32` divisor.
const TONE_PEAK: f32 = 20_000.0 / 32_768.0;

/// The old envelope was `exp(-4 * t_frac)`, a `1/e` fall every quarter of the
/// cue, which is an ADSR decay of `secs / 4`.
const TONE_DECAY: f32 = 4.0;

pub struct GameAudio {
    audio: Audio,
    retry_in: f32,
}

impl GameAudio {
    pub fn new() -> Self {
        Self::with_audio(Audio::try_init().unwrap_or_else(|_| Audio::noop()))
    }

    pub fn ensure_live(&mut self, dt_secs: f32) {
        if self.audio.is_live() {
            return;
        }
        self.retry_in -= dt_secs;
        if self.retry_in > 0.0 {
            return;
        }
        self.retry_in = AUDIO_RETRY_SECS;
        let mut audio = Audio::try_init().unwrap_or_else(|_| Audio::noop());
        if !audio.is_live() {
            return;
        }
        if let Err(error) = load_bank(&mut audio) {
            log::warn!("audio bank failed to load: {error}");
        }
        self.audio = audio;
    }

    pub fn play(&mut self, cue: &str) {
        let _ = self.audio.play(cue);
    }

    pub fn drain(&mut self, world: &mut World) {
        let Some(mut pending) = world.get_resource_mut::<PendingCues>() else {
            return;
        };
        let cues = std::mem::take(&mut pending.0);
        for cue in cues {
            self.play(cue);
        }
    }

    pub fn apply_volumes(&mut self, master: f32, sfx: f32, music: f32) {
        let channels = self.audio.channels();
        if channels.master() != master {
            self.audio.set_channel(AudioChannel::Master, master);
        }
        if channels.sfx() != sfx {
            self.audio.set_channel(AudioChannel::Sfx, sfx);
        }
        if channels.music() != music {
            self.audio.set_channel(AudioChannel::Music, music);
        }
    }

    pub fn update(&mut self, dt_secs: f32) {
        self.audio.update(dt_secs);
    }

    fn with_audio(mut audio: Audio) -> Self {
        if let Err(error) = load_bank(&mut audio) {
            log::warn!("audio bank failed to load: {error}");
        }
        Self {
            audio,
            retry_in: AUDIO_RETRY_SECS,
        }
    }
}

impl Default for GameAudio {
    fn default() -> Self {
        Self::new()
    }
}

fn load_bank(audio: &mut Audio) -> anyhow::Result<()> {
    for cue in TONE_CUES {
        let def = CueDef {
            bus: AudioChannel::Sfx,
            gain: cue.gain,
            ..Default::default()
        };
        audio.load_synth(cue.name, def, &[tone_spec(cue)])?;
    }
    Ok(())
}

/// The cue's length was authored as whole milliseconds at 22.05 kHz, so the
/// frame count is rounded the same way the table was.
fn tone_spec(cue: &ToneCue) -> CueSpec {
    let frames = (RATE as u64 * cue.millis / 1000).max(1) as f32;
    let secs = frames / RATE as f32;
    CueSpec::new(Waveform::Sine, cue.freq, secs)
        .with_sweep(cue.freq, Sweep::Constant)
        .with_amplitude(AmplitudeEnvelope::Adsr {
            attack_s: 0.0,
            decay_s: secs / TONE_DECAY,
            sustain: 0.0,
            release_s: 0.0,
        })
        .with_gain(TONE_PEAK)
        .with_sample_rate(RATE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_cues_render_for_their_whole_length() {
        for cue in TONE_CUES {
            let spec = tone_spec(cue);
            let expected = (RATE as u64 * cue.millis / 1000).max(1) as usize;
            assert_eq!(
                spec.duration_frames(RATE).expect("tone has a length"),
                expected,
                "{}",
                cue.name
            );
            let rendered = spec.render().expect("tone renders");
            assert_eq!(rendered.len_frames(), expected, "{}", cue.name);
        }
    }

    #[test]
    fn bank_loads_every_cue() {
        let mut audio = Audio::noop();
        load_bank(&mut audio).expect("all tone cues load");
    }

    #[test]
    fn drain_takes_pending_cues() {
        let mut audio = GameAudio::with_audio(Audio::noop());
        let mut world = World::new();
        world.insert_resource(PendingCues(vec!["role_reveal", "body"]));
        audio.drain(&mut world);
        assert!(world.resource::<PendingCues>().0.is_empty());
    }
}
