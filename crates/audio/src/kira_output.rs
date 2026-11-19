//! Plays through the default audio device with kira.

use std::io::Cursor;
use std::sync::Arc;

use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::track::{TrackBuilder, TrackHandle};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Frame, Tween};

use crate::{ALL_SOUNDS, AudioOutput, LoopSlot, SoundId, Voice, embedded_data, file_name, is_loop};

/// Linear gain (0..1) as kira decibels; 0 is silence.
fn gain_to_db(gain: f32) -> Decibels {
    if gain <= 0.0 {
        Decibels::SILENCE
    } else {
        Decibels((20.0 * gain.log10()).max(Decibels::SILENCE.0))
    }
}

/// Playback rate kira accepts: finite and above 0.
fn clean_pitch(pitch: f32) -> f64 {
    if pitch.is_finite() && pitch > 0.0 {
        f64::from(pitch)
    } else {
        1.0
    }
}

/// kira pans -1 left .. +1 right, the same as [`Voice::pan`].
fn clean_pan(pan: f32) -> f32 {
    if pan.is_finite() {
        pan.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

#[allow(clippy::cast_possible_truncation)] // samples are normalised to -1..1 before the cast
fn decode(id: SoundId) -> Result<StaticSoundData, String> {
    let name = file_name(id);
    let mut reader = hound::WavReader::new(Cursor::new(embedded_data(id)))
        .map_err(|e| format!("{name}: {e}"))?;
    let spec = reader.spec();
    let channels = usize::from(spec.channels);
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (f64::from(1_u32 << (spec.bits_per_sample - 1)));
            reader
                .samples::<i32>()
                .map(|s| s.map(|s| (f64::from(s) * scale) as f32))
                .collect()
        }
    }
    .map_err(|e| format!("{name}: {e}"))?;
    let frames: Vec<Frame> = samples
        .chunks_exact(channels)
        .map(|c| match c {
            [m] => Frame::from_mono(*m),
            [l, r, ..] => Frame::new(*l, *r),
            [] => Frame::ZERO,
        })
        .collect();
    let data = StaticSoundData {
        sample_rate: spec.sample_rate,
        frames: Arc::from(frames),
        settings: kira::sound::static_sound::StaticSoundSettings::new(),
        slice: None,
    };
    Ok(if is_loop(id) {
        data.loop_region(..)
    } else {
        data
    })
}

fn voiced(data: &StaticSoundData, voice: Voice) -> StaticSoundData {
    data.volume(gain_to_db(voice.volume.clamp(0.0, 1.0)))
        .playback_rate(clean_pitch(voice.pitch))
        .panning(clean_pan(voice.pan))
}

/// One kira track per loop slot plus one for one-shots, under the main track
/// that carries the master volume and the mute.
pub struct KiraOutput {
    manager: AudioManager<DefaultBackend>,
    sounds: Vec<StaticSoundData>,
    one_shots: TrackHandle,
    loop_tracks: [TrackHandle; 3],
    loops: [Option<(SoundId, StaticSoundHandle)>; 3],
    muted: bool,
    volume: f32,
}

impl KiraOutput {
    /// Opens the default device and decodes every embedded sound.
    ///
    /// # Errors
    /// The reason, when there is no device or a sound cannot be decoded.
    pub fn new() -> Result<Self, String> {
        let sounds = ALL_SOUNDS
            .iter()
            .map(|&id| decode(id))
            .collect::<Result<Vec<_>, _>>()?;
        let mut manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())
            .map_err(|e| e.to_string())?;
        let mut track = || {
            manager
                .add_sub_track(TrackBuilder::new())
                .map_err(|e| e.to_string())
        };
        let one_shots = track()?;
        let loop_tracks = [track()?, track()?, track()?];
        Ok(Self {
            manager,
            sounds,
            one_shots,
            loop_tracks,
            loops: [None, None, None],
            muted: false,
            volume: 1.0,
        })
    }

    fn apply_master(&mut self) {
        // Muting only silences the main track: loops keep running, so unmuting resumes in place.
        let gain = if self.muted { 0.0 } else { self.volume };
        self.manager
            .main_track()
            .set_volume(gain_to_db(gain), Tween::default());
    }
}

impl AudioOutput for KiraOutput {
    fn play(&mut self, sound: SoundId, voice: Voice) {
        let data = voiced(&self.sounds[sound as usize], voice);
        // A full track drops the cue; a missed effect is not worth an error.
        let _ = self.one_shots.play(data);
    }

    fn set_loop(&mut self, slot: LoopSlot, sound: Option<(SoundId, Voice)>) {
        let i = slot as usize;
        match (sound, &mut self.loops[i]) {
            (Some((id, voice)), Some((current, handle))) if *current == id => {
                let tween = Tween::default();
                handle.set_volume(gain_to_db(voice.volume.clamp(0.0, 1.0)), tween);
                handle.set_playback_rate(clean_pitch(voice.pitch), tween);
                handle.set_panning(clean_pan(voice.pan), tween);
            }
            (new, current) => {
                if let Some((_, mut handle)) = current.take() {
                    handle.stop(Tween::default());
                }
                if let Some((id, voice)) = new {
                    let data = voiced(&self.sounds[id as usize], voice);
                    if let Ok(handle) = self.loop_tracks[i].play(data) {
                        self.loops[i] = Some((id, handle));
                    }
                }
            }
        }
    }

    fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        self.apply_master();
    }

    fn muted(&self) -> bool {
        self.muted
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        self.apply_master();
    }

    fn volume(&self) -> f32 {
        self.volume
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_maps_to_decibels() {
        assert_eq!(gain_to_db(0.0), Decibels::SILENCE);
        assert_eq!(gain_to_db(-1.0), Decibels::SILENCE);
        assert!(gain_to_db(1.0).0.abs() < 1e-6);
        assert!((gain_to_db(0.5).0 + 6.0206).abs() < 1e-3);
        assert_eq!(gain_to_db(1e-9), Decibels::SILENCE);
    }

    #[test]
    fn pitch_and_pan_are_made_valid() {
        assert_eq!(clean_pitch(2.0), 2.0);
        assert_eq!(clean_pitch(0.0), 1.0);
        assert_eq!(clean_pitch(f32::NAN), 1.0);
        assert_eq!(clean_pan(1.0), 1.0);
        assert_eq!(clean_pan(-3.0), -1.0);
        assert_eq!(clean_pan(f32::NAN), 0.0);
    }

    #[test]
    fn every_embedded_sound_converts_to_frames_without_a_device() {
        for id in ALL_SOUNDS {
            let data = decode(id).unwrap();
            assert!(data.num_frames() > 0, "{}", file_name(id));
            assert!(
                data.frames.iter().all(|f| f.left.abs() <= 1.0),
                "{}",
                file_name(id)
            );
        }
    }
}
