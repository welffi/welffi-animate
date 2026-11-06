use crate::rig::Pose;
use glam::{Mat3, Mat4, Vec3};

const THIGH: f32 = 0.7071;
const SHIN: f32 = 1.1985;
pub const HIP_HEIGHT: f32 = -1.002;
const FOOT_HEIGHT: f32 = 0.093;
const POLE: Vec3 = Vec3::new(0.0, -0.029, -0.465);

pub struct Leg {
    hip: Vec3,
    rest_foot: Vec3,
    rest_world: Mat4,
    rest_shin: Mat3,
    rest_end: Vec3,
}

struct Chain {
    end: Vec3,
    shin: Mat3,
}

impl Leg {
    pub fn new(side: f32, rest_torso: Mat4, rest_world: Mat4) -> Leg {
        let mut leg = Leg {
            hip: Vec3::new(side * 0.5, HIP_HEIGHT, 0.0),
            rest_foot: Vec3::new(side * 0.5, FOOT_HEIGHT, 0.0),
            rest_world,
            rest_shin: Mat3::IDENTITY,
            rest_end: Vec3::ZERO,
        };
        let rest = leg.solve(rest_torso, Vec3::ZERO, leg.target(Pose::IDENTITY));
        leg.rest_shin = rest.shin;
        leg.rest_end = rest.end;
        leg
    }

    pub fn target(&self, pose: Pose) -> Mat4 {
        Mat4::from_rotation_translation(pose.rot, self.rest_foot + pose.pos)
    }

    pub fn pose_for_target(&self, target: Mat4) -> Pose {
        let pose = Pose::from_matrix(target);
        Pose { pos: pose.pos - self.rest_foot, rot: pose.rot }
    }

    pub fn world(&self, torso: Mat4, hip_shift: Vec3, pose: Pose) -> Mat4 {
        let chain = self.solve(torso, hip_shift, self.target(pose));
        let turn = Mat4::from_mat3(chain.shin * self.rest_shin.transpose());
        Mat4::from_translation(chain.end) * turn * Mat4::from_translation(-self.rest_end) * self.rest_world
    }

    fn solve(&self, torso: Mat4, hip_shift: Vec3, target: Mat4) -> Chain {
        let hip = torso.transform_point3(self.hip + hip_shift);
        let pole = target.transform_point3(POLE);
        let to = target.w_axis.truncate() - hip;
        let dir = to.try_normalize().unwrap_or(Vec3::NEG_Y);
        let reach = to.length().clamp((THIGH - SHIN).abs() + 1e-3, THIGH + SHIN);
        let along = (THIGH * THIGH - SHIN * SHIN + reach * reach) / (2.0 * reach);
        let height = (THIGH * THIGH - along * along).max(0.0).sqrt();
        let bend = (pole - hip).reject_from(dir).try_normalize().unwrap_or_else(|| dir.any_orthonormal_vector());
        let knee = hip + dir * along + bend * height;
        let end = hip + dir * reach;
        let down = (end - knee).normalize();
        let side = down.cross(bend).normalize();
        Chain { end, shin: Mat3::from_cols(down, side, down.cross(side)) }
    }
}

#[cfg(test)]
mod tests {
    use crate::rig::{Pose, Rig, JOINT_COUNT, LEFT_HIP, LEFT_LEG};
    use glam::Vec3;

    fn hip_point(lift: f32) -> Vec3 {
        let rig = Rig::r6();
        let mut poses = [Pose::IDENTITY; JOINT_COUNT];
        poses[4].pos.y = lift;
        let worlds = rig.worlds(&poses);
        (worlds[4] * rig.joints[4].c1).w_axis.truncate()
    }

    #[test]
    fn rest_target_keeps_leg_at_rest() {
        assert!((hip_point(0.0) - Vec3::new(-1.0, 2.0, 0.0)).length() < 1e-4);
    }

    #[test]
    fn moving_the_hip_leaves_the_foot_planted() {
        let rig = Rig::r6();
        let rest = rig.worlds(&[Pose::IDENTITY; JOINT_COUNT]);
        let foot = Vec3::new(-0.5, 0.093, 0.0);
        let on_leg = rest[LEFT_LEG].inverse().transform_point3(foot);
        let mut poses = [Pose::IDENTITY; JOINT_COUNT];
        poses[LEFT_HIP].pos = Vec3::new(0.2, -0.3, 0.1);
        let worlds = rig.worlds(&poses);
        assert!((worlds[LEFT_LEG].transform_point3(on_leg) - foot).length() < 0.02);
        assert!((worlds[LEFT_LEG].w_axis - rest[LEFT_LEG].w_axis).truncate().length() > 0.1);
    }

    #[test]
    fn raised_foot_matches_blender_rig() {
        assert!((hip_point(0.5) - Vec3::new(-1.0, 2.257, -0.93)).length() < 0.02);
        assert!((hip_point(1.0) - Vec3::new(-1.0, 2.655, -1.093)).length() < 0.02);
    }
}
