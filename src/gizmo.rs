use crate::transform::Axis;
use eframe::egui::{self, Color32, Pos2, Stroke};
use glam::Vec3;

pub const RING_RADIUS: f32 = 80.0;
const VIEW_RING_RADIUS: f32 = 96.0;
const AXIS_COLORS: [Color32; 3] = [Color32::from_rgb(232, 64, 80), Color32::from_rgb(120, 200, 40), Color32::from_rgb(60, 130, 240)];

pub struct Gizmo {
    pub center: Vec3,
    pub screen_center: Pos2,
    pub rings: [Vec<(Pos2, bool)>; 3],
    pub local: bool,
}

impl Gizmo {
    pub fn ring_at(&self, p: Pos2) -> Option<Axis> {
        let mut best: Option<(f32, usize)> = None;
        for (i, ring) in self.rings.iter().enumerate() {
            for w in ring.windows(2) {
                if !(w[0].1 && w[1].1) {
                    continue;
                }
                let d = segment_distance(p, w[0].0, w[1].0);
                if d < 7.0 && best.is_none_or(|(b, _)| d < b) {
                    best = Some((d, i));
                }
            }
        }
        if let Some((_, i)) = best {
            return Some(if self.local { Axis::Local(i) } else { Axis::Global(i) });
        }
        let distance = p.distance(self.screen_center);
        if (distance - VIEW_RING_RADIUS).abs() < 6.0 {
            Some(Axis::View)
        } else {
            (distance < RING_RADIUS).then_some(Axis::Trackball)
        }
    }

    pub fn draw(&self, painter: egui::Painter, hovered: Option<Axis>) {
        if hovered == Some(Axis::Trackball) {
            painter.circle_filled(self.screen_center, RING_RADIUS, Color32::from_white_alpha(18));
        }
        let view_hot = hovered == Some(Axis::View);
        painter.circle_stroke(self.screen_center, VIEW_RING_RADIUS, Stroke::new(if view_hot { 3.0 } else { 1.5 }, Color32::from_gray(if view_hot { 255 } else { 200 })));
        for (i, ring) in self.rings.iter().enumerate() {
            let hot = matches!(hovered, Some(Axis::Local(j)) | Some(Axis::Global(j)) if j == i);
            let color = if hot { AXIS_COLORS[i].gamma_multiply(1.3) } else { AXIS_COLORS[i] };
            let stroke = Stroke::new(if hot { 3.5 } else { 2.0 }, color);
            for w in ring.windows(2) {
                if w[0].1 && w[1].1 {
                    painter.line_segment([w[0].0, w[1].0], stroke);
                }
            }
        }
        painter.circle_filled(self.screen_center, 2.5, Color32::WHITE);
    }
}

fn segment_distance(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 0.0 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}
