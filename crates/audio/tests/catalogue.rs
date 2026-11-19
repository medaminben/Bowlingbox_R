use std::collections::BTreeSet;
use std::path::Path;

use bowlingbox_audio::{ALL_SOUNDS, SoundId, decode_info, embedded_data, file_name, is_loop};

#[test]
fn every_sound_has_a_unique_wav_file_and_data() {
    let mut names = BTreeSet::new();
    for id in ALL_SOUNDS {
        let name = file_name(id);
        assert_eq!(
            Path::new(name).extension().and_then(|e| e.to_str()),
            Some("wav"),
            "{name}"
        );
        assert!(names.insert(name), "{name} is listed twice");
        assert!(
            embedded_data(id).len() > 44,
            "{name} is not more than a header"
        );
    }
    assert_eq!(names.len(), 13);
}

#[test]
fn file_names_match_the_cpp_catalogue() {
    let expected = [
        (SoundId::Roll, "roll.wav"),
        (SoundId::GutterDrop, "gutter_drop.wav"),
        (SoundId::PinHit, "pin_hit.wav"),
        (SoundId::PinClatter, "pin_clatter.wav"),
        (SoundId::ChargeTone, "charge.wav"),
        (SoundId::Release, "release.wav"),
        (SoundId::Strike, "strike.wav"),
        (SoundId::Spare, "spare.wav"),
        (SoundId::NewGame, "new_game.wav"),
        (SoundId::ThemeSwitch, "theme_switch.wav"),
        (SoundId::AmbientNeon, "ambient_neon.wav"),
        (SoundId::AmbientClassic, "ambient_classic.wav"),
        (SoundId::AmbientOutdoor, "ambient_outdoor.wav"),
    ];
    for (id, name) in expected {
        assert_eq!(file_name(id), name);
    }
    assert_eq!(ALL_SOUNDS.map(file_name), expected.map(|(_, n)| n));
}

#[test]
fn loops_are_roll_charge_and_the_ambiences() {
    for id in ALL_SOUNDS {
        let expected = matches!(
            id,
            SoundId::Roll
                | SoundId::ChargeTone
                | SoundId::AmbientNeon
                | SoundId::AmbientClassic
                | SoundId::AmbientOutdoor
        );
        assert_eq!(is_loop(id), expected, "{}", file_name(id));
    }
}

// Effects keep their highs at 44.1 kHz; the long ambience beds stay at 22.05 kHz to save space.
#[test]
fn every_sound_decodes_to_mono_at_a_supported_rate_within_its_length_limit() {
    for id in ALL_SOUNDS {
        let name = file_name(id);
        let info = decode_info(id).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(info.channels, 1, "{name}");
        assert!(
            info.sample_rate == 22_050 || info.sample_rate == 44_100,
            "{name}: {}",
            info.sample_rate
        );
        if is_loop(id) {
            assert!(
                (0.5..=45.0).contains(&info.seconds),
                "{name}: {}",
                info.seconds
            );
        } else {
            assert!(
                (0.05..=3.0).contains(&info.seconds),
                "{name}: {}",
                info.seconds
            );
        }
    }
}

#[test]
fn embedded_sounds_stay_under_six_megabytes() {
    let total: usize = ALL_SOUNDS.iter().map(|&id| embedded_data(id).len()).sum();
    assert!(total < 6 * 1024 * 1024, "{total} bytes");
}

#[test]
fn sources_lists_exactly_the_wav_files_present() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("sounds");
    let present: BTreeSet<String> = std::fs::read_dir(&dir)
        .expect("sounds directory")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.to_ascii_lowercase().ends_with(".wav"))
        .collect();
    let sources = std::fs::read_to_string(dir.join("SOURCES.md")).expect("SOURCES.md");
    let listed: BTreeSet<String> = sources
        .lines()
        .filter_map(|l| l.strip_prefix("| `"))
        .filter_map(|l| l.split('`').next())
        .filter(|n| Path::new(n).extension().is_some_and(|e| e == "wav"))
        .map(str::to_owned)
        .collect();
    assert_eq!(present, listed);
    let catalogue: BTreeSet<String> = ALL_SOUNDS
        .iter()
        .map(|&id| file_name(id).to_owned())
        .collect();
    assert_eq!(present, catalogue);
}
