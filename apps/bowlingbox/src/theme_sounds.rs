//! The ambience loop of each theme: the one place where render themes meet audio sounds.

use bowlingbox_audio::SoundId;
use bowlingbox_render::ThemeId;

pub fn ambient_for(theme: ThemeId) -> SoundId {
    match theme {
        ThemeId::NeonHall => SoundId::AmbientNeon,
        ThemeId::ClassicAlley => SoundId::AmbientClassic,
        ThemeId::Outdoor => SoundId::AmbientOutdoor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_theme_has_its_own_ambience() {
        assert_eq!(ambient_for(ThemeId::NeonHall), SoundId::AmbientNeon);
        assert_eq!(ambient_for(ThemeId::ClassicAlley), SoundId::AmbientClassic);
        assert_eq!(ambient_for(ThemeId::Outdoor), SoundId::AmbientOutdoor);
    }
}
