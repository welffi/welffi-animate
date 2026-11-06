#![windows_subsystem = "windows"]

mod anim;
mod body;
mod body_panel;
mod app;
mod camera;
mod curve;
mod easing;
mod editor;
mod export;
mod graph;
mod hints;
mod ik;
mod menus;
mod move_panel;
mod panels;
mod moves;
mod live;
mod step_panel;
mod steps;
mod nametags;
mod project;
mod render;
mod rig;
mod timeline;
mod transform;
mod viewport;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("welffi animate")
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 500.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native("welffi animate", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
