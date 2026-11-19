//! The sound settings in eframe's key-value storage. The values are strings, so a hand-edited or
//! damaged file can hold anything: every read falls back to the default instead of failing.

use crate::sound_settings::SoundSettings;

pub const MUTED_KEY: &str = "sound.muted";
pub const VOLUME_KEY: &str = "sound.volume";
pub const AMBIENCE_KEY: &str = "sound.ambience";

fn read_bool(storage: &dyn eframe::Storage, key: &str, default: bool) -> bool {
    match storage.get_string(key).as_deref().map(str::trim) {
        Some("true") => true,
        Some("false") => false,
        _ => default,
    }
}

fn read_volume(storage: &dyn eframe::Storage, default: i32) -> i32 {
    storage
        .get_string(VOLUME_KEY)
        .and_then(|text| text.trim().parse::<i64>().ok())
        .map_or(default, |volume| {
            i32::try_from(volume.clamp(0, 100)).unwrap_or(default)
        })
}

/// The stored settings; a missing or unreadable value gives that setting's default.
pub fn load(storage: Option<&dyn eframe::Storage>) -> SoundSettings {
    let default = SoundSettings::default();
    let Some(storage) = storage else {
        return default;
    };
    SoundSettings {
        muted: read_bool(storage, MUTED_KEY, default.muted),
        volume: read_volume(storage, default.volume),
        ambience: read_bool(storage, AMBIENCE_KEY, default.ambience),
    }
}

pub fn save(storage: &mut dyn eframe::Storage, settings: SoundSettings) {
    storage.set_string(MUTED_KEY, settings.muted.to_string());
    storage.set_string(VOLUME_KEY, settings.volume.clamp(0, 100).to_string());
    storage.set_string(AMBIENCE_KEY, settings.ambience.to_string());
}

/// An in-memory `eframe::Storage` for tests.
#[cfg(test)]
pub mod test_storage {
    use std::collections::HashMap;

    #[derive(Default)]
    pub struct Fake(pub HashMap<String, String>);

    impl eframe::Storage for Fake {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }
        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }
        fn flush(&mut self) {}
    }
}

#[cfg(test)]
mod tests {
    use super::test_storage::Fake;
    use super::*;
    use eframe::Storage;

    fn with(pairs: &[(&str, &str)]) -> Fake {
        let mut fake = Fake::default();
        for (k, v) in pairs {
            fake.set_string(k, (*v).to_owned());
        }
        fake
    }

    #[test]
    fn settings_round_trip() {
        let mut fake = Fake::default();
        let s = SoundSettings {
            muted: true,
            volume: 30,
            ambience: false,
        };
        save(&mut fake, s);
        assert_eq!(load(Some(&fake)), s);
    }

    #[test]
    fn the_keys_are_the_documented_ones() {
        let mut fake = Fake::default();
        save(&mut fake, SoundSettings::default());
        assert_eq!(fake.get_string("sound.muted").as_deref(), Some("false"));
        assert_eq!(fake.get_string("sound.volume").as_deref(), Some("80"));
        assert_eq!(fake.get_string("sound.ambience").as_deref(), Some("true"));
    }

    #[test]
    fn no_storage_or_an_empty_one_gives_the_defaults() {
        assert_eq!(load(None), SoundSettings::default());
        assert_eq!(load(Some(&Fake::default())), SoundSettings::default());
    }

    #[test]
    fn volume_is_clamped_to_0_to_100() {
        assert_eq!(load(Some(&with(&[(VOLUME_KEY, "250")]))).volume, 100);
        assert_eq!(load(Some(&with(&[(VOLUME_KEY, "-5")]))).volume, 0);
        assert_eq!(
            load(Some(&with(&[(VOLUME_KEY, "99999999999999999999")]))).volume,
            80
        );
        assert_eq!(load(Some(&with(&[(VOLUME_KEY, " 40 ")]))).volume, 40);
    }

    #[test]
    fn unparsable_values_give_the_default_of_that_setting_only() {
        let s = load(Some(&with(&[
            (MUTED_KEY, "maybe"),
            (VOLUME_KEY, "loud"),
            (AMBIENCE_KEY, "false"),
        ])));
        assert_eq!(
            s,
            SoundSettings {
                muted: false,
                volume: 80,
                ambience: false
            }
        );
        assert_eq!(load(Some(&with(&[(VOLUME_KEY, "")]))).volume, 80);
        assert_eq!(load(Some(&with(&[(VOLUME_KEY, "7.5")]))).volume, 80);
        assert!(load(Some(&with(&[(MUTED_KEY, "true")]))).muted);
    }
}
