use crate::ik::Leg;
use glam::{Mat4, Quat, Vec3, Vec4};
use serde::{Deserialize, Serialize};

pub const JOINT_COUNT: usize = 6;
pub const TORSO: usize = 0;

pub struct Joint {
    pub part: &'static str,
    pub parent: Option<usize>,
    pub mirror: usize,
    pub c0: Mat4,
    pub c1: Mat4,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pose {
    pub pos: Vec3,
    pub rot: Quat,
}

impl Pose {
    pub const IDENTITY: Pose = Pose { pos: Vec3::ZERO, rot: Quat::IDENTITY };

    pub fn matrix(self) -> Mat4 {
        Mat4::from_rotation_translation(self.rot, self.pos)
    }

    pub fn from_matrix(m: Mat4) -> Pose {
        let (_, rot, pos) = m.to_scale_rotation_translation();
        Pose { pos, rot: rot.normalize() }
    }

    pub fn channels(self) -> [f32; 7] {
        [self.pos.x, self.pos.y, self.pos.z, self.rot.w, self.rot.x, self.rot.y, self.rot.z]
    }

    pub fn from_channels(c: [f32; 7]) -> Pose {
        let rot = Quat::from_xyzw(c[4], c[5], c[6], c[3]);
        let rot = if rot.length_squared() > 1e-12 { rot.normalize() } else { Quat::IDENTITY };
        Pose { pos: Vec3::new(c[0], c[1], c[2]), rot }
    }

    pub fn raw(c: [f32; 7]) -> Pose {
        Pose { pos: Vec3::new(c[0], c[1], c[2]), rot: Quat::from_xyzw(c[4], c[5], c[6], c[3]) }
    }

    pub fn normalized(self) -> Pose {
        Pose::from_channels(self.channels())
    }

    pub fn lerp(self, other: Pose, t: f32) -> Pose {
        Pose { pos: self.pos.lerp(other.pos, t), rot: self.rot.slerp(other.rot, t) }
    }

    pub fn aligned_to(self, reference: Quat) -> Pose {
        if self.rot.dot(reference) < 0.0 {
            Pose { pos: self.pos, rot: -self.rot }
        } else {
            self
        }
    }
}

pub fn cframe(v: [f32; 12]) -> Mat4 {
    Mat4::from_cols(
        Vec4::new(v[3], v[6], v[9], 0.0),
        Vec4::new(v[4], v[7], v[10], 0.0),
        Vec4::new(v[5], v[8], v[11], 0.0),
        Vec4::new(v[0], v[1], v[2], 1.0),
    )
}

pub fn cframe_values(m: Mat4) -> [f32; 12] {
    let r = |row: usize, col: usize| m.col(col)[row];
    [
        m.w_axis.x, m.w_axis.y, m.w_axis.z,
        r(0, 0), r(0, 1), r(0, 2),
        r(1, 0), r(1, 1), r(1, 2),
        r(2, 0), r(2, 1), r(2, 2),
    ]
}

pub fn root() -> Mat4 {
    Mat4::from_translation(Vec3::new(0.0, 3.0, 0.0))
}

const TORSO_FRAME: [f32; 9] = [-1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0];
const LEFT_FRAME: [f32; 9] = [0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0];
const RIGHT_FRAME: [f32; 9] = [0.0, 0.0, 1.0, 0.0, 1.0, 0.0, -1.0, 0.0, 0.0];

fn joint_cf(x: f32, y: f32, z: f32, r: [f32; 9]) -> Mat4 {
    cframe([x, y, z, r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], r[8]])
}

pub fn joints() -> [Joint; JOINT_COUNT] {
    [
        Joint { part: "Torso", parent: None, mirror: 0, c0: joint_cf(0.0, 0.0, 0.0, TORSO_FRAME), c1: joint_cf(0.0, 0.0, 0.0, TORSO_FRAME) },
        Joint { part: "Head", parent: Some(TORSO), mirror: 1, c0: joint_cf(0.0, 1.0, 0.0, TORSO_FRAME), c1: joint_cf(0.0, -0.5, 0.0, TORSO_FRAME) },
        Joint { part: "Left Arm", parent: Some(TORSO), mirror: 3, c0: joint_cf(-1.0, 0.5, 0.0, LEFT_FRAME), c1: joint_cf(0.5, 0.5, 0.0, LEFT_FRAME) },
        Joint { part: "Right Arm", parent: Some(TORSO), mirror: 2, c0: joint_cf(1.0, 0.5, 0.0, RIGHT_FRAME), c1: joint_cf(-0.5, 0.5, 0.0, RIGHT_FRAME) },
        Joint { part: "Left Leg", parent: Some(TORSO), mirror: 5, c0: joint_cf(-1.0, -1.0, 0.0, LEFT_FRAME), c1: joint_cf(-0.5, 1.0, 0.0, LEFT_FRAME) },
        Joint { part: "Right Leg", parent: Some(TORSO), mirror: 4, c0: joint_cf(1.0, -1.0, 0.0, RIGHT_FRAME), c1: joint_cf(0.5, 1.0, 0.0, RIGHT_FRAME) },
    ]
}

pub const LEFT_LEG: usize = 4;
pub const RIGHT_LEG: usize = 5;

pub struct Rig {
    pub joints: [Joint; JOINT_COUNT],
    legs: [Leg; 2],
}

impl Rig {
    pub fn r6() -> Rig {
        let joints = joints();
        let rest = |i: usize| root() * joints[i].c0 * joints[i].c1.inverse();
        let torso = rest(TORSO);
        let leg_rest = |i: usize| torso * joints[i].c0 * joints[i].c1.inverse();
        let legs = [Leg::new(-1.0, torso, leg_rest(LEFT_LEG)), Leg::new(1.0, torso, leg_rest(RIGHT_LEG))];
        Rig { joints, legs }
    }

