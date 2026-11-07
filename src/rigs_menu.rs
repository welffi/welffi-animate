use crate::editor::Editor;
use crate::rigs::Stand;
use crate::viewport::Viewport;
use eframe::egui::{self, DragValue, RichText};

pub fn chips(ui: &mut egui::Ui, ed: &mut Editor, viewport: &mut Viewport, busy: bool) {
    if ed.anim.partner.is_none() {
        return;
    }
    for second in [false, true] {
        let on = ed.anim.second == second;
        let name = if second { "Rig 2" } else { "Rig 1" };
        let clicked = ui.add_enabled_ui(!busy || on, |ui| ui.selectable_label(on, name)).inner.clicked();
        if clicked && !on {
            viewport.switch_rig(ed);
        }
    }
    ui.separator();
}

pub fn menu(ui: &mut egui::Ui, ed: &mut Editor, viewport: &mut Viewport) {
    ui.menu_button("Rigs", |ui| {
        if ed.anim.partner.is_none() {
            if ui.button("Add a second rig").on_hover_text("Animate two characters in one scene, like a fight").clicked() {
                ed.add_partner();
                viewport.frame_pair();
                ui.close();
            }
            return;
        }
        if ui.button("Key both rigs   Shift+I").clicked() {
            ed.key_both();
            ui.close();
        }
        ui.separator();
        ui.label(RichText::new("Where the other rig stands").weak());
        placement(ui, ed);
        ui.separator();
        if ui.button("Remove the other rig").clicked() {
            ed.remove_partner();
            ui.close();
        }
    });
}

fn placement(ui: &mut egui::Ui, ed: &mut Editor) {
    let stand = ed.anim.at;
    let (mut side, mut front, mut turn) = (stand.x, -stand.z, stand.turn.to_degrees());
    let (mut started, mut changed) = (false, false);
    let mut field = |ui: &mut egui::Ui, label: &str, value: &mut f32, suffix: &str| {
        ui.horizontal(|ui| {
            ui.label(label);
            let response = ui.add(DragValue::new(value).speed(0.05).suffix(suffix));
            started |= response.drag_started() || response.gained_focus();
            changed |= response.changed();
        });
    };
    field(ui, "Side", &mut side, "");
    field(ui, "In front", &mut front, "");
    field(ui, "Turn", &mut turn, "°");
    if started {
        ed.checkpoint();
    }
    if changed {
        ed.anim.at = Stand { x: side, z: -front, turn: turn.to_radians() };
    }
}
