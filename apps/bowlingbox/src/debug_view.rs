//! The top-down debug view: the lane drawn from above, a scoreboard row and a power bar.

use bowlingbox_lane::{BALL_START_Z, LANE, Phase, Snapshot};
use bowlingbox_score::{ScoreSheet, frames_of, notation, total_notation};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, StrokeKind, pos2};

/// Metres of the lane shown: a little approach before the foul line, the pit behind the pins.
const VIEW_Z_MIN: f32 = -1.4;
const VIEW_Z_MAX: f32 = 18.7;
/// Length of the aim line in metres.
const AIM_LINE: f32 = 4.0;

/// Maps lane metres to screen points: x to the right, down the lane upwards.
#[derive(Clone, Copy, Debug)]
struct Projection {
    centre_x: f32,
    bottom: f32,
    scale: f32,
}

impl Projection {
    /// Fits the viewed lane section into `rect`, centred.
    fn fit(rect: Rect) -> Self {
        let half_width = LANE.half_width + LANE.gutter_width;
        let scale = (rect.height() / (VIEW_Z_MAX - VIEW_Z_MIN))
            .min(rect.width() / (2.0 * half_width))
            .max(1.0);
        Self {
            centre_x: rect.center().x,
            bottom: rect.center().y + scale * (VIEW_Z_MAX - VIEW_Z_MIN) / 2.0,
            scale,
        }
    }

    fn point(&self, x: f32, z: f32) -> Pos2 {
        pos2(
            self.centre_x + x * self.scale,
            self.bottom - (z - VIEW_Z_MIN) * self.scale,
        )
    }

    /// The rectangle between two x and two z values in metres.
    fn rect(&self, x0: f32, x1: f32, z0: f32, z1: f32) -> Rect {
        Rect::from_two_pos(self.point(x0, z0), self.point(x1, z1))
    }
}

/// The scoreboard row: ten frames with their rolls and running totals.
pub fn scoreboard(ui: &mut egui::Ui, sheet: &dyn ScoreSheet) {
    let frames = frames_of(sheet);
    ui.columns(frames.len(), |columns| {
        for (frame, ui) in frames.iter().zip(columns) {
            let rolls = frame.rolls[..frame.roll_count]
                .iter()
                .map(|slot| notation(*slot))
                .collect::<Vec<_>>()
                .join(" ");
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.vertical_centered(|ui| {
                    ui.small(format!("{}", frame.index + 1));
                    ui.monospace(if rolls.trim().is_empty() { " " } else { &rolls });
                    ui.strong(if frame.total.is_some() {
                        total_notation(frame.total)
                    } else {
                        " ".to_string()
                    });
                });
            });
        }
    });
}

/// The throw power while charging.
pub fn power_bar(ui: &mut egui::Ui, snapshot: &Snapshot) {
    ui.add(
        egui::ProgressBar::new(snapshot.power)
            .desired_width(ui.available_width())
            .text(format!("Power {:.0} %", snapshot.power * 100.0)),
    );
}

/// Draws the lane from above in the space left in `ui` and returns the area used.
pub fn lane(ui: &mut egui::Ui, snapshot: &Snapshot) -> Rect {
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    let wood = Color32::from_rgb(196, 154, 99);
    let gutter = Color32::from_gray(45);
    let ink = visuals.text_color();
    let projection = Projection::fit(rect);

    let half = LANE.half_width;
    let outer = half + LANE.gutter_width;
    painter.rect_filled(projection.rect(-half, half, 0.0, LANE.length), 0.0, wood);
    for (x0, x1) in [(-outer, -half), (half, outer)] {
        painter.rect_filled(projection.rect(x0, x1, 0.0, LANE.length), 0.0, gutter);
    }
    painter.line_segment(
        [projection.point(-outer, 0.0), projection.point(outer, 0.0)],
        Stroke::new(2.0, ink),
    );

    for i in 0..snapshot.pins.len() {
        if !snapshot.pin_in_play[i] {
            continue;
        }
        let [x, _, z] = snapshot.pins[i].position;
        let centre = projection.point(x, z);
        let radius = LANE.pin_max_radius * projection.scale;
        if snapshot.pin_standing[i] {
            painter.circle_filled(centre, radius, Color32::WHITE);
            painter.circle_stroke(centre, radius, Stroke::new(1.0, Color32::DARK_GRAY));
        } else {
            painter.circle_stroke(
                centre,
                radius,
                Stroke::new(1.5, Color32::from_rgb(190, 40, 40)),
            );
        }
    }

    let [bx, _, bz] = snapshot.ball.position;
    painter.circle_filled(
        projection.point(bx, bz),
        LANE.ball_radius * projection.scale,
        Color32::from_rgb(30, 60, 140),
    );

    if matches!(snapshot.phase, Phase::Aiming | Phase::Charging) {
        let angle = snapshot.aim_angle.to_radians();
        let from = projection.point(snapshot.aim_offset, BALL_START_Z);
        let to = projection.point(
            snapshot.aim_offset + AIM_LINE * angle.sin(),
            BALL_START_Z + AIM_LINE * angle.cos(),
        );
        painter.line_segment(
            [from, to],
            Stroke::new(2.0, Color32::from_rgb(220, 120, 20)),
        );
    }
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, Color32::from_gray(90)),
        StrokeKind::Inside,
    );
    rect
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::vec2;

    fn projection() -> Projection {
        Projection::fit(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 800.0)))
    }

    #[test]
    fn pins_sit_above_the_foul_line_and_right_is_right() {
        let p = projection();
        assert!(p.point(0.0, 17.4).y < p.point(0.0, 0.0).y);
        assert!(p.point(0.5, 5.0).x > p.point(-0.5, 5.0).x);
    }

    #[test]
    fn the_view_fits_its_rectangle() {
        let p = projection();
        let low = p.point(0.0, VIEW_Z_MIN);
        let high = p.point(0.0, VIEW_Z_MAX);
        assert!(low.y <= 800.0 && high.y >= 0.0);
    }
}
