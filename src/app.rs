use crate::anim::Animation;
use crate::editor::Editor;
use crate::export::roblox_clipboard;
use crate::rigs_menu;
use crate::panels::{Panels, Tool};
use crate::curve::Handle;
use crate::library_view::LibraryView;
use crate::project::{load_animation, load_session, load_settings, save_animation, save_session, save_settings, Session, Settings};
use crate::props_panel::PropsPanel;
use crate::render::load_parts;
use crate::timeline::Timeline;
use crate::viewport::Viewport;
use eframe::egui::{self, Event, Key, Modifiers, RichText};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const AUTOSAVE_EVERY: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Animate,
    Library,
}

pub struct App {
    ed: Editor,
    path: Option<PathBuf>,
    page: Page,
    library: LibraryView,
    viewport: Viewport,
    timeline: Timeline,
    panels: Panels,
    props: PropsPanel,
    autosaved: u64,
    file_saved: u64,
    last_autosave: Instant,
    title: String,
}

impl App {
    pub fn new(cc: &eframe::CreationContext) -> App {
        let rs = cc.wgpu_render_state.as_ref().expect("welffi animate needs the wgpu renderer");
        let meshes = load_parts(include_bytes!("../assets/r6.bin"));
        let viewport = Viewport::new(rs, meshes, include_bytes!("../assets/r6.png"));
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let session = load_session();
        let mut ed = Editor::new(session.anim);
        let settings = load_settings();
        ed.new_handle = settings.new_handle;
        ed.key_names = settings.key_names;
        App {
            ed,
            path: session.path,
            page: Page::Animate,
            library: LibraryView::new(rs),
            viewport,
            timeline: Timeline::default(),
            panels: Panels::default(),
            props: PropsPanel::default(),
            autosaved: 0,
            file_saved: 0,
            last_autosave: Instant::now(),
            title: String::new(),
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.selectable_value(&mut self.page, Page::Animate, "Animate");
            ui.selectable_value(&mut self.page, Page::Library, "Library");
            ui.separator();
            ui.menu_button("File", |ui| {
                if ui.button("New            Ctrl+N").clicked() {
                    self.new_file();
                }
                if ui.button("Open...        Ctrl+O").clicked() {
                    self.open();
                }
                if ui.button("Save           Ctrl+S").clicked() {
                    self.save(false);
                }
                if ui.button("Save As...     Ctrl+Shift+S").clicked() {
                    self.save(true);
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui.button("Undo           Ctrl+Z").clicked() {
                    self.ed.undo();
                }
                if ui.button("Redo           Ctrl+Y").clicked() {
                    self.ed.redo();
                }
                ui.separator();
                if ui.button("Insert key     I").clicked() {
                    self.ed.insert_keys();
                }
                if ui.button("Copy pose      Ctrl+C").clicked() {
                    self.ed.copy_pose();
                }
                if ui.button("Paste pose     Ctrl+V").clicked() {
                    self.ed.paste_pose(false);
                }
                if ui.button("Paste mirrored Ctrl+Shift+V").clicked() {
                    self.ed.paste_pose(true);
                }
                if ui.button("Build-up and overshoot   B").clicked() {
                    self.panels.open(Tool::Moves, ui.ctx(), &mut self.ed);
                }
                if ui.button("Body follow   F").clicked() {
                    self.panels.open(Tool::Body, ui.ctx(), &mut self.ed);
                }
                if ui.button("Low FPS look").clicked() {
                    self.panels.open(Tool::Steps, ui.ctx(), &mut self.ed);
                }
                ui.separator();
                ui.label(RichText::new("New key handles").weak());
                for handle in Handle::ALL.into_iter().filter(|h| h.computed()) {
                    if ui.radio(self.ed.new_handle == handle, handle.name()).clicked() {
                        self.set_new_handle(handle);
                    }
                }
                ui.separator();
                let mut names = self.ed.key_names;
                if ui.checkbox(&mut names, "Part names near keys").changed() {
                    self.set_key_names(names);
                }
            });
            if ui.selectable_label(self.props.open, "Props").on_hover_text("Add items the character holds, like a sword").clicked() {
                self.props.open = !self.props.open;
            }
            rigs_menu::menu(ui, &mut self.ed, &mut self.viewport);
            ui.separator();
            let busy = self.viewport.busy() || self.timeline.busy() || self.panels.busy();
            rigs_menu::chips(ui, &mut self.ed, &mut self.viewport, busy);
            let copy = RichText::new("Copy for Roblox").strong();
            if self.ed.anim.partner.is_some() {
                ui.menu_button(copy, |ui| {
                    for second in [false, true] {
                        if ui.button(format!("Rig {}", second as u8 + 1)).clicked() {
                            self.export(ui.ctx(), second);
                            ui.close();
                        }
                    }
                });
            } else if ui.button(copy).on_hover_text("Ctrl+Shift+C. Paste the text into the Roblox animation importer plugin.").clicked() {
                self.export(ui.ctx(), self.ed.anim.second);
            }
            ui.separator();
            let auto = RichText::new("Auto key").color(if self.ed.auto_key { egui::Color32::from_rgb(230, 70, 70) } else { ui.visuals().weak_text_color() });
            if ui.selectable_label(self.ed.auto_key, auto).on_hover_text("When on, posing a part keys it at the current frame").clicked() {
                self.ed.auto_key = !self.ed.auto_key;
            }
            ui.separator();
            self.play_controls(ui);
            ui.separator();
            let (mut fps, mut end) = (self.ed.anim.fps, self.ed.anim.end);
            ui.label("FPS");
            let fps_changed = ui.add(egui::DragValue::new(&mut fps).range(1..=240)).changed();
            ui.label("End");
            let end_changed = ui.add(egui::DragValue::new(&mut end).range(1..=100_000)).changed();
            if fps_changed || end_changed {
                self.ed.checkpoint();
                self.ed.anim.set_timing(fps, end);
            }
        });
    }

