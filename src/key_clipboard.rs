use crate::anim::{Animation, Key, KeyId};
use std::collections::BTreeSet;

#[derive(Default)]
pub struct KeyClipboard {
    keys: Vec<(usize, Key)>,
}

impl KeyClipboard {
    pub fn copy(anim: &Animation, ids: &BTreeSet<KeyId>) -> KeyClipboard {
        let keys = ids.iter().filter_map(|&(joint, frame)| Some((joint, *anim.tracks.get(joint)?.key_at(frame)?))).collect();
        KeyClipboard { keys }
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn paste(&self, anim: &mut Animation, at: i32) -> BTreeSet<KeyId> {
        let Some(first) = self.keys.iter().map(|(_, key)| key.frame).min() else { return BTreeSet::new() };
        let mut placed = BTreeSet::new();
        for &(joint, mut key) in &self.keys {
            let Some(track) = anim.tracks.get_mut(joint) else { continue };
            key.frame += at - first;
            track.put(key);
            placed.insert((joint, key.frame));
        }
        anim.recalc();
        placed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::Handle;
    use crate::rig::Pose;

    fn keyed(frames: &[(usize, i32)]) -> Animation {
        let mut anim = Animation::default();
        for &(joint, frame) in frames {
            anim.tracks[joint].set(frame, Pose::IDENTITY, Handle::AutoClamped);
        }
        anim
    }

    fn ids(list: &[(usize, i32)]) -> BTreeSet<KeyId> {
        list.iter().copied().collect()
    }

    #[test]
    fn pasting_keeps_the_gaps_between_keys_and_starts_at_the_playhead() {
        let mut anim = keyed(&[(0, 10), (0, 14), (3, 12)]);
        let clip = KeyClipboard::copy(&anim, &ids(&[(0, 10), (0, 14), (3, 12)]));
        let placed = clip.paste(&mut anim, 30);
        assert_eq!(placed, ids(&[(0, 30), (0, 34), (3, 32)]));
        assert!(anim.tracks[0].key_at(34).is_some());
        assert!(anim.tracks[0].key_at(10).is_some());
    }

    #[test]
    fn pasting_over_a_key_replaces_it() {
        let mut anim = keyed(&[(1, 0), (1, 5)]);
        let mut moved = Pose::IDENTITY;
        moved.pos.x = 2.0;
        anim.tracks[1].set(5, moved, Handle::AutoClamped);
        let clip = KeyClipboard::copy(&anim, &ids(&[(1, 5)]));
        clip.paste(&mut anim, 0);
        assert_eq!(anim.tracks[1].keys.len(), 2);
        assert_eq!(anim.tracks[1].key_at(0).unwrap().pose.pos.x, 2.0);
    }

    #[test]
    fn the_copy_keeps_its_style_and_ignores_missing_keys() {
        let mut anim = keyed(&[(2, 4)]);
        anim.tracks[2].keys[0].interp = crate::easing::Interp::Constant;
        let clip = KeyClipboard::copy(&anim, &ids(&[(2, 4), (2, 99), (40, 1)]));
        assert_eq!(clip.len(), 1);
        clip.paste(&mut anim, 20);
        assert_eq!(anim.tracks[2].key_at(20).unwrap().interp, crate::easing::Interp::Constant);
    }

    #[test]
    fn an_empty_clipboard_places_nothing() {
        let mut anim = keyed(&[(0, 0)]);
        assert!(KeyClipboard::default().paste(&mut anim, 5).is_empty());
    }
}
