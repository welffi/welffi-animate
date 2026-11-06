use crate::curve::Handle;
use crate::easing::{Ease, Interp};
use crate::editor::Editor;
use eframe::egui::{self, Key, Pos2, RichText};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MenuKind {
    Interp,
    Handle,
}

pub struct Menu {
    pub kind: MenuKind,
    pub at: Pos2,
}

pub fn show(ui: &egui::Ui, ed: &mut Editor, menu: &mut Option<Menu>) {
    let Some(open) = menu.as_ref() else { return };
    if ed.keys.is_empty() {
        *menu = None;
        return;
    }
    let kind = open.kind;
    let mut chosen = false;
    let area = egui::Area::new(egui::Id::unique("key menu")).order(egui::Order::Foreground).fixed_pos(open.at).show(ui.ctx(), |ui| {
        egui::Frame::menu(ui.style()).show(ui, |ui| {
            ui.set_min_width(150.0);
            chosen = match kind {
                MenuKind::Interp => interp_items(ui, ed),
                MenuKind::Handle => handle_items(ui, ed),
            };
        });
    });
    let clicked_outside = ui.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer();
    if chosen || clicked_outside || ui.input(|i| i.key_pressed(Key::Escape)) {
        *menu = None;
    }
}

fn interp_items(ui: &mut egui::Ui, ed: &mut Editor) -> bool {
    let mut chosen = false;
    ui.label(RichText::new("Interpolation").weak());
    for interp in Interp::ALL {
        if ui.button(interp.name()).clicked() {
            ed.checkpoint();
            ed.anim.set_interp(&ed.keys, interp);
            chosen = true;
        }
    }
    ui.separator();
    ui.label(RichText::new("Direction (Cubic, Bounce, Elastic)").weak());
    for ease in Ease::ALL {
        if ui.button(ease.name()).clicked() {
            ed.checkpoint();
            ed.anim.set_ease(&ed.keys, ease);
            chosen = true;
        }
    }
    chosen
}

fn handle_items(ui: &mut egui::Ui, ed: &mut Editor) -> bool {
    let mut chosen = false;
    let current: Vec<Handle> = ed.keys.iter().filter_map(|&(track, frame)| ed.anim.tracks[track].key_at(frame)).map(|k| k.handle).collect();
    ui.label(RichText::new("Handle type").weak());
    for handle in Handle::ALL {
        let all_this = current.iter().all(|&h| h == handle);
        if ui.selectable_label(all_this, handle.name()).clicked() {
            ed.checkpoint();
            ed.anim.set_handle(&ed.keys, handle);
            ed.status = format!("Set {} keys to {}", ed.keys.len(), handle.name());
            chosen = true;
        }
    }
    chosen
}
