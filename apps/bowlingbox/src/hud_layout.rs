//! Screen bands of the HUD: scoreboard on top, power bar, hint and result at the bottom.

#![allow(
    dead_code,
    reason = "the HUD drawing that uses it comes in the next task"
)]

/// Share of the height for the scoreboard.
const TOP_SHARE: f32 = 0.18;
/// Share of the height for the power bar, hint and result.
const BOTTOM_SHARE: f32 = 0.10;

/// Band heights in points; the 3D view gets `middle`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudLayout {
    pub top: f32,
    pub bottom: f32,
    pub middle: f32,
}

/// The bands for a `width` x `height` viewport: 18 % on top, 10 % at the bottom. The width does
/// not change the bands; it is part of the signature so callers pass the whole viewport.
pub fn hud_layout(_width: f32, height: f32) -> HudLayout {
    let top = (height * TOP_SHARE).floor();
    let bottom = (height * BOTTOM_SHARE).floor();
    HudLayout {
        top,
        bottom,
        middle: height - top - bottom,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_reserves_top_and_bottom_bands() {
        let l = hud_layout(1000.0, 800.0);
        assert_eq!(l.top, 144.0);
        assert_eq!(l.bottom, 80.0);
        assert_eq!(l.middle, 576.0);
    }

    #[test]
    fn the_bands_rescale_with_the_height() {
        let l = hud_layout(1920.0, 1080.0);
        assert_eq!((l.top, l.bottom, l.middle), (194.0, 108.0, 778.0));
        let small = hud_layout(320.0, 200.0);
        assert_eq!((small.top, small.bottom, small.middle), (36.0, 20.0, 144.0));
    }
}
