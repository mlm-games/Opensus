use bevy_ecs::prelude::World;
use repame_audio::{Audio, AudioChannel, CueDef};

use crate::game::{PendingCues, TONE_CUES};

pub struct GameAudio {
    audio: Audio,
}

impl GameAudio {
    pub fn new() -> Self {
        Self::with_audio(Audio::try_init().unwrap_or_else(|_| Audio::noop()))
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
        Self { audio }
    }
}

impl Default for GameAudio {
    fn default() -> Self {
        Self::new()
    }
}

fn load_bank(audio: &mut Audio) -> anyhow::Result<()> {
    for cue in TONE_CUES {
        let wav = synth_tone(cue.freq, cue.millis);
        let def = CueDef {
            bus: AudioChannel::Sfx,
            gain: cue.gain,
            ..Default::default()
        };
        audio.load_cue(cue.name, def, &[wav.as_slice()])?;
    }
    Ok(())
}

fn synth_tone(freq_hz: f32, millis: u64) -> Vec<u8> {
    const RATE: u32 = 22_050;
    let samples = ((RATE as u64 * millis / 1000).max(1)) as usize;
    let mut pcm = Vec::with_capacity(samples * 2);
    for i in 0..samples {
        let t = i as f32 / RATE as f32;
        let envelope = (-4.0 * i as f32 / samples as f32).exp();
        let value = (t * freq_hz * std::f32::consts::TAU).sin() * envelope * 20000.0;
        pcm.extend_from_slice(&(value as i16).to_le_bytes());
    }
    let data_len = pcm.len() as u32;
    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(&pcm);
    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_cues_decode_as_wav() {
        for cue in TONE_CUES {
            let wav = synth_tone(cue.freq, cue.millis);
            assert_eq!(&wav[0..4], b"RIFF");
            assert_eq!(&wav[8..12], b"WAVE");
            let expected = 44 + 2 * (22_050u64 * cue.millis / 1000) as usize;
            assert_eq!(wav.len(), expected, "{}", cue.name);
            repame_audio::decode_bytes(&wav).expect("tone wav decodes");
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
