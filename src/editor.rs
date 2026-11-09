use crate::anim::{Animation, KeyId};
use crate::curve::Handle;
use crate::key_clipboard::KeyClipboard;
use crate::rigs::Stand;
use crate::rig::{Pose, Rig};
use std::collections::BTreeSet;

const UNDO_LIMIT: usize = 200;

pub struct Editor {
    pub rig: Rig,
    pub partner_rig: Option<Rig>,
    pub anim: Animation,
    pub frame: f32,
    pub playing: bool,
    pub auto_key: bool,
    pub new_handle: Handle,
    pub key_names: bool,
    pub parts: BTreeSet<usize>,
    pub active: Option<usize>,
    pub keys: BTreeSet<KeyId>,
    pub overrides: Vec<Option<Pose>>,
    pub status: String,
    pub changes: u64,
    copied: Vec<Option<Pose>>,
    key_clipboard: KeyClipboard,
    undo: Vec<Animation>,
    redo: Vec<Animation>,
}

impl Editor {
    pub fn new(anim: Animation) -> Editor {
        Editor {
            rig: Rig::new(&anim.props),
            partner_rig: partner_rig(&anim),
            overrides: vec![None; anim.tracks.len()],
            anim,
            frame: 0.0,
            playing: false,
            auto_key: true,
            new_handle: Handle::AutoClamped,
            key_names: false,
            parts: BTreeSet::new(),
            active: None,
            keys: BTreeSet::new(),
            status: String::new(),
            changes: 0,
            copied: Vec::new(),
            key_clipboard: KeyClipboard::default(),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn current(&self) -> i32 {
        self.frame.round() as i32
    }

    pub fn poses(&self) -> Vec<Pose> {
        let mut poses = self.anim.poses(self.frame);
        for (pose, over) in poses.iter_mut().zip(&self.overrides) {
            if let Some(over) = *over {
                *pose = over;
            }
        }
        poses
    }

    pub fn set_frame(&mut self, frame: f32) {
        if frame != self.frame {
            self.frame = frame;
            self.clear_overrides();
        }
    }

    fn clear_overrides(&mut self) {
        self.overrides.iter_mut().for_each(|o| *o = None);
    }

    pub fn props_changed(&mut self) {
        self.rig = Rig::new(&self.anim.props);
        self.partner_rig = partner_rig(&self.anim);
        let count = self.anim.tracks.len();
        self.overrides = vec![None; count];
        self.copied.resize(count, None);
        self.parts.retain(|&j| j < count);
        self.active = self.active.filter(|&j| j < count);
        self.keys.retain(|&(j, _)| j < count);
    }

    pub fn checkpoint(&mut self) {
        self.undo.push(self.anim.clone());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.changes += 1;
    }

    pub fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            let current = self.swap_in(previous);
            self.redo.push(current);
            self.after_history("Undo");
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            let current = self.swap_in(next);
            self.undo.push(current);
            self.after_history("Redo");
        }
    }

    fn swap_in(&mut self, anim: Animation) -> Animation {
        let anim = if anim.second == self.anim.second { anim } else { anim.rerooted() };
        std::mem::replace(&mut self.anim, anim)
    }

    pub fn rig_for(&self, second: bool) -> Option<(&Animation, &Rig)> {
        if self.anim.second == second {
            return Some((&self.anim, &self.rig));
        }
        self.anim.partner.as_deref().zip(self.partner_rig.as_ref())
    }

    pub fn add_partner(&mut self) {
        self.checkpoint();
        self.anim.add_partner();
        self.props_changed();
        self.status = "Added a second rig. Click either one, or press Tab, to switch".to_string();
    }

    pub fn remove_partner(&mut self) {
        self.checkpoint();
        self.anim.partner = None;
        self.props_changed();
        self.status = "Removed the other rig".to_string();
    }

