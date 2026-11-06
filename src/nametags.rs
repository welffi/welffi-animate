use crate::editor::Editor;
use crate::timeline::{Layout, KEY_RADIUS};
use eframe::egui::{self, Color32, CornerRadius, FontId, Pos2, Rect, Vec2};
use std::collections::BTreeMap;

const REACH: f32 = 80.0;
const FONT_SIZE: f32 = 12.0;
const GAP: f32 = 4.0;
const PADDING: Vec2 = Vec2::new(5.0, 2.0);

pub struct Tag {
    pub joint: usize,
    pub at: Pos2,
    pub strength: f32,
}

pub fn nearest_per_row(keys: &[(usize, Pos2)], pointer: Pos2) -> Vec<Tag> {
    let mut nearest: BTreeMap<usize, (f32, Pos2)> = BTreeMap::new();
    for &(joint, at) in keys {
        let distance = at.distance(pointer);
        if distance <= REACH && nearest.get(&joint).is_none_or(|&(best, _)| distance < best) {
            nearest.insert(joint, (distance, at));
        }
    }
    nearest.into_iter().map(|(joint, (distance, at))| Tag { joint, at, strength: 1.0 - distance / REACH }).collect()
}

pub fn paint(ui: &egui::Ui, layout: &Layout, ed: &Editor, pointer: Pos2) {
    let painter = ui.painter_at(layout.body());
    let keys: Vec<(usize, Pos2)> = ed
        .anim
        .tracks
        .iter()
        .enumerate()
        .flat_map(|(joint, track)| {
            let y = layout.row_y(joint + 1);
            track.keys.iter().map(move |key| (joint, Pos2::new(layout.x(key.frame as f32), y)))
        })
        .collect();
    for tag in nearest_per_row(&keys, pointer) {
        let alpha = ((tag.strength * 1.6).min(1.0) * 255.0) as u8;
        let galley = painter.layout_no_wrap(ed.rig.joints[tag.joint].part.to_string(), FontId::proportional(FONT_SIZE), Color32::from_white_alpha(alpha));
        let size = galley.size() + PADDING * 2.0;
        let offset = KEY_RADIUS + GAP;
        let mut left = tag.at.x + offset;
        if left + size.x > layout.rect.right() {
            left = tag.at.x - offset - size.x;
        }
        let rect = Rect::from_min_size(Pos2::new(left, tag.at.y - size.y / 2.0), size);
        painter.rect_filled(rect, CornerRadius::same(4), Color32::from_black_alpha((alpha as f32 * 0.85) as u8));
        painter.galley(rect.min + PADDING, galley, Color32::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_nearest_key_in_each_row() {
        let keys = [(0, Pos2::new(100.0, 10.0)), (0, Pos2::new(130.0, 10.0)), (1, Pos2::new(120.0, 32.0))];
        let tags = nearest_per_row(&keys, Pos2::new(126.0, 10.0));
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].joint, 0);
        assert_eq!(tags[0].at, Pos2::new(130.0, 10.0));
        assert_eq!(tags[1].joint, 1);
    }

    #[test]
    fn ignores_keys_out_of_reach() {
        let keys = [(0, Pos2::new(0.0, 0.0)), (1, Pos2::new(REACH, 0.0))];
        let tags = nearest_per_row(&keys, Pos2::new(REACH + 1.0, 0.0));
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].joint, 1);
        assert!(tags[0].strength > 0.98);
    }
}
