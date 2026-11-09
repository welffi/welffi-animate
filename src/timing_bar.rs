use crate::editor::Editor;
use eframe::egui::{self, Button, DragValue};

pub fn ui(ui: &mut egui::Ui, ed: &mut Editor) {
    let (mut fps, mut end) = (ed.anim.fps, ed.anim.end);
    let end_changed = ui.add(DragValue::new(&mut end).range(1..=100_000)).changed();
    ui.label("End");
    let fps_changed = ui.add(DragValue::new(&mut fps).range(1..=240)).changed();
    ui.label("FPS");
    if fps_changed || end_changed {
        ed.checkpoint();
        ed.anim.set_timing(fps, end);
    }
    ui.separator();
    let auto_key = Button::new("Auto key").selected(ed.auto_key);
    if ui.add(auto_key).on_hover_text("When on, posing a part keys it at the current frame").clicked() {
        ed.auto_key = !ed.auto_key;
    }
}
