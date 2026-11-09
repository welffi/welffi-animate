use crate::clip::Clip;
use crate::clip_load::{is_media, Loader, EXTENSIONS};
use crate::player::{Player, SPEEDS};
use eframe::egui::{self, Button, ColorImage, RichText, Slider, TextureHandle, TextureOptions, Vec2};
use std::path::PathBuf;

#[derive(Default)]
pub struct ReferencePanel {
    pub open: bool,
    synced: bool,
    timeline_was_playing: bool,
    name: String,
    clip: Clip,
    loader: Option<Loader>,
    player: Player,
    texture: Option<(TextureHandle, usize)>,
    error: Option<String>,
}

impl ReferencePanel {
    pub fn pick(&mut self, ctx: &egui::Context) {
        let dialog = rfd::FileDialog::new().add_filter("GIF or video", &EXTENSIONS);
        if let Some(path) = dialog.pick_file() {
            self.load(path, ctx);
        }
    }

    pub fn accept_drop(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).find(|p| is_media(p)));
        if let Some(path) = dropped {
            self.load(path, ctx);
        }
    }

    fn load(&mut self, path: PathBuf, ctx: &egui::Context) {
        self.name = path.file_name().map_or(String::new(), |n| n.to_string_lossy().to_string());
        self.clip = Clip::default();
        self.player = Player::default();
        self.texture = None;
        self.error = None;
        self.loader = Some(Loader::start(path, ctx.clone()));
        self.open = true;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, timeline_playing: bool) {
        if self.synced {
            self.player.follow(timeline_playing, self.timeline_was_playing, self.clip.end);
        }
        self.timeline_was_playing = timeline_playing;
        self.receive(ui.ctx());
        self.header(ui);
        egui::Panel::bottom("reference_controls").show_separator_line(false).show(ui, |ui| self.controls(ui));
        self.picture(ui);
    }

    fn receive(&mut self, ctx: &egui::Context) {
        if let Some(loader) = &self.loader {
            self.error = loader.poll(&mut self.clip).or(self.error.take());
            if self.clip.finished {
                self.loader = None;
            }
        }
        if self.player.playing {
            self.player.advance(ctx.input(|i| i.stable_dt).min(0.1) as f64, self.clip.end);
            ctx.request_repaint();
        }
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Reference");
            if ui.button("Open...").on_hover_text("A GIF or video. You can also drop one on the window").clicked() {
                self.pick(ui.ctx());
            }
            if ui.button("Close").clicked() {
                self.open = false;
            }
        });
        if let Some(error) = &self.error {
            ui.label(RichText::new(error).color(ui.visuals().error_fg_color));
        } else if self.clip.frames.is_empty() && self.loader.is_none() {
            ui.label(RichText::new("Open or drop a GIF or video to use as a reference.").weak());
        } else {
            ui.label(RichText::new(&self.name).weak());
        }
    }

    fn picture(&mut self, ui: &mut egui::Ui) {
        let index = self.clip.index_at(self.player.time);
        let Some(frame) = self.clip.frames.get(index) else { return };
        let image = ColorImage::from_rgba_unmultiplied(frame.size, &frame.rgba);
        match &mut self.texture {
            Some((texture, shown)) if *shown != index => {
                texture.set(image, TextureOptions::LINEAR);
                *shown = index;
            }
            None => self.texture = Some((ui.ctx().load_texture("reference", image, TextureOptions::LINEAR), index)),
            _ => {}
        }
        let Some((texture, _)) = &self.texture else { return };
        let room = ui.available_size();
        let size = Vec2::new(frame.size[0] as f32, frame.size[1] as f32);
        let fitted = size * (room.x / size.x).min(room.y / size.y).max(0.0);
        ui.vertical_centered(|ui| ui.add(egui::Image::new((texture.id(), fitted)).fit_to_exact_size(fitted)));
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        let duration = self.clip.end;
        let ready = !self.clip.frames.is_empty();
        ui.add_enabled_ui(ready, |ui| {
            self.scrubber(ui, duration);
            self.transport(ui, duration);
            self.looping(ui);
            self.speeds(ui);
        });
    }

    fn scrubber(&mut self, ui: &mut egui::Ui, duration: f64) {
        ui.horizontal(|ui| {
            ui.spacing_mut().slider_width = (ui.available_width() - 130.0).max(60.0);
            let mut time = self.player.time;
            if ui.add(Slider::new(&mut time, 0.0..=duration.max(0.001)).show_value(false)).changed() {
                self.player.time = time;
                self.player.playing = false;
            }
            let frame = self.clip.index_at(self.player.time) + 1;
            ui.label(RichText::new(format!("{:.2}s  {frame}/{}", self.player.time, self.clip.frames.len())).monospace());
        });
    }

    fn transport(&mut self, ui: &mut egui::Ui, duration: f64) {
        ui.horizontal(|ui| {
            let (start, _) = self.player.range(duration);
            if ui.button("|<").on_hover_text("Start of the loop").clicked() {
                self.player.time = start;
            }
            if ui.button("<").on_hover_text("Previous frame").clicked() {
                self.step(false);
            }
            let label = if self.player.playing { "Pause" } else { "Play" };
            if ui.button(label).clicked() {
                self.player.toggle(duration);
            }
            if ui.button(">").on_hover_text("Next frame").clicked() {
                self.step(true);
            }
            ui.separator();
            if ui.add(Button::new("Sync").selected(self.synced)).on_hover_text("Play and pause along with the timeline").clicked() {
                self.synced = !self.synced;
            }
        });
    }

    fn step(&mut self, forward: bool) {
        self.player.playing = false;
        self.player.time = self.clip.neighbor_start(self.player.time, forward);
    }

    fn looping(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.player.looping, "Loop");
            if ui.button("Set in").on_hover_text("Start the loop at this frame").clicked() {
                self.player.set_in();
            }
            if ui.button("Set out").on_hover_text("End the loop at this frame").clicked() {
                self.player.set_out();
            }
            if ui.button("Clear").on_hover_text("Loop the whole clip").clicked() {
                self.player.clear_loop();
            }
        });
        let (start, end) = self.player.range(self.clip.end);
        if self.player.loop_in.is_some() || self.player.loop_out.is_some() {
            ui.label(RichText::new(format!("Loop {start:.2}s to {end:.2}s")).weak());
        }
        if self.clip.cut_short {
            ui.label(RichText::new(format!("Only the first {:.1}s fit in memory", self.clip.end)).weak());
        }
    }

    fn speeds(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Speed");
            for speed in SPEEDS {
                ui.selectable_value(&mut self.player.speed, speed, format!("{speed}x"));
            }
        });
    }
}
