use crate::anim::{Animation, PointId, Track, CHANNELS};
use crate::curve::Side;
use crate::easing::Interp;
use crate::editor::Editor;
use crate::timeline::{Layout, ROW_HEIGHT, SELECTED_KEY};
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, PointerButton, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};
use std::collections::BTreeSet;

const NAMES: [&str; CHANNELS] = ["Position X", "Position Y", "Position Z", "Rotation W", "Rotation X", "Rotation Y", "Rotation Z"];
const COLORS: [Color32; CHANNELS] = [
    Color32::from_rgb(235, 85, 85),
    Color32::from_rgb(130, 205, 80),
    Color32::from_rgb(85, 145, 245),
    Color32::from_rgb(205, 205, 205),
    Color32::from_rgb(245, 150, 130),
    Color32::from_rgb(180, 230, 140),
    Color32::from_rgb(140, 185, 250),
];
const POINT_SIZE: f32 = 7.0;
const HANDLE_RADIUS: f32 = 3.5;
const GRAB: f32 = 7.0;
const PAD: f32 = 16.0;

enum Drag {
    Points { from: Pos2, base: Animation, picked: BTreeSet<PointId> },
    Handle { point: PointId, side: Side, base: Animation },
    Box { from: Pos2 },
}

#[derive(Default)]
pub struct Graph {
    hidden: [bool; CHANNELS],
    points: BTreeSet<PointId>,
    drag: Option<Drag>,
    range: Option<(f32, f32)>,
}

struct Axis {
    low: f32,
    high: f32,
    top: f32,
    bottom: f32,
}

impl Axis {
    fn y(&self, value: f32) -> f32 {
        self.bottom - (value - self.low) / (self.high - self.low) * (self.bottom - self.top)
    }

    fn per_pixel(&self) -> f32 {
        (self.high - self.low) / (self.bottom - self.top)
    }
}

struct View<'a> {
    layout: &'a Layout,
    axis: Axis,
}

impl View<'_> {
    fn pos(&self, frame: f32, value: f32) -> Pos2 {
        Pos2::new(self.layout.x(frame), self.axis.y(value))
    }

    fn point(&self, track: &Track, (_, channel, frame): PointId) -> Option<Pos2> {
        track.key_at(frame).map(|k| self.pos(frame as f32, k.pose.channels()[channel]))
    }

    fn handle_ends(&self, track: &Track, (_, channel, frame): PointId) -> Vec<(Side, Pos2)> {
        let Some(i) = track.keys.iter().position(|k| k.frame == frame) else { return Vec::new() };
        let key = &track.keys[i];
        let origin = self.pos(frame as f32, key.pose.channels()[channel]);
        let to = |offset: egui::Vec2| origin + Vec2::new(offset.x * self.layout.pixels_per_frame, -offset.y / self.axis.per_pixel());
        let mut ends = Vec::new();
        if i > 0 && track.keys[i - 1].interp == Interp::Bezier {
            ends.push((Side::Left, to(Vec2::new(key.left[channel].x, key.left[channel].y))));
        }
        if i + 1 < track.keys.len() && key.interp == Interp::Bezier {
            ends.push((Side::Right, to(Vec2::new(key.right[channel].x, key.right[channel].y))));
        }
        ends
    }
}

impl Graph {
    pub fn busy(&self) -> bool {
        self.drag.is_some()
    }

    pub fn clear(&mut self) {
        self.points.clear();
    }

    fn curves(&self, ed: &Editor) -> Vec<(usize, usize)> {
        ed.parts.iter().filter(|&&j| !ed.anim.tracks[j].keys.is_empty()).flat_map(|&j| (0..CHANNELS).filter(|&c| !self.hidden[c]).map(move |c| (j, c))).collect()
    }

    fn all_points(&self, ed: &Editor) -> BTreeSet<PointId> {
        self.curves(ed).into_iter().flat_map(|(j, c)| ed.anim.tracks[j].keys.iter().map(move |k| (j, c, k.frame))).collect()
    }

