use bowlingbox_audio::{AudioOutput, LoopSlot, SilentOutput, SoundId, Voice, open_output_with};

#[test]
fn voice_default_is_full_volume_normal_pitch_centred() {
    let v = Voice::default();
    assert_eq!((v.volume, v.pitch, v.pan), (1.0, 1.0, 0.0));
}

#[test]
fn silent_output_tracks_mute_and_ignores_cues() {
    let mut out = SilentOutput::default();
    assert!(!out.muted());
    out.play(SoundId::Strike, Voice::default());
    out.set_muted(true);
    assert!(out.muted());
    out.set_muted(false);
    assert!(!out.muted());
}

#[test]
fn silent_output_clamps_the_master_volume() {
    let mut out = SilentOutput::default();
    assert_eq!(out.volume(), 1.0);
    out.set_volume(0.4);
    assert_eq!(out.volume(), 0.4);
    out.set_volume(2.0);
    assert_eq!(out.volume(), 1.0);
    out.set_volume(-1.0);
    assert_eq!(out.volume(), 0.0);
}

#[test]
fn set_loop_same_sound_updates_the_voice_without_restarting() {
    let mut out = SilentOutput::default();
    let a = Voice {
        volume: 0.35,
        ..Voice::default()
    };
    let b = Voice {
        volume: 0.5,
        pitch: 1.2,
        pan: -0.3,
    };
    assert_eq!(out.loop_state(LoopSlot::Ambient), None);
    out.set_loop(LoopSlot::Ambient, Some((SoundId::AmbientNeon, a)));
    assert_eq!(
        out.loop_state(LoopSlot::Ambient),
        Some((SoundId::AmbientNeon, a))
    );
    assert_eq!(out.loop_starts(LoopSlot::Ambient), 1);
    out.set_loop(LoopSlot::Ambient, Some((SoundId::AmbientNeon, b)));
    assert_eq!(
        out.loop_state(LoopSlot::Ambient),
        Some((SoundId::AmbientNeon, b))
    );
    assert_eq!(out.loop_starts(LoopSlot::Ambient), 1);
}

#[test]
fn set_loop_other_sound_restarts_and_none_stops() {
    let mut out = SilentOutput::default();
    let v = Voice::default();
    out.set_loop(LoopSlot::Ambient, Some((SoundId::AmbientNeon, v)));
    out.set_loop(LoopSlot::Ambient, Some((SoundId::AmbientClassic, v)));
    assert_eq!(
        out.loop_state(LoopSlot::Ambient),
        Some((SoundId::AmbientClassic, v))
    );
    assert_eq!(out.loop_starts(LoopSlot::Ambient), 2);
    out.set_loop(LoopSlot::Ambient, None);
    assert_eq!(out.loop_state(LoopSlot::Ambient), None);
    out.set_loop(LoopSlot::Ambient, Some((SoundId::AmbientClassic, v)));
    assert_eq!(out.loop_starts(LoopSlot::Ambient), 3);
    assert_eq!(out.loop_state(LoopSlot::Roll), None);
}

#[test]
fn failing_factory_gives_silent_output_and_a_warning() {
    let (mut out, warning) = open_output_with(|| Err("no audio device".to_owned()));
    assert_eq!(warning.as_deref(), Some("sound disabled: no audio device"));
    out.set_volume(0.5);
    assert_eq!(out.volume(), 0.5);
    out.set_muted(true);
    assert!(out.muted());
}

#[test]
fn working_factory_is_used_without_a_warning() {
    let (out, warning) = open_output_with(|| {
        let mut silent = SilentOutput::default();
        silent.set_volume(0.25);
        Ok(Box::new(silent) as Box<dyn AudioOutput>)
    });
    assert_eq!(warning, None);
    assert_eq!(out.volume(), 0.25);
}