    pub fn leg(&self, joint: usize) -> Option<&Leg> {
        match joint {
            LEFT_LEG => Some(&self.legs[0]),
            RIGHT_LEG => Some(&self.legs[1]),
            _ => None,
        }
    }

    pub fn parent_world(&self, joint: usize, worlds: &[Mat4; JOINT_COUNT]) -> Mat4 {
        match self.joints[joint].parent {
            Some(p) => worlds[p],
            None => root(),
        }
    }

    pub fn worlds(&self, poses: &[Pose; JOINT_COUNT]) -> [Mat4; JOINT_COUNT] {
        let mut worlds = [Mat4::IDENTITY; JOINT_COUNT];
        for i in 0..JOINT_COUNT {
            let j = &self.joints[i];
            worlds[i] = match self.leg(i) {
                Some(leg) => leg.world(worlds[TORSO], poses[i]),
                None => self.parent_world(i, &worlds) * j.c0 * poses[i].matrix() * j.c1.inverse(),
            };
        }
        worlds
    }

    pub fn motors(&self, poses: &[Pose; JOINT_COUNT]) -> [Pose; JOINT_COUNT] {
        let worlds = self.worlds(poses);
        std::array::from_fn(|i| match self.leg(i) {
            Some(_) => self.pose_for_world(i, worlds[TORSO], worlds[i]),
            None => poses[i],
        })
    }

    pub fn control(&self, joint: usize, poses: &[Pose; JOINT_COUNT], worlds: &[Mat4; JOINT_COUNT]) -> Mat4 {
        match self.leg(joint) {
            Some(leg) => leg.target(poses[joint]),
            None => worlds[joint],
        }
    }

    pub fn pivot(&self, joint: usize, poses: &[Pose; JOINT_COUNT], worlds: &[Mat4; JOINT_COUNT]) -> Vec3 {
        let frame = match self.leg(joint) {
            Some(leg) => leg.target(poses[joint]),
            None => self.parent_world(joint, worlds) * self.joints[joint].c0 * poses[joint].matrix(),
        };
        frame.w_axis.truncate()
    }

    pub fn pose_for_control(&self, joint: usize, worlds: &[Mat4; JOINT_COUNT], control: Mat4) -> Pose {
        match self.leg(joint) {
            Some(leg) => leg.pose_for_target(control),
            None => self.pose_for_world(joint, self.parent_world(joint, worlds), control),
        }
    }

    pub fn pose_for_world(&self, joint: usize, parent_world: Mat4, world: Mat4) -> Pose {
        let j = &self.joints[joint];
        Pose::from_matrix(j.c0.inverse() * parent_world.inverse() * world * j.c1)
    }

    pub fn mirrored(&self, joint: usize, pose: Pose) -> Pose {
        let flip = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
        if self.leg(joint).is_some() {
            return Pose::from_matrix(flip * pose.matrix() * flip);
        }
        let from = &self.joints[joint];
        let relative = from.c0 * pose.matrix() * from.c1.inverse();
        let mirrored = flip * relative * flip;
        let to = &self.joints[from.mirror];
        Pose::from_matrix(to.c0.inverse() * mirrored * to.c1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn rest_pose_places_parts_like_roblox() {
        let rig = Rig::r6();
        let worlds = rig.worlds(&[Pose::IDENTITY; JOINT_COUNT]);
        let expected = [(0.0, 3.0, 0.0), (0.0, 4.5, 0.0), (-1.5, 3.0, 0.0), (1.5, 3.0, 0.0), (-0.5, 1.0, 0.0), (0.5, 1.0, 0.0)];
        for (w, e) in worlds.iter().zip(expected) {
            assert!(near(w.w_axis.truncate(), Vec3::new(e.0, e.1, e.2)));
            assert!(Quat::from_mat4(w).angle_between(Quat::IDENTITY) < 1e-4);
        }
    }

    #[test]
    fn pose_for_world_inverts_worlds() {
        let rig = Rig::r6();
        let mut poses = [Pose::IDENTITY; JOINT_COUNT];
        poses[2] = Pose { pos: Vec3::new(0.1, 0.2, 0.0), rot: Quat::from_euler(glam::EulerRot::XYZ, 0.3, -0.5, 1.1) };
        let worlds = rig.worlds(&poses);
        let back = rig.pose_for_world(2, worlds[0], worlds[2]);
        assert!(near(back.pos, poses[2].pos));
        assert!(back.rot.angle_between(poses[2].rot) < 1e-4);
    }

    #[test]
    fn cframe_round_trips() {
        let v = [1.0, 2.0, 3.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0];
        assert_eq!(cframe_values(cframe(v)), v);
    }

    #[test]
    fn mirror_swaps_arm_raise_to_other_side() {
        let rig = Rig::r6();
        let mut poses = [Pose::IDENTITY; JOINT_COUNT];
        let worlds = rig.worlds(&poses);
        let pivot = rig.pivot(2, &poses, &worlds);
        let raise = Mat4::from_translation(pivot) * Mat4::from_rotation_z(-1.2) * Mat4::from_translation(-pivot);
        poses[2] = rig.pose_for_world(2, worlds[0], raise * worlds[2]);
        poses[3] = rig.mirrored(2, poses[2]);
        let posed = rig.worlds(&poses);
        let left = posed[2].w_axis.truncate();
        let right = posed[3].w_axis.truncate();
        assert!(near(right, Vec3::new(-left.x, left.y, left.z)));
    }
}