    pub fn select_all(&mut self, ed: &mut Editor) {
        self.points = self.all_points(ed);
        self.publish(ed);
    }

    fn publish(&self, ed: &mut Editor) {
        ed.keys = self.points.iter().map(|&(track, _, frame)| (track, frame)).collect();
    }

    fn follow(&mut self, ed: &Editor) {
        let all = self.all_points(ed);
        self.points.retain(|p| all.contains(p) && ed.keys.contains(&(p.0, p.2)));
        for &(track, frame) in &ed.keys {
            if !self.points.iter().any(|p| p.0 == track && p.2 == frame) {
                self.points.extend(all.iter().filter(|p| p.0 == track && p.2 == frame));
            }
        }
    }

    fn fit(&self, ed: &Editor) -> (f32, f32) {
        if let Some(range) = self.range {
            return range;
        }
        let (mut low, mut high) = (f32::MAX, f32::MIN);
        for (j, c) in self.curves(ed) {
            let track = &ed.anim.tracks[j];
            let (first, last) = (track.keys[0].frame, track.keys[track.keys.len() - 1].frame);
            let step = ((last - first) / 400).max(1);
            for frame in (first..=last).step_by(step as usize).chain([last]) {
                let value = track.channel(c, frame as f32);
                low = low.min(value);
                high = high.max(value);
            }
        }
        if low > high {
            return (-1.0, 1.0);
        }
        let middle = (low + high) / 2.0;
        let half = ((high - low) / 2.0).max(0.05);
        (middle - half, middle + half)
    }

    pub fn ui(&mut self, ui: &egui::Ui, response: &egui::Response, layout: &Layout, ed: &mut Editor) {
        self.follow(ed);
        let (low, high) = self.fit(ed);
        let view = View { layout, axis: Axis { low, high, top: layout.body_top() + PAD, bottom: layout.rect.bottom() - PAD } };
        self.mouse(ui, response, &view, ed);
        self.paint(ui, &view, ed);
    }

    fn handle_at(&self, view: &View, ed: &Editor, p: Pos2) -> Option<(PointId, Side)> {
        self.points.iter().find_map(|&id| view.handle_ends(&ed.anim.tracks[id.0], id).into_iter().find(|(_, end)| end.distance(p) <= GRAB).map(|(side, _)| (id, side)))
    }