    pub fn switch_rig(&mut self) -> Option<Stand> {
        self.anim.partner.as_ref()?;
        self.anim = std::mem::take(&mut self.anim).rerooted();
        self.parts.clear();
        self.active = None;
        self.keys.clear();
        self.props_changed();
        self.changes += 1;
        self.status = format!("Editing Rig {}", if self.anim.second { 2 } else { 1 });
        Some(self.anim.at)
    }

    pub fn key_both(&mut self) {
        if self.anim.partner.is_none() {
            return;
        }
        let frame = self.current();
        let handle = self.new_handle;
        let own = self.poses();
        self.checkpoint();
        for (track, pose) in self.anim.tracks.iter_mut().zip(own) {
            track.set(frame, pose, handle);
        }
        if let Some(partner) = self.anim.partner.as_mut() {
            let poses = partner.poses(frame as f32);
            for (track, pose) in partner.tracks.iter_mut().zip(poses) {
                track.set(frame, pose, handle);
            }
        }
        self.clear_overrides();
        self.frame = frame as f32;
        self.status = format!("Keyed both rigs at frame {frame}");
    }

    fn after_history(&mut self, what: &str) {
        self.props_changed();
        let all = self.anim.all_keys();
        self.keys.retain(|k| all.contains(k));
        self.changes += 1;
        self.status = what.to_string();
    }

    pub fn replace(&mut self, anim: Animation) {
        self.anim = anim;
        self.undo.clear();
        self.redo.clear();
        self.keys.clear();
        self.props_changed();
        self.frame = 0.0;
        self.playing = false;
        self.changes += 1;
    }

    pub fn finish_pose_edit(&mut self, joints: &[usize]) {
        if self.auto_key && !joints.is_empty() {
            self.key_joints(joints);
        }
    }

    pub fn insert_keys(&mut self) {
        let joints: Vec<usize> = if self.parts.is_empty() { (0..self.anim.tracks.len()).collect() } else { self.parts.iter().copied().collect() };
        self.key_joints(&joints);
        self.status = format!("Keyed {} part{} at frame {}", joints.len(), if joints.len() == 1 { "" } else { "s" }, self.current());
    }

    fn key_joints(&mut self, joints: &[usize]) {
        let poses = self.poses();
        let frame = self.current();
        self.checkpoint();
        for &j in joints {
            self.anim.tracks[j].set(frame, poses[j], self.new_handle);
            self.overrides[j] = None;
        }
        self.frame = frame as f32;
    }

    pub fn clear_pose(&mut self, rotation: bool) {
        let poses = self.poses();
        let joints: Vec<usize> = self.parts.iter().copied().collect();
        for &j in &joints {
            let mut pose = poses[j];
            if rotation {
                pose.rot = glam::Quat::IDENTITY;
            } else {
                pose.pos = glam::Vec3::ZERO;
            }
            self.overrides[j] = Some(pose);
        }
        self.finish_pose_edit(&joints);
    }

    pub fn copy_pose(&mut self) {
        let poses = self.poses();
        self.copied = vec![None; poses.len()];
        for &j in &self.parts {
            self.copied[j] = Some(poses[j]);
        }
        let count = self.parts.len();
        self.status = format!("Copied pose of {count} part{}", if count == 1 { "" } else { "s" });
    }

    pub fn paste_pose(&mut self, mirrored: bool) {
        let mut joints = Vec::new();
        for (j, pose) in self.copied.iter().enumerate() {
            let Some(pose) = *pose else { continue };
            let (target, pose) = if mirrored { (self.rig.joints[j].mirror, self.rig.mirrored(j, pose)) } else { (j, pose) };
            self.overrides[target] = Some(pose);
            joints.push(target);
        }
        if joints.is_empty() {
            self.status = "Nothing copied yet. Select parts and press Ctrl+C".to_string();
            return;
        }
        self.finish_pose_edit(&joints);
        self.status = if mirrored { "Pasted mirrored pose".to_string() } else { "Pasted pose".to_string() };
    }

