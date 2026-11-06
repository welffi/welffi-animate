use crate::anim::{Animation, KeyId, Track};
use crate::curve::Handle;
use crate::rig::{Pose, Rig, TORSO};
use glam::{Mat4, Quat, Vec3};
use std::collections::{BTreeMap, BTreeSet};

pub const HEAD: usize = 1;
pub const LEFT_ARM: usize = 2;
pub const RIGHT_ARM: usize = 3;
const TWIST: f32 = 0.12;
const FORWARD_LEAN: f32 = 0.05;
const SIDE_LEAN: f32 = 0.06;
const HEAD_COUNTER: f32 = 0.6;
const ARM_SWING: f32 = 0.15;
const HAND: Vec3 = Vec3::new(0.0, -1.0, 0.0);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Follow {
    pub torso: f32,
    pub head: f32,
    pub arm: f32,
    pub lead: i32,
    pub drag: i32,
}

impl Default for Follow {
    fn default() -> Self {
        Follow { torso: 1.0, head: 1.0, arm: 1.0, lead: 2, drag: 3 }
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    pub keyed: Vec<(usize, usize)>,
    pub skipped: Vec<(KeyId, &'static str)>,
}

fn other_arm(arm: usize) -> usize {
    if arm == LEFT_ARM { RIGHT_ARM } else { LEFT_ARM }
}

fn hand(rig: &Rig, anim: &Animation, arm: usize, frame: i32) -> Vec3 {
    let worlds = rig.worlds(&anim.poses(frame as f32));
    (worlds[TORSO].inverse() * worlds[arm]).transform_point3(HAND)
}

fn torso_turn(reach: Vec3, side: f32, strength: f32) -> Quat {
    let twist = Quat::from_rotation_y(-side * TWIST * reach.z * strength);
    let lean = Quat::from_rotation_x(FORWARD_LEAN * reach.z * strength);
    let bend = Quat::from_rotation_z(side * SIDE_LEAN * reach.y * strength);
    twist * lean * bend
}

struct Reactions {
    tracks: BTreeMap<usize, Vec<Track>>,
}

impl Reactions {
    fn add(&mut self, target: usize, keys: Vec<(i32, Quat)>) {
        let mut track = Track::default();
        for (frame, rot) in keys {
            track.set(frame, Pose { pos: Vec3::ZERO, rot }, Handle::AutoClamped);
        }
        self.tracks.entry(target).or_default().push(track);
    }

    fn offset(&self, target: usize, frame: i32) -> Quat {
        self.tracks[&target].iter().fold(Quat::IDENTITY, |turn, track| turn * track.sample(frame as f32).rot)
    }

    fn frames(&self, target: usize) -> BTreeSet<i32> {
        self.tracks[&target].iter().flat_map(|t| t.keys.iter().map(|k| k.frame)).collect()
    }
}

fn arm_moves(anim: &Animation, keys: &BTreeSet<KeyId>, outcome: &mut Outcome) -> BTreeMap<usize, Vec<i32>> {
    let mut moves: BTreeMap<usize, Vec<i32>> = BTreeMap::new();
    for &(track, frame) in keys {
        if track != LEFT_ARM && track != RIGHT_ARM {
            outcome.skipped.push(((track, frame), "only arm keys move the body for now"));
            continue;
        }
        moves.entry(track).or_default().push(frame);
    }
    for (&arm, frames) in moves.iter_mut() {
        let first = frames[0];
        if let Some(anchor) = anim.tracks[arm].keys.iter().rev().map(|k| k.frame).find(|&f| f < first) {
            frames.insert(0, anchor);
        }
    }
    moves.retain(|&arm, frames| {
        let moving = frames.len() > 1;
        if !moving {
            outcome.skipped.push(((arm, frames[0]), "first key, so no move lands on it"));
        }
        moving
    });
    moves
}

pub fn follow_body(anim: &mut Animation, rig: &Rig, keys: &BTreeSet<KeyId>, follow: Follow, handle: Handle) -> Outcome {
    let mut outcome = Outcome::default();
    let base = anim.clone();
    let moves = arm_moves(&base, keys, &mut outcome);
    let mut reactions = Reactions { tracks: BTreeMap::new() };
    for (&arm, frames) in &moves {
        let side = if arm == RIGHT_ARM { 1.0 } else { -1.0 };
        let start = hand(rig, &base, arm, frames[0]);
        let reaches: Vec<(i32, Vec3)> = frames.iter().map(|&f| (f, hand(rig, &base, arm, f) - start)).collect();
        let turns: Vec<(i32, Quat)> = reaches.iter().map(|&(f, reach)| (f, torso_turn(reach, side, follow.torso))).collect();
        reactions.add(TORSO, turns.iter().map(|&(f, turn)| (f - follow.lead, turn)).collect());
        reactions.add(HEAD, turns.iter().map(|&(f, turn)| (f + follow.drag, Quat::IDENTITY.slerp(turn.inverse(), HEAD_COUNTER * follow.head))).collect());
        if !moves.contains_key(&other_arm(arm)) {
            let swing = reaches.iter().map(|&(f, reach)| (f + follow.drag, Quat::from_rotation_x(ARM_SWING * reach.z * follow.arm))).collect();
            reactions.add(other_arm(arm), swing);
        }
    }
    for (&target, _) in &reactions.tracks {
        let own = base.tracks[target].keys.iter().map(|k| k.frame);
        let frames: BTreeSet<i32> = own.chain(reactions.frames(target)).filter(|&f| f >= 0).collect();
        for &frame in &frames {
            let poses = base.poses(frame as f32);
            let worlds = rig.worlds(&poses);
            let pivot = rig.pivot(target, &poses, &worlds);
            let torso = Quat::from_mat4(&worlds[TORSO]);
            let turn = torso * reactions.offset(target, frame) * torso.inverse();
            let around = Mat4::from_translation(pivot) * Mat4::from_quat(turn) * Mat4::from_translation(-pivot);
            let pose = rig.pose_for_world(target, rig.parent_world(target, &worlds), around * worlds[target]);
            anim.tracks[target].set(frame, pose, handle);
        }
        outcome.keyed.push((target, frames.len()));
    }
    anim.recalc();
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig::JOINT_COUNT;

    fn punch() -> (Rig, Animation) {
        let rig = Rig::r6();
        let mut anim = Animation::default();
        let poses = [Pose::IDENTITY; JOINT_COUNT];
        let worlds = rig.worlds(&poses);
        let pivot = rig.pivot(RIGHT_ARM, &poses, &worlds);
        let raise = Mat4::from_translation(pivot) * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2) * Mat4::from_translation(-pivot);
        anim.tracks[RIGHT_ARM].set(0, Pose::IDENTITY, Handle::AutoClamped);
        anim.tracks[RIGHT_ARM].set(10, rig.pose_for_world(RIGHT_ARM, worlds[TORSO], raise * worlds[RIGHT_ARM]), Handle::AutoClamped);
        (rig, anim)
    }

    #[test]
    fn punch_reaches_forward() {
        let (rig, anim) = punch();
        assert!(hand(&rig, &anim, RIGHT_ARM, 10).z < -1.0);
    }

    #[test]
    fn torso_leads_and_the_rest_drags_behind() {
        let (rig, mut anim) = punch();
        anim.tracks[TORSO].set(0, Pose { pos: Vec3::ZERO, rot: Quat::from_rotation_y(0.3) }, Handle::AutoClamped);
        let own = anim.tracks[TORSO].keys[0].pose.rot;
        let shoulder_before = rig.worlds(&anim.poses(8.0))[TORSO].transform_vector3(Vec3::X);
        let outcome = follow_body(&mut anim, &rig, &BTreeSet::from([(RIGHT_ARM, 10)]), Follow::default(), Handle::AutoClamped);
        assert!(outcome.skipped.is_empty());
        let frames = |t: usize| anim.tracks[t].keys.iter().map(|k| k.frame).collect::<Vec<_>>();
        assert_eq!(frames(TORSO), vec![0, 8]);
        assert_eq!(frames(HEAD), vec![3, 13]);
        assert_eq!(frames(LEFT_ARM), vec![3, 13]);
        assert!(anim.tracks[TORSO].keys[0].pose.rot.angle_between(own) < 0.05);
        let right_shoulder = rig.worlds(&anim.poses(8.0))[TORSO].transform_vector3(Vec3::X);
        assert!(right_shoulder.z < shoulder_before.z - 0.1, "{right_shoulder} {shoulder_before}");
        let worlds = rig.worlds(&anim.poses(13.0));
        let head_in_torso = Quat::from_mat4(&(worlds[TORSO].inverse() * worlds[HEAD]));
        assert!(head_in_torso.to_axis_angle().1 > 0.01);
        assert!(hand(&rig, &anim, LEFT_ARM, 13).z > 0.1);
    }

    #[test]
    fn says_why_keys_were_left_alone() {
        let (rig, mut anim) = punch();
        anim.tracks[HEAD].set(5, Pose::IDENTITY, Handle::AutoClamped);
        let outcome = follow_body(&mut anim, &rig, &BTreeSet::from([(HEAD, 5), (RIGHT_ARM, 0)]), Follow::default(), Handle::AutoClamped);
        assert_eq!(outcome.skipped.len(), 2);
        assert!(outcome.keyed.is_empty());
    }
}
