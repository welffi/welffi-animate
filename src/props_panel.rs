use crate::editor::Editor;
use crate::props::{grip, is_inside, parse_obj, unique_name, Prop, Shape};
use crate::rig::{cframe_values, is_hip, Pose, JOINT_COUNT};
use eframe::egui::{self, RichText};
use glam::Vec3;
use std::collections::BTreeSet;

#[derive(Default)]
pub struct PropsPanel {
    pub open: bool,
    editing: bool,
    renaming: Option<(usize, String)>,
}

enum Action {
    Place(usize),
    Reset(usize),
    Delete(usize),
    CopyC0(usize),
}

impl PropsPanel {
    pub fn ui(&mut self, ctx: &egui::Context, ed: &mut Editor) {
        if !self.open {
            return;
        }
        if !ctx.input(|i| i.pointer.any_down()) {
            self.editing = false;
        }
        let mut open = true;
        let mut action = None;
        egui::Window::new("Props").open(&mut open).resizable(false).default_width(320.0).show(ctx, |ui| {
            ui.label(RichText::new("In Studio, add a Motor6D. Set Part0 to the parent part, and Part1 to a part with the same name as the prop. Set its C0 with Copy C0.").weak());
            ui.horizontal(|ui| {
                if ui.button("Add block").clicked() {
                    add(ed, "Block", Shape::Block(Vec3::new(0.4, 0.4, 2.0)));
                }
                if ui.button("Add ball").clicked() {
                    add(ed, "Ball", Shape::Ball(Vec3::ONE));
                }
                if ui.button("Import .obj...").clicked() {
                    import(ed);
                }
            });
            for k in 0..ed.anim.props.len() {
                ui.separator();
                self.prop_ui(ui, ed, k, &mut action);
            }
        });
        match action {
            Some(Action::Place(k)) => {
                let pose = ed.poses()[JOINT_COUNT + k];
                ed.checkpoint();
                ed.anim.bake_rest(k, pose);
                ed.props_changed();
                ed.status = format!("{} now rests where it is", ed.anim.props[k].name);
            }
            Some(Action::Reset(k)) => {
                ed.checkpoint();
                let prop = &mut ed.anim.props[k];
                prop.rest = grip(prop.parent);
                ed.props_changed();
                ed.status = format!("Reset where {} rests", ed.anim.props[k].name);
            }
            Some(Action::Delete(k)) => {
                ed.checkpoint();
                let name = ed.anim.props[k].name.clone();
                ed.anim.remove_prop(k);
                ed.parts.clear();
                ed.active = None;
                ed.keys.clear();
                ed.props_changed();
                ed.status = format!("Deleted {name}");
            }
            Some(Action::CopyC0(k)) => {
                let prop = &ed.anim.props[k];
                ctx.copy_text(c0_text(prop.rest));
                ed.status = format!("Copied the C0 for {}", prop.name);
            }
            None => {}
        }
        self.open = open;
    }

    fn prop_ui(&mut self, ui: &mut egui::Ui, ed: &mut Editor, k: usize, action: &mut Option<Action>) {
        let joint = JOINT_COUNT + k;
        let before = ed.anim.props[k].clone();
        let mut prop = before.clone();
        ui.horizontal(|ui| {
            let mut text = match &self.renaming {
                Some((at, text)) if *at == k => text.clone(),
                _ => prop.name.clone(),
            };
            let response = ui.add(egui::TextEdit::singleline(&mut text).desired_width(150.0));
            if response.has_focus() {
                self.renaming = Some((k, text));
            } else if response.lost_focus() {
                self.renaming = None;
                prop.name = unique_name(&ed.anim.props, &text, Some(k));
            }
            ui.color_edit_button_srgb(&mut prop.color);
            ui.checkbox(&mut prop.hidden, "Hide");
            if ui.selectable_label(ed.parts.contains(&joint), "Select").clicked() {
                ed.parts = BTreeSet::from([joint]);
                ed.active = Some(joint);
            }
        });
        ui.horizontal(|ui| {
            ui.label("On");
            egui::ComboBox::from_id_salt(("prop parent", k)).selected_text(ed.rig.joints[prop.parent].part.as_str()).show_ui(ui, |ui| {
                for j in (0..ed.anim.tracks.len()).filter(|&j| !is_hip(j) && !is_inside(&ed.anim.props, j, joint)) {
                    ui.selectable_value(&mut prop.parent, j, ed.rig.joints[j].part.as_str());
                }
            });
            if let Shape::Block(size) | Shape::Ball(size) = &mut prop.shape {
                ui.label("Size");
                for value in [&mut size.x, &mut size.y, &mut size.z] {
                    ui.add(egui::DragValue::new(value).speed(0.02).range(0.05..=50.0).max_decimals(2));
                }
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Place here").on_hover_text("The prop's current pose becomes where it rests. Its keys keep their look.").clicked() {
                *action = Some(Action::Place(k));
            }
            if ui.button("Reset place").clicked() {
                *action = Some(Action::Reset(k));
            }
            if ui.button("Copy C0").on_hover_text("Copies a CFrame for the Motor6D's C0").clicked() {
                *action = Some(Action::CopyC0(k));
            }
            if ui.button("Delete").clicked() {
                *action = Some(Action::Delete(k));
            }
        });
        if prop.parent != before.parent {
            prop.rest = grip(prop.parent);
        }
        if prop != before {
            if !self.editing {
                ed.checkpoint();
            }
            self.editing = ui.input(|i| i.pointer.any_down());
            let moved = prop.parent != before.parent;
            ed.anim.props[k] = prop;
            ed.changes += 1;
            if moved {
                ed.props_changed();
            }
        }
    }
}

fn add(ed: &mut Editor, name: &str, shape: Shape) {
    ed.checkpoint();
    let name = unique_name(&ed.anim.props, name, None);
    let joint = ed.anim.add_prop(Prop::new(name.clone(), shape));
    ed.props_changed();
    ed.parts = BTreeSet::from([joint]);
    ed.active = Some(joint);
    ed.status = format!("Added {name} to the right arm");
}

fn import(ed: &mut Editor) {
    let Some(path) = rfd::FileDialog::new().add_filter("OBJ model", &["obj"]).pick_file() else { return };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            ed.status = format!("Couldn't read {}: {error}", path.display());
            return;
        }
    };
    if parse_obj(&text).is_empty() {
        ed.status = format!("{} has no faces", path.display());
        return;
    }
    let name = path.file_stem().map_or("Prop".to_string(), |s| s.to_string_lossy().to_string());
    add(ed, &name, Shape::Obj(text));
}

fn c0_text(rest: Pose) -> String {
    let values: Vec<String> = cframe_values(rest.matrix()).iter().map(|v| format!("{}", (v * 10000.0).round() / 10000.0 + 0.0)).collect();
    format!("CFrame.new({})", values.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c0_is_a_lua_cframe() {
        let rest = Pose { pos: Vec3::new(0.0, -1.0, 0.0), rot: glam::Quat::IDENTITY };
        assert_eq!(c0_text(rest), "CFrame.new(0, -1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1)");
    }
}