    fn point_at(&self, view: &View, ed: &Editor, p: Pos2) -> Option<PointId> {
        self.all_points(ed)
            .into_iter()
            .filter_map(|id| view.point(&ed.anim.tracks[id.0], id).map(|at| (id, at.distance(p))))
            .filter(|(_, distance)| *distance <= GRAB)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    fn mouse(&mut self, ui: &egui::Ui, response: &egui::Response, view: &View, ed: &mut Editor) {
        let shift = ui.input(|i| i.modifiers.shift);
        let body = view.layout.body();
        if response.drag_started_by(PointerButton::Primary) {
            if let Some(origin) = ui.input(|i| i.pointer.press_origin()).filter(|p| body.contains(*p)) {
                self.range = Some((view.axis.low, view.axis.high));
                self.drag = Some(if let Some((point, side)) = self.handle_at(view, ed, origin) {
                    Drag::Handle { point, side, base: ed.anim.clone() }
                } else if let Some(id) = self.point_at(view, ed, origin) {
                    if !self.points.contains(&id) {
                        if !shift {
                            self.points.clear();
                        }
                        self.points.insert(id);
                        self.publish(ed);
                    }
                    Drag::Points { from: origin, base: ed.anim.clone(), picked: self.points.clone() }
                } else {
                    Drag::Box { from: origin }
                });
            }
        }
        if let Some(p) = response.interact_pointer_pos() {
            match &self.drag {
                Some(Drag::Points { from, base, picked }) => {
                    let mut moved = p - *from;
                    if shift {
                        if moved.x.abs() > moved.y.abs() {
                            moved.y = 0.0;
                        } else {
                            moved.x = 0.0;
                        }
                    }
                    ed.anim = base.clone();
                    let frames = (moved.x / view.layout.pixels_per_frame).round() as i32;
                    self.points = ed.anim.nudge(picked, frames, -moved.y * view.axis.per_pixel());
                    self.publish(ed);
                }
                Some(Drag::Handle { point: (track, channel, frame), side, base }) => {
                    ed.anim = base.clone();
                    let Some(origin) = view.point(&ed.anim.tracks[*track], (*track, *channel, *frame)) else { return };
                    let offset = p - origin;
                    let units = egui::Vec2::new(offset.x / view.layout.pixels_per_frame, -offset.y * view.axis.per_pixel());
                    ed.anim.tracks[*track].drag_handle(*channel, *frame, *side, glam::Vec2::new(units.x, units.y));
                }
                _ => {}
            }
        }
        if response.drag_stopped() {
            match self.drag.take() {
                Some(Drag::Points { base, .. } | Drag::Handle { base, .. }) => commit(ed, base),
                Some(Drag::Box { from }) => {
                    if let Some(to) = response.interact_pointer_pos().or(ui.input(|i| i.pointer.hover_pos())) {
                        let area = Rect::from_two_pos(from, to);
                        let found: Vec<PointId> = self.all_points(ed).into_iter().filter(|&id| view.point(&ed.anim.tracks[id.0], id).is_some_and(|at| area.contains(at))).collect();
                        if !shift {
                            self.points.clear();
                        }
                        self.points.extend(found);
                        self.publish(ed);
                    }
                }
                None => {}
            }
            self.range = None;
        }
        self.clicks(response, view, ed, shift);
    }

    fn clicks(&mut self, response: &egui::Response, view: &View, ed: &mut Editor, shift: bool) {
        let Some(p) = response.interact_pointer_pos() else { return };
        if response.clicked() && p.x < view.layout.keys_left {
            if let Some(channel) = self.channel_row(view.layout, p.y) {
                self.hidden[channel] = !self.hidden[channel];
            }
            return;
        }
        let body = view.layout.body();
        if !body.contains(p) || self.handle_at(view, ed, p).is_some() {
            return;
        }
        let hit = self.point_at(view, ed, p);
        if response.clicked() {
            match hit {
                Some(id) if shift => {
                    if !self.points.remove(&id) {
                        self.points.insert(id);
                    }
                }
                Some(id) => self.points = [id].into(),
                None if !shift => self.points.clear(),
                None => {}
            }
            self.publish(ed);
        }
        if response.secondary_clicked() {
            if let Some(id) = hit.filter(|id| !self.points.contains(id)) {
                self.points = [id].into();
                self.publish(ed);
            }
        }
    }

    fn channel_row(&self, layout: &Layout, y: f32) -> Option<usize> {
        let row = ((y - layout.body_top()) / ROW_HEIGHT).floor();
        (row >= 0.0 && row < CHANNELS as f32).then_some(row as usize)
    }

    fn paint(&self, ui: &egui::Ui, view: &View, ed: &Editor) {
        let layout = view.layout;
        let painter = ui.painter_at(layout.rect);
        for (c, name) in NAMES.iter().enumerate() {
            let y = layout.body_top() + c as f32 * ROW_HEIGHT + ROW_HEIGHT / 2.0;
            let color = if self.hidden[c] { Color32::from_gray(80) } else { COLORS[c] };
            let swatch = Rect::from_center_size(Pos2::new(layout.rect.left() + 14.0, y), Vec2::splat(8.0));
            if self.hidden[c] {
                painter.rect_stroke(swatch, CornerRadius::same(2), Stroke::new(1.0, color), StrokeKind::Inside);
            } else {
                painter.rect_filled(swatch, CornerRadius::same(2), color);
            }
            let text = if self.hidden[c] { Color32::from_gray(100) } else { ui.visuals().text_color() };
            painter.text(Pos2::new(layout.rect.left() + 26.0, y), Align2::LEFT_CENTER, *name, FontId::proportional(12.5), text);
        }

        let body = painter.with_clip_rect(layout.body());
        let step = value_step(view.axis.per_pixel() * 32.0);
        let mut value = (view.axis.low / step).floor() * step;
        while value <= view.axis.high + step {
            let y = view.axis.y(value);
            let stroke = if value.abs() < step / 2.0 { Stroke::new(1.0, Color32::from_white_alpha(28)) } else { Stroke::new(1.0, Color32::from_white_alpha(10)) };
            body.line_segment([Pos2::new(layout.keys_left, y), Pos2::new(layout.rect.right(), y)], stroke);
            body.text(Pos2::new(layout.keys_left + 4.0, y - 1.0), Align2::LEFT_BOTTOM, format_value(value, step), FontId::monospace(10.0), Color32::from_gray(120));
            value += step;
        }

        let curves = self.curves(ed);
        if curves.is_empty() {
            let message = if ed.parts.is_empty() { "Select a part to see its curves" } else { "No keys on the selected parts yet" };
            body.text(layout.body().center(), Align2::CENTER_CENTER, message, FontId::proportional(13.0), Color32::from_gray(120));
        }
        for &(j, c) in &curves {
            let track = &ed.anim.tracks[j];
            let (first, last) = (track.keys[0].frame as f32, track.keys[track.keys.len() - 1].frame as f32);
            let from = first.max(layout.frame(layout.keys_left));
            let to = last.min(layout.frame(layout.rect.right()));
            let step = 2.0 / layout.pixels_per_frame;
            let mut line = Vec::new();
            let mut frame = from;
            while frame < to {
                line.push(view.pos(frame, track.channel(c, frame)));
                frame += step;
            }
            if from <= to {
                line.push(view.pos(to, track.channel(c, to)));
            }
            body.add(Shape::line(line, Stroke::new(1.6, COLORS[c])));
        }

        for &id in &self.points {
            let track = &ed.anim.tracks[id.0];
            let Some(origin) = view.point(track, id) else { continue };
            for (_, end) in view.handle_ends(track, id) {
                body.line_segment([origin, end], Stroke::new(1.0, Color32::from_gray(190)));
                body.circle(end, HANDLE_RADIUS, Color32::from_gray(230), Stroke::new(1.0, Color32::from_gray(20)));
            }
        }
        for &(j, c) in &curves {
            for key in &ed.anim.tracks[j].keys {
                let id = (j, c, key.frame);
                let at = view.pos(key.frame as f32, key.pose.channels()[c]);
                let square = Rect::from_center_size(at, Vec2::splat(POINT_SIZE));
                if self.points.contains(&id) {
                    body.rect_filled(square, CornerRadius::ZERO, SELECTED_KEY);
                    body.rect_stroke(square, CornerRadius::ZERO, Stroke::new(1.0, Color32::from_gray(15)), StrokeKind::Outside);
                } else {
                    body.rect_filled(square, CornerRadius::ZERO, Color32::from_gray(28));
                    body.rect_stroke(square, CornerRadius::ZERO, Stroke::new(1.5, COLORS[c]), StrokeKind::Inside);
                }
            }
        }

        if let (Some(Drag::Box { from }), Some(to)) = (&self.drag, ui.input(|i| i.pointer.hover_pos())) {
            let r = Rect::from_two_pos(*from, to);
            body.rect_filled(r, CornerRadius::ZERO, Color32::from_white_alpha(12));
            body.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, Color32::from_gray(200)), StrokeKind::Inside);
        }
    }
}

fn commit(ed: &mut Editor, base: Animation) {
    if ed.anim != base {
        let edited = std::mem::replace(&mut ed.anim, base);
        ed.checkpoint();
        ed.anim = edited;
    }
}

fn value_step(at_least: f32) -> f32 {
    let power = 10f32.powf(at_least.log10().floor());
    [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * power).find(|s| *s >= at_least).unwrap_or(10.0 * power)
}

fn format_value(value: f32, step: f32) -> String {
    let decimals = (-step.log10().floor()).max(0.0) as usize;
    format!("{:.*}", decimals, if value.abs() < step / 2.0 { 0.0 } else { value })
}