    fn play_controls(&mut self, ui: &mut egui::Ui) {
        if ui.button("|<").on_hover_text("Start  (Shift+Left)").clicked() {
            self.ed.set_frame(0.0);
        }
        if ui.button("<").on_hover_text("Previous key  (Down)").clicked() {
            self.ed.jump_key(false);
        }
        let play = if self.ed.playing { "Pause" } else { "Play" };
        if ui.button(play).on_hover_text("Play  (Space)").clicked() {
            self.toggle_play();
        }
        if ui.button(">").on_hover_text("Next key  (Up)").clicked() {
            self.ed.jump_key(true);
        }
        if ui.button(">|").on_hover_text("End  (Shift+Right)").clicked() {
            self.ed.set_frame(self.ed.anim.end as f32);
        }
    }

    fn set_new_handle(&mut self, handle: Handle) {
        self.ed.new_handle = handle;
        self.save_settings(format!("New keys get {} handles", handle.name()));
    }

    fn set_key_names(&mut self, on: bool) {
        self.ed.key_names = on;
        self.save_settings(format!("Part names near keys {}", if on { "on" } else { "off" }));
    }

    fn save_settings(&mut self, done: String) {
        let settings = Settings { new_handle: self.ed.new_handle, key_names: self.ed.key_names };
        self.ed.status = match save_settings(&settings) {
            Ok(()) => done,
            Err(error) => format!("Couldn't save settings: {error}"),
        };
    }