    pub fn copy_keys(&mut self) {
        self.key_clipboard = KeyClipboard::copy(&self.anim, &self.keys);
        let count = self.key_clipboard.len();
        self.status = format!("Copied {count} key{}", if count == 1 { "" } else { "s" });
    }

    pub fn paste_keys(&mut self) {
        if self.key_clipboard.is_empty() {
            self.status = "No keys copied yet. Select keys and press Ctrl+C".to_string();
            return;
        }
        self.checkpoint();
        let at = self.current();
        self.keys = self.key_clipboard.paste(&mut self.anim, at);
        self.clear_overrides();
        let count = self.keys.len();
        self.status = format!("Pasted {count} key{} at frame {}", if count == 1 { "" } else { "s" }, self.current());
    }

    pub fn delete_keys(&mut self) {
        if self.keys.is_empty() {
            return;
        }
        self.checkpoint();
        self.anim.delete(&self.keys);
        self.status = format!("Deleted {} key{}", self.keys.len(), if self.keys.len() == 1 { "" } else { "s" });
        self.keys.clear();
    }

    pub fn jump_key(&mut self, forward: bool) {
        let frames = self.anim.key_frames();
        let now = self.current();
        let next = if forward { frames.range(now + 1..).next() } else { frames.range(..now).next_back() };
        if let Some(&frame) = next {
            self.set_frame(frame as f32);
        }
    }
}

fn partner_rig(anim: &Animation) -> Option<Rig> {
    anim.partner.as_ref().map(|partner| Rig::new(&partner.props))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> Editor {
        let mut ed = Editor::new(Animation::fresh());
        ed.add_partner();
        ed
    }

    #[test]
    fn switching_edits_the_other_rig_and_moves_the_camera_stand() {
        let mut ed = pair();
        let stand = ed.switch_rig().unwrap();
        assert!(ed.anim.second);
        assert_eq!(stand, ed.anim.at);
        ed.checkpoint();
        ed.anim.tracks[2].set(10, Pose::IDENTITY, Handle::AutoClamped);
        assert!(ed.rig_for(true).unwrap().0.tracks[2].key_at(10).is_some());
        assert!(ed.rig_for(false).unwrap().0.tracks[2].key_at(10).is_none());
        assert!(ed.partner_rig.is_some());
    }

    #[test]
    fn undo_and_redo_stay_on_the_rig_being_edited() {
        let mut ed = pair();
        ed.checkpoint();
        ed.anim.tracks[2].set(10, Pose::IDENTITY, Handle::AutoClamped);
        ed.switch_rig();
        ed.checkpoint();
        ed.anim.tracks[3].set(5, Pose::IDENTITY, Handle::AutoClamped);
        ed.undo();
        assert!(ed.anim.second);
        assert!(ed.anim.tracks[3].key_at(5).is_none());
        ed.undo();
        assert!(ed.anim.second);
        assert!(ed.rig_for(false).unwrap().0.tracks[2].key_at(10).is_none());
        ed.redo();
        ed.redo();
        assert!(ed.anim.second);
        assert!(ed.anim.tracks[3].key_at(5).is_some());
        assert!(ed.rig_for(false).unwrap().0.tracks[2].key_at(10).is_some());
    }

    #[test]
    fn key_both_keys_each_rig_once() {
        let mut ed = pair();
        ed.frame = 12.0;
        ed.key_both();
        assert!(ed.anim.tracks.iter().all(|t| t.key_at(12).is_some()));
        assert!(ed.anim.partner.as_ref().unwrap().tracks.iter().all(|t| t.key_at(12).is_some()));
        ed.undo();
        assert!(ed.anim.tracks.iter().all(|t| t.key_at(12).is_none()));
        assert!(ed.anim.partner.as_ref().unwrap().tracks.iter().all(|t| t.key_at(12).is_none()));
    }

    #[test]
    fn a_lone_rig_has_nothing_to_switch_to() {
        let mut ed = Editor::new(Animation::fresh());
        assert!(ed.switch_rig().is_none());
        assert!(ed.rig_for(true).is_none());
    }
}
