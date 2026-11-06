use eframe::egui::{Pos2, Rect, Vec2};
use glam::{Mat4, Vec3, Vec4Swizzles};
use std::f32::consts::{FRAC_PI_2, PI};

const FOV: f32 = 0.7;

pub struct Camera {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { target: Vec3::new(0.0, 2.6, 0.0), yaw: PI + 0.5, pitch: 0.25, distance: 12.0 }
    }
}

impl Camera {
    pub fn toward_viewer(&self) -> Vec3 {
        Vec3::new(self.yaw.sin() * self.pitch.cos(), self.pitch.sin(), self.yaw.cos() * self.pitch.cos())
    }

    pub fn eye(&self) -> Vec3 {
        self.target + self.toward_viewer() * self.distance
    }

    pub fn right(&self) -> Vec3 {
        Vec3::Y.cross(self.toward_viewer()).normalize_or(Vec3::X)
    }

    pub fn up(&self) -> Vec3 {
        self.toward_viewer().cross(self.right())
    }

    pub fn view_proj(&self, rect: Rect) -> Mat4 {
        let aspect = rect.width().max(1.0) / rect.height().max(1.0);
        let view = glam::camera::rh::view::look_at_mat4(self.eye(), self.target, self.up());
        glam::camera::rh::proj::directx::perspective(FOV, aspect, 0.05, 1000.0) * view
    }

    pub fn project(&self, rect: Rect, point: Vec3) -> Option<Pos2> {
        let clip = self.view_proj(rect) * point.extend(1.0);
        if clip.w <= 1e-4 {
            return None;
        }
        let ndc = clip.xy() / clip.w;
        Some(Pos2::new(rect.left() + (ndc.x + 1.0) * 0.5 * rect.width(), rect.top() + (1.0 - ndc.y) * 0.5 * rect.height()))
    }

    pub fn ray(&self, rect: Rect, pos: Pos2) -> (Vec3, Vec3) {
        let inverse = self.view_proj(rect).inverse();
        let x = (pos.x - rect.left()) / rect.width() * 2.0 - 1.0;
        let y = 1.0 - (pos.y - rect.top()) / rect.height() * 2.0;
        let near = inverse.project_point3(Vec3::new(x, y, 0.0));
        let far = inverse.project_point3(Vec3::new(x, y, 1.0));
        (near, (far - near).normalize())
    }

    pub fn units_per_pixel(&self, rect: Rect, point: Vec3) -> f32 {
        let depth = (point - self.eye()).dot(-self.toward_viewer()).max(0.05);
        2.0 * depth * (FOV / 2.0).tan() / rect.height().max(1.0)
    }

    pub fn orbit(&mut self, delta: Vec2) {
        self.yaw -= delta.x * 0.008;
        self.pitch = (self.pitch + delta.y * 0.008).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
    }

    pub fn pan(&mut self, rect: Rect, delta: Vec2) {
        let scale = self.units_per_pixel(rect, self.target);
        self.target += (-self.right() * delta.x + self.up() * delta.y) * scale;
    }

    pub fn zoom(&mut self, steps: f32) {
        self.distance = (self.distance * 0.88f32.powf(steps)).clamp(0.5, 300.0);
    }

    pub fn look_from(&mut self, yaw: f32, pitch: f32) {
        self.yaw = yaw;
        self.pitch = pitch.clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
    }
}
