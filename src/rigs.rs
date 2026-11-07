use crate::anim::Animation;
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stand {
    pub x: f32,
    pub z: f32,
    pub turn: f32,
}

impl Default for Stand {
    fn default() -> Self {
        Stand { x: 0.0, z: -4.0, turn: PI }
    }
}

impl Stand {
    pub fn matrix(self) -> Mat4 {
        Mat4::from_translation(Vec3::new(self.x, 0.0, self.z)) * Mat4::from_rotation_y(self.turn)
    }

    pub fn inverse(self) -> Stand {
        let back = Mat4::from_rotation_y(-self.turn).transform_vector3(Vec3::new(-self.x, 0.0, -self.z));
        Stand { x: back.x, z: back.z, turn: -self.turn }
    }
}

impl Animation {
    pub fn add_partner(&mut self) {
        let mut partner = Animation::fresh();
        partner.second = !self.second;
        self.at = Stand::default();
        self.partner = Some(Box::new(partner));
        self.sync_partner();
    }

    pub fn sync_partner(&mut self) {
        let (fps, end) = (self.fps, self.end);
        if let Some(partner) = self.partner.as_mut() {
            partner.recalc();
            partner.fps = fps;
            partner.end = end;
        }
    }

    pub fn set_timing(&mut self, fps: u32, end: i32) {
        self.fps = fps;
        self.end = end;
        self.sync_partner();
    }

    pub fn rerooted(mut self) -> Animation {
        let Some(mut other) = self.partner.take() else { return self };
        other.at = self.at.inverse();
        self.at = Stand::default();
        other.partner = Some(Box::new(self));
        *other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::Handle;
    use crate::rig::Pose;

    fn near(a: Mat4, b: Mat4) -> bool {
        a.abs_diff_eq(b, 1e-4)
    }

    #[test]
    fn a_stand_and_its_inverse_cancel_out() {
        let stand = Stand { x: 1.5, z: -3.0, turn: 0.8 };
        assert!(near(stand.matrix() * stand.inverse().matrix(), Mat4::IDENTITY));
        assert!(near(Stand::default().matrix() * Stand::default().inverse().matrix(), Mat4::IDENTITY));
    }

    #[test]
    fn the_second_rig_starts_facing_the_first() {
        let facing = Stand::default().matrix().transform_vector3(Vec3::NEG_Z);
        assert!(facing.abs_diff_eq(Vec3::Z, 1e-5));
    }

    #[test]
    fn rerooting_twice_gets_the_same_animation_back() {
        let mut anim = Animation::fresh();
        anim.add_partner();
        anim.at = Stand { x: 1.0, z: -2.5, turn: 2.5 };
        anim.partner.as_mut().unwrap().tracks[2].set(10, Pose::IDENTITY, Handle::AutoClamped);
        let switched = anim.clone().rerooted();
        assert!(switched.second);
        assert!(switched.tracks[2].key_at(10).is_some());
        assert!(switched.partner.as_ref().is_some_and(|p| !p.second && p.partner.is_none()));
        let back = switched.rerooted();
        assert_eq!(back.second, anim.second);
        assert!(near(back.at.matrix(), anim.at.matrix()));
        assert_eq!(back.tracks, anim.tracks);
    }

    #[test]
    fn the_partner_follows_the_timing() {
        let mut anim = Animation::fresh();
        anim.add_partner();
        anim.set_timing(30, 90);
        let partner = anim.partner.as_ref().unwrap();
        assert_eq!((partner.fps, partner.end), (30, 90));
        let switched = anim.rerooted();
        assert_eq!((switched.fps, switched.end), (30, 90));
    }

    #[test]
    fn a_partner_survives_save_and_load_text() {
        let mut anim = Animation::fresh();
        anim.add_partner();
        let text = serde_json::to_string(&anim).unwrap();
        let back: Animation = serde_json::from_str(&text).unwrap();
        assert_eq!(back, anim);
        let old: Animation = serde_json::from_str(r#"{"fps":60,"end":30,"tracks":[]}"#).unwrap();
        assert!(old.partner.is_none() && !old.second);
    }
}