    fn toggle_play(&mut self) {
        self.ed.playing = !self.ed.playing;
        if !self.ed.playing {
            self.ed.set_frame(self.ed.frame.round());
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() || self.viewport.busy() || self.page != Page::Animate {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        for event in &events {
            match event {
                Event::Copy => self.ed.copy_pose(),
                Event::Paste(_) | Event::PasteImage(_) => self.ed.paste_pose(false),
                _ => {}
            }
        }
        let command_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        let pressed = |modifiers: Modifiers, key: Key| ctx.input_mut(|i| i.consume_key(modifiers, key));
        if pressed(command_shift, Key::C) {
            self.export(ctx, self.ed.anim.second);
        }
        if pressed(command_shift, Key::V) {
            self.ed.paste_pose(true);
        }
        if pressed(command_shift, Key::Z) || pressed(Modifiers::COMMAND, Key::Y) {
            self.ed.redo();
        }
        if pressed(Modifiers::COMMAND, Key::Z) {
            self.ed.undo();
        }
        if pressed(command_shift, Key::S) {
            self.save(true);
        }
        if pressed(Modifiers::COMMAND, Key::S) {
            self.save(false);
        }
        if pressed(Modifiers::COMMAND, Key::O) {
            self.open();
        }
        if pressed(Modifiers::COMMAND, Key::N) {
            self.new_file();
        }
        if pressed(Modifiers::NONE, Key::Space) {
            self.toggle_play();
        }
        if pressed(Modifiers::SHIFT, Key::ArrowLeft) {
            self.ed.set_frame(0.0);
        }
        if pressed(Modifiers::SHIFT, Key::ArrowRight) {
            self.ed.set_frame(self.ed.anim.end as f32);
        }
        if pressed(Modifiers::NONE, Key::ArrowLeft) {
            self.ed.set_frame((self.ed.frame.round() - 1.0).max(0.0));
        }
        if pressed(Modifiers::NONE, Key::ArrowRight) {
            self.ed.set_frame(self.ed.frame.round() + 1.0);
        }
        if pressed(Modifiers::NONE, Key::ArrowUp) {
            self.ed.jump_key(true);
        }
        if pressed(Modifiers::NONE, Key::ArrowDown) {
            self.ed.jump_key(false);
        }
        if pressed(Modifiers::SHIFT, Key::I) {
            self.ed.key_both();
        }
        if pressed(Modifiers::NONE, Key::I) {
            self.ed.insert_keys();
        }
        if pressed(Modifiers::NONE, Key::B) {
            self.panels.open(Tool::Moves, ctx, &mut self.ed);
        }
        if pressed(Modifiers::NONE, Key::F) {
            self.panels.open(Tool::Body, ctx, &mut self.ed);
        }
    }

    fn export(&mut self, ctx: &egui::Context, second: bool) {
        let Some((anim, rig)) = self.ed.rig_for(second) else { return };
        let export = roblox_clipboard(anim, rig);
        let who = if self.ed.anim.partner.is_some() { format!("Rig {}: ", second as u8 + 1) } else { String::new() };
        if export.keyframes == 0 {
            self.ed.status = format!("{who}nothing to copy yet. Key a pose first");
            return;
        }
        ctx.copy_text(export.text);
        self.ed.status = format!("{who}copied {} keyframes ({:.2}s) for Roblox. Paste it into the importer plugin", export.keyframes, export.seconds);
    }

    fn new_file(&mut self) {
        self.ed.replace(Animation::fresh());
        self.path = None;
        self.file_saved = self.ed.changes;
        self.ed.status = "New animation".to_string();
    }

    fn open(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("welffi animation", &["wanim"]).pick_file() else { return };
        self.open_path(path);
    }

    fn open_path(&mut self, path: PathBuf) {
        match load_animation(&path) {
            Ok(anim) => {
                self.ed.replace(anim);
                self.ed.status = format!("Opened {}", path.display());
                self.path = Some(path);
                self.file_saved = self.ed.changes;
            }
            Err(error) => self.ed.status = format!("Couldn't open {}: {error}", path.display()),
        }
    }

    fn save(&mut self, choose: bool) {
        let path = match (&self.path, choose) {
            (Some(path), false) => path.clone(),
            _ => {
                let dialog = rfd::FileDialog::new().add_filter("welffi animation", &["wanim"]).set_file_name("animation.wanim");
                let Some(path) = dialog.save_file() else { return };
                path
            }
        };
        match save_animation(&path, &self.ed.anim) {
            Ok(()) => {
                self.ed.status = format!("Saved {}", path.display());
                self.path = Some(path);
                self.file_saved = self.ed.changes;
            }
            Err(error) => self.ed.status = format!("Couldn't save {}: {error}", path.display()),
        }
    }

    fn autosave(&mut self, force: bool) {
        if self.autosaved == self.ed.changes || (!force && self.last_autosave.elapsed() < AUTOSAVE_EVERY) {
            return;
        }
        let session = Session { path: self.path.clone(), anim: self.ed.anim.clone() };
        if let Err(error) = save_session(&session) {
            self.ed.status = format!("Autosave failed: {error}");
        }
        self.autosaved = self.ed.changes;
        self.last_autosave = Instant::now();
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let name = self.path.as_ref().and_then(|p| p.file_stem()).map_or("Untitled".to_string(), |s| s.to_string_lossy().to_string());
        let dirty = if self.file_saved != self.ed.changes { " *" } else { "" };
        let title = format!("{name}{dirty} - welffi animate");
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn advance_playback(&mut self, ctx: &egui::Context) {
        if !self.ed.playing {
            return;
        }
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        let end = self.ed.anim.end.max(1) as f32;
        let mut frame = self.ed.frame + dt * self.ed.anim.fps as f32;
        if frame > end {
            frame = frame.rem_euclid(end);
        }
        self.ed.set_frame(frame);
        ctx.request_repaint();
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx);
        self.advance_playback(&ctx);

        let rs = frame.wgpu_render_state().expect("wgpu renderer");
        let page = self.page;
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        if page != self.page && self.page == Page::Library {
            self.library.open(rs);
        }
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.label(&self.ed.status);
        });
        if self.page == Page::Animate {
            egui::Panel::bottom("timeline").resizable(true).default_size(210.0).frame(egui::Frame::NONE).show(ui, |ui| {
                self.timeline.ui(ui, &mut self.ed);
            });
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| match self.page {
            Page::Animate => self.viewport.ui(ui, rs, &mut self.ed),
            Page::Library => {
                if let Some(path) = self.library.ui(ui, rs, &mut self.ed) {
                    self.open_path(path);
                    self.page = Page::Animate;
                }
            }
        });

        self.panels.ui(&ctx, &mut self.ed);
        self.props.ui(&ctx, &mut self.ed);
        if self.viewport.busy() || self.timeline.busy() {
            ctx.request_repaint();
        }
        self.update_title(&ctx);
        self.autosave(false);
        if self.autosaved != self.ed.changes {
            ctx.request_repaint_after(AUTOSAVE_EVERY);
        }
    }

    fn on_exit(&mut self) {
        self.autosave(true);
    }
}
