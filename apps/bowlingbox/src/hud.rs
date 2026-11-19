//! The HUD drawn over and around the 3D view, as the C++ `drawHud`: the scoreboard in the top
//! band, the power bar, the hint and the result in the bottom band.

use bowlingbox_lane::Snapshot;
use bowlingbox_score::ScoreSheet;
use eframe::egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::hud_layout::hud_layout;
use crate::hud_text;
use crate::scoreboard::{self, ScoreboardLayout};

/// Behind the bands, as the C++ clear colour.
pub const CLEAR: Color32 = Color32::from_rgb(5, 8, 20);
const LINE: Color32 = Color32::from_rgb(0, 229, 255);
const PANEL: Color32 = Color32::from_rgba_premultiplied(2, 14, 56, 199);
const INK: Color32 = Color32::WHITE;
const POWER_LOW: Color32 = Color32::from_rgb(255, 138, 0);
const POWER_HIGH: Color32 = Color32::from_rgb(255, 199, 0);
const HINT_INK: Color32 = Color32::from_rgb(217, 217, 217);
const RESULT_INK: Color32 = Color32::from_rgb(255, 199, 51);

const MARGIN_SHARE: f32 = 0.02;
const MAX_MARGIN_OF_TOP: f32 = 0.2;
/// 21 roll cells, half a cell of gap and four for the total.
const BOARD_CELLS: f32 = 26.0;

/// Where everything of the HUD sits for one window size, in points.
#[derive(Clone, Copy, Debug)]
pub struct HudGeometry {
    pub top: Rect,
    pub middle: Rect,
    pub bottom: Rect,
    pub margin: f32,
    pub board: ScoreboardLayout,
    pub power_bar: Rect,
}

impl HudGeometry {
    pub fn new(area: Rect) -> Self {
        let layout = hud_layout(area.width(), area.height());
        let band = |y: f32, h: f32| {
            Rect::from_min_size(pos2(area.min.x, area.min.y + y), vec2(area.width(), h))
        };
        let top = band(0.0, layout.top);
        let middle = band(layout.top, layout.middle);
        let bottom = band(layout.top + layout.middle, layout.bottom);
        // The C++ margin is 2 % of the width; capped so a wide, low window keeps room for the cells.
        let margin = (area.width() * MARGIN_SHARE).min(top.height() * MAX_MARGIN_OF_TOP);
        let cell = ((area.width() - 2.0 * margin) / BOARD_CELLS)
            .min((top.height() - 2.0 * margin) / 2.0)
            .max(0.0);
        let board = ScoreboardLayout {
            origin_x: area.min.x + margin,
            origin_y: area.min.y + margin,
            cell,
        };
        let bar_h = bottom.height() * 0.4;
        let power_bar = Rect::from_min_size(
            pos2(area.min.x + margin, area.max.y - bottom.height() * 0.7),
            vec2(area.width() * 0.3, bar_h),
        );
        Self {
            top,
            middle,
            bottom,
            margin,
            board,
            power_bar,
        }
    }
}

/// Draws the HUD of `snapshot` into `painter`.
pub fn draw(painter: &Painter, g: &HudGeometry, snapshot: &Snapshot, sheet: &dyn ScoreSheet) {
    draw_scoreboard(&painter.with_clip_rect(g.top), g, sheet);
    draw_bottom(painter, g, snapshot, sheet);
}

fn to_egui(r: scoreboard::Rect) -> Rect {
    Rect::from_min_size(pos2(r.x, r.y), vec2(r.w, r.h))
}

fn centred(painter: &Painter, rect: Rect, text: &str, size: f32) {
    if !text.is_empty() {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            FontId::proportional(size),
            INK,
        );
    }
}

fn draw_scoreboard(painter: &Painter, g: &HudGeometry, sheet: &dyn ScoreSheet) {
    let layout = g.board;
    let cell = layout.cell;
    if cell <= 0.0 {
        return;
    }
    let thickness = (cell / 30.0).max(1.0);
    let line = Stroke::new(thickness, LINE);
    let text_size = cell * 0.5;
    let board = Rect::from_min_size(
        pos2(layout.origin_x, layout.origin_y),
        vec2(layout.total_width() + cell * 5.0, cell * 2.0),
    );
    painter.rect_filled(board, 0.0, PANEL);
    for (index, frame) in scoreboard::frames(sheet).iter().enumerate() {
        painter.rect_stroke(
            to_egui(layout.frame_rect(index)),
            0.0,
            line,
            StrokeKind::Inside,
        );
        for (c, mark) in frame.marks.iter().enumerate() {
            let roll = to_egui(layout.roll_cell_rect(2 * index + c));
            if c > 0 {
                painter.line_segment([roll.left_top(), roll.left_bottom()], line);
            }
            centred(painter, roll, mark, text_size);
        }
        let score = to_egui(layout.score_rect(index));
        painter.line_segment([score.left_top(), score.right_top()], line);
        centred(painter, score, &frame.total, text_size);
    }
    let total = Rect::from_min_size(
        pos2(
            layout.origin_x + layout.total_width() + cell * 0.5,
            layout.origin_y,
        ),
        vec2(cell * 4.0, cell * 2.0),
    );
    centred(painter, total, &scoreboard::total_text(sheet), text_size);
}

