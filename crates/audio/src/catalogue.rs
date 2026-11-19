//! The embedded sounds. Files live in `sounds/` (see `SOURCES.md` there).

use std::io::Cursor;

/// Every embedded sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SoundId {
    Roll,
    GutterDrop,
    PinHit,
    PinClatter,
    ChargeTone,
    Release,
    Strike,
    Spare,
    NewGame,
    ThemeSwitch,
    AmbientNeon,
    AmbientClassic,
    AmbientOutdoor,
}

pub const ALL_SOUNDS: [SoundId; 13] = [
    SoundId::Roll,
    SoundId::GutterDrop,
    SoundId::PinHit,
    SoundId::PinClatter,
    SoundId::ChargeTone,
    SoundId::Release,
    SoundId::Strike,
    SoundId::Spare,
    SoundId::NewGame,
    SoundId::ThemeSwitch,
    SoundId::AmbientNeon,
    SoundId::AmbientClassic,
    SoundId::AmbientOutdoor,
];

/// What the WAV header of an embedded sound says.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoundInfo {
    pub channels: u16,
    pub sample_rate: u32,
    pub seconds: f32,
}

struct Entry {
    file: &'static str,
    data: &'static [u8],
    looping: bool,
}

macro_rules! entry {
    ($file:literal, $loop:expr) => {
        Entry {
            file: $file,
            data: include_bytes!(concat!("../sounds/", $file)),
            looping: $loop,
        }
    };
}

/// Same order as [`SoundId`].
static ENTRIES: [Entry; 13] = [
    entry!("roll.wav", true),
    entry!("gutter_drop.wav", false),
    entry!("pin_hit.wav", false),
    entry!("pin_clatter.wav", false),
    entry!("charge.wav", true),
    entry!("release.wav", false),
    entry!("strike.wav", false),
    entry!("spare.wav", false),
    entry!("new_game.wav", false),
    entry!("theme_switch.wav", false),
    entry!("ambient_neon.wav", true),
    entry!("ambient_classic.wav", true),
    entry!("ambient_outdoor.wav", true),
];

fn entry(id: SoundId) -> &'static Entry {
    &ENTRIES[id as usize]
}

/// The file name, for example `"roll.wav"`.
#[must_use]
pub fn file_name(id: SoundId) -> &'static str {
    entry(id).file
}

#[must_use]
pub fn embedded_data(id: SoundId) -> &'static [u8] {
    entry(id).data
}

/// Roll, charge and the three ambiences loop; the rest play once.
#[must_use]
pub fn is_loop(id: SoundId) -> bool {
    entry(id).looping
}

/// Reads the embedded file's header without an audio device.
///
/// # Errors
/// The file name and the reason when the data is not a readable WAV file.
pub fn decode_info(id: SoundId) -> Result<SoundInfo, String> {
    let reader = hound::WavReader::new(Cursor::new(embedded_data(id)))
        .map_err(|e| format!("{}: {e}", file_name(id)))?;
    let spec = reader.spec();
    #[allow(clippy::cast_precision_loss)] // a length in seconds needs no more precision
    let seconds = reader.duration() as f32 / spec.sample_rate as f32;
    Ok(SoundInfo {
        channels: spec.channels,
        sample_rate: spec.sample_rate,
        seconds,
    })
}
