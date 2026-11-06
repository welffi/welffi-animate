use eframe::egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};

const HINTS: [(&str, &str); 11] = [
    ("R", "Rotate"),
    ("G", "Move"),
    ("I", "Key"),
    ("Space", "Play"),
    ("T", "Easing"),
    ("V", "Handles"),
    ("Shift+E", "Loop / extend"),
    ("B", "Build-up"),
    ("F", "Body follow"),
    ("Ctrl+Tab", "Graph"),
    ("Ctrl+Shift+C", "Copy for Roblox"),
];
const MARGIN: f32 = 10.0;
const ROW_HEIGHT: f32 = 20.0;
const CAP_HEIGHT: f32 = 17.0;
const CAP_PADDING: f32 = 10.0;
const LABEL_GAP: f32 = 8.0;

pub fn paint(painter: &Painter, area: Rect) {
    let cap_font = FontId::monospace(11.0);
    let cap_width = HINTS.iter().map(|(key, _)| painter.layout_no_wrap(key.to_string(), cap_font.clone(), Color32::WHITE).size().x).fold(0.0, f32::max) + CAP_PADDING;
    let top = area.bottom() - MARGIN - HINTS.len() as f32 * ROW_HEIGHT;
    for (row, (key, label)) in HINTS.iter().enumerate() {
        let y = top + (row as f32 + 0.5) * ROW_HEIGHT;
        let cap = Rect::from_center_size(Pos2::new(area.left() + MARGIN + cap_width / 2.0, y), Vec2::new(cap_width, CAP_HEIGHT));
        painter.rect_filled(cap, CornerRadius::same(4), Color32::from_black_alpha(90));
        painter.rect_stroke(cap, CornerRadius::same(4), Stroke::new(1.0, Color32::from_white_alpha(40)), StrokeKind::Inside);
        painter.text(cap.center(), Align2::CENTER_CENTER, key, cap_font.clone(), Color32::from_gray(200));
        painter.text(Pos2::new(cap.right() + LABEL_GAP, y), Align2::LEFT_CENTER, label, FontId::proportional(12.0), Color32::from_gray(150));
    }
}