/// A text size that keeps `text` within `max_width`, starting from `size`.
fn fit(painter: &Painter, text: &str, size: f32, max_width: f32) -> f32 {
    let width = painter
        .layout_no_wrap(text.to_owned(), FontId::proportional(size), INK)
        .size()
        .x;
    if width > max_width && width > 0.0 {
        (size * max_width / width).max(1.0)
    } else {
        size
    }
}

fn draw_bottom(painter: &Painter, g: &HudGeometry, snapshot: &Snapshot, sheet: &dyn ScoreSheet) {
    let bar = g.power_bar;
    painter.rect_stroke(bar, 0.0, Stroke::new(2.0, LINE), StrokeKind::Inside);
    let power = snapshot.power.clamp(0.0, 1.0);
    let fill = Rect::from_min_size(
        bar.min + vec2(2.0, 2.0),
        vec2(
            ((bar.width() - 4.0) * power).max(0.0),
            (bar.height() - 4.0).max(0.0),
        ),
    );
    painter.rect_filled(fill, 0.0, POWER_LOW.lerp_to_gamma(POWER_HIGH, power));

    let size = (g.bottom.height() * 0.45).max(8.0);
    let mid_y = bar.center().y;
    let right = g.bottom.right() - g.margin;
    let result = hud_text::result(snapshot, sheet);
    let result_size = fit(painter, &result, size * 1.5, g.bottom.width() * 0.3);
    let result_width = painter
        .layout_no_wrap(
            result.clone(),
            FontId::proportional(result_size),
            RESULT_INK,
        )
        .size()
        .x;
    painter.text(
        pos2(right, mid_y),
        Align2::RIGHT_CENTER,
        &result,
        FontId::proportional(result_size),
        RESULT_INK,
    );
    // The hint stays centred while it fits between the bar and the result, else it shrinks.
    let centre = g.bottom.center().x;
    let room = (centre - bar.right())
        .min(right - result_width - centre)
        .max(0.0);
    let hint = hud_text::hint(snapshot.phase);
    let hint_size = fit(painter, hint, size, 2.0 * room - 2.0 * g.margin);
    painter.text(
        pos2(centre, mid_y),
        Align2::CENTER_CENTER,
        hint,
        FontId::proportional(hint_size),
        HINT_INK,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(w: f32, h: f32) -> Rect {
        Rect::from_min_size(pos2(10.0, 30.0), vec2(w, h))
    }

    fn board_rect(g: &HudGeometry) -> Rect {
        let b = g.board;
        Rect::from_min_size(
            pos2(b.origin_x, b.origin_y),
            vec2(b.cell * 26.0, b.cell * 2.0),
        )
    }

    fn check(w: f32, h: f32) {
        let a = area(w, h);
        let g = HudGeometry::new(a);
        assert!(g.board.cell > 0.0, "{w}x{h}: cell {}", g.board.cell);
        assert!(g.top.contains_rect(board_rect(&g)), "{w}x{h}: board");
        assert!(g.bottom.contains_rect(g.power_bar), "{w}x{h}: bar");
        assert!(a.contains_rect(g.middle) && g.middle.height() > 0.0);
        assert_eq!(g.top.bottom(), g.middle.top());
        assert_eq!(g.middle.bottom(), g.bottom.top());
        assert_eq!(g.bottom.bottom(), a.bottom());
    }

    #[test]
    fn the_bands_follow_the_c_plus_plus_proportions() {
        let g = HudGeometry::new(area(1000.0, 800.0));
        assert_eq!(g.top.height(), 144.0);
        assert_eq!(g.bottom.height(), 80.0);
        assert_eq!(g.middle.height(), 576.0);
        assert_eq!(g.top.min, pos2(10.0, 30.0));
    }

    #[test]
    fn a_tiny_window_still_fits_everything() {
        check(320.0, 200.0);
    }

    #[test]
    fn a_very_wide_window_still_fits_everything() {
        check(3840.0, 600.0);
    }

    #[test]
    fn ordinary_sizes_fit_and_rescale() {
        for (w, h) in [(960.0, 600.0), (1920.0, 1080.0), (400.0, 1200.0)] {
            check(w, h);
        }
        let small = HudGeometry::new(area(960.0, 600.0));
        let big = HudGeometry::new(area(1920.0, 1200.0));
        assert!(big.board.cell > small.board.cell);
    }
}
