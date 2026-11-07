use crate::editor::Editor;
use crate::markers::{marker_name, DEFAULT_NAME};
use crate::timeline::{Layout, SELECTED_KEY};
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Key, Pos2, Rect, Sense, Shape, Stroke, TextEdit, Vec2};

const FLAG: Color32 = Color32::from_rgb(90, 200, 190);
const FLAG_WIDTH: f32 = 9.0;
const FIELD_WIDTH: f32 = 110.0;

#[derive(Default)]
pub struct MarkerLane {
    selected: Option<usize>,
    drag: Option<(usize, i32)>,
    renaming: Option<(usize, String)>,
}

enum Action {
    Delete(usize),
}

impl MarkerLane {
    pub fn busy(&self) -> bool {
        self.drag.is_some() || self.renaming.is_some()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, layout: &Layout, ed: &mut Editor) {
        let lane = layout.lane();
        let painter = ui.painter_at(layout.rect);
        let strip = Rect::from_min_max(Pos2::new(layout.rect.left(), lane.top()), Pos2::new(layout.rect.right(), lane.bottom()));
        painter.rect_filled(strip, CornerRadius::ZERO, Color32::from_black_alpha(40));
        painter.text(Pos2::new(layout.rect.left() + 10.0, lane.center().y), Align2::LEFT_CENTER, "Markers", FontId::proportional(12.0), Color32::from_gray(150));

        let clip = painter.with_clip_rect(lane);
        let background = ui.interact(lane, ui.scope_id().with("marker lane"), Sense::click());
        if background.double_clicked() {
            if let Some(p) = background.interact_pointer_pos() {
                self.add(ed, layout.frame(p.x).round().max(0.0) as i32);
            }
        }
        let mut action = None;
        for i in 0..ed.anim.markers.len() {
            action = self.marker(ui, layout, ed, i, &clip).or(action);
        }
        if self.selected.is_some() && background.hovered() && self.renaming.is_none() && ui.input(|input| input.key_pressed(Key::Delete) || input.key_pressed(Key::X)) {
            action = self.selected.map(Action::Delete);
        }
        if let Some(Action::Delete(i)) = action.filter(|Action::Delete(i)| *i < ed.anim.markers.len()) {
            ed.checkpoint();
            let removed = ed.anim.markers.remove(i);
            ed.status = format!("Deleted marker \"{}\"", removed.name);
            self.selected = None;
            self.renaming = None;
        }
    }

    fn add(&mut self, ed: &mut Editor, frame: i32) {
        ed.checkpoint();
        let at = ed.anim.add_marker(frame, DEFAULT_NAME);
        self.selected = Some(at);
        self.renaming = Some((at, DEFAULT_NAME.to_string()));
        ed.status = format!("Marker added at frame {frame}. Type a name and press Enter");
    }

    fn marker(&mut self, ui: &mut egui::Ui, layout: &Layout, ed: &mut Editor, i: usize, clip: &egui::Painter) -> Option<Action> {
        let lane = layout.lane();
        let frame = match self.drag {
            Some((at, frame)) if at == i => frame,
            _ => ed.anim.markers[i].frame,
        };
        let x = layout.x(frame as f32);
        let renaming = self.renaming.as_ref().is_some_and(|(at, _)| *at == i);
        let selected = self.selected == Some(i);
        let name = ed.anim.markers[i].name.clone();
        let color = if selected { SELECTED_KEY } else { FLAG };
        let galley = clip.layout_no_wrap(name.clone(), FontId::proportional(12.0), Color32::from_gray(220));
        let width = FLAG_WIDTH + 6.0 + if renaming { FIELD_WIDTH } else { galley.size().x };
        let hit = Rect::from_min_size(Pos2::new(x - 4.0, lane.top()), Vec2::new(width + 4.0, lane.height()));
        let response = ui.interact(hit, ui.scope_id().with(("marker", i)), Sense::click_and_drag());

        let top = lane.top() + 3.0;
        clip.line_segment([Pos2::new(x, top), Pos2::new(x, lane.bottom() - 2.0)], Stroke::new(1.5, color));
        clip.add(Shape::convex_polygon(vec![Pos2::new(x, top), Pos2::new(x + FLAG_WIDTH, top + 4.0), Pos2::new(x, top + 8.0)], color, Stroke::NONE));
        if !renaming {
            clip.galley(Pos2::new(x + FLAG_WIDTH + 4.0, lane.center().y - galley.size().y / 2.0), galley, Color32::from_gray(220));
        }

        if response.clicked() || response.drag_started() {
            self.selected = Some(i);
        }
        if let (true, Some(p)) = (response.dragged(), response.interact_pointer_pos()) {
            self.drag = Some((i, layout.frame(p.x - 4.0).round().max(0.0) as i32));
        }
        if response.drag_stopped() {
            self.finish_drag(ed);
        }
        if response.double_clicked() {
            self.renaming = Some((i, name.clone()));
        }
        let mut action = None;
        response.context_menu(|ui| {
            if ui.button("Rename").clicked() {
                self.renaming = Some((i, name.clone()));
                ui.close();
            }
            if ui.button("Delete").clicked() {
                action = Some(Action::Delete(i));
                ui.close();
            }
        });
        if renaming {
            self.rename_field(ui, ed, i, Rect::from_min_size(Pos2::new(x + FLAG_WIDTH + 3.0, lane.top() + 1.0), Vec2::new(FIELD_WIDTH, lane.height() - 2.0)));
        }
        action
    }

    fn finish_drag(&mut self, ed: &mut Editor) {
        let Some((at, frame)) = self.drag.take() else { return };
        if at < ed.anim.markers.len() && ed.anim.markers[at].frame != frame {
            ed.checkpoint();
            ed.anim.markers[at].frame = frame;
            ed.status = format!("Moved marker to frame {frame}");
        }
    }

    fn rename_field(&mut self, ui: &mut egui::Ui, ed: &mut Editor, i: usize, rect: Rect) {
        let Some((_, text)) = self.renaming.as_mut() else { return };
        let field = ui.put(rect, TextEdit::singleline(text).font(FontId::proportional(12.0)));
        if !field.has_focus() && !field.lost_focus() {
            field.request_focus();
            return;
        }
        if !field.lost_focus() {
            return;
        }
        let name = marker_name(text);
        self.renaming = None;
        if ui.input(|input| input.key_pressed(Key::Escape)) || ed.anim.markers[i].name == name {
            return;
        }
        ed.checkpoint();
        ed.anim.markers[i].name = name;
    }
}
