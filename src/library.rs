use crate::anim::Animation;
use crate::project::{data_dir, load_animation, save_animation};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const EXTENSION: &str = "wanim";
const DEFAULT_NAME: &str = "Animation";

pub struct Saved {
    pub name: String,
    pub path: PathBuf,
    pub anim: Animation,
}

pub fn folder() -> PathBuf {
    let path = data_dir().join("library");
    let _ = std::fs::create_dir_all(&path);
    path
}

pub fn list(folder: &Path) -> Vec<Saved> {
    let mut found: Vec<(SystemTime, Saved)> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == EXTENSION))
        .filter_map(|path| {
            let anim = load_animation(&path).ok()?;
            let modified = path.metadata().and_then(|m| m.modified()).ok()?;
            let name = path.file_stem()?.to_string_lossy().to_string();
            Some((modified, Saved { name, path, anim }))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.into_iter().map(|(_, saved)| saved).collect()
}

pub fn save(folder: &Path, name: &str, anim: &Animation) -> io::Result<Saved> {
    let path = unique_path(folder, name);
    save_animation(&path, anim)?;
    Ok(Saved { name: stem(&path), path, anim: anim.clone() })
}

pub fn rename(path: &Path, name: &str) -> io::Result<PathBuf> {
    let target = path.with_file_name(format!("{}.{EXTENSION}", file_name(name)));
    let same_file = target.to_string_lossy().to_lowercase() == path.to_string_lossy().to_lowercase();
    if target.exists() && !same_file {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("\"{}\" is already in the library", stem(&target))));
    }
    if target != path {
        std::fs::rename(path, &target)?;
    }
    Ok(target)
}

pub fn stem(path: &Path) -> String {
    path.file_stem().map_or(DEFAULT_NAME.to_string(), |s| s.to_string_lossy().to_string())
}

fn file_name(text: &str) -> String {
    let cleaned: String = text.chars().filter(|c| !c.is_control() && !"\\/:*?\"<>|".contains(*c)).collect();
    let trimmed = cleaned.trim().trim_end_matches('.').trim();
    if trimmed.is_empty() { DEFAULT_NAME.to_string() } else { trimmed.to_string() }
}

fn unique_path(folder: &Path, name: &str) -> PathBuf {
    let base = file_name(name);
    (1..)
        .map(|n| folder.join(if n == 1 { format!("{base}.{EXTENSION}") } else { format!("{base} {n}.{EXTENSION}") }))
        .find(|path| !path.exists())
        .expect("an unused name")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::Handle;
    use crate::rig::Pose;

    fn scratch(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("welffi_library_{name}"));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn names_lose_characters_windows_rejects() {
        assert_eq!(file_name("  Walk: fast/slow?.  "), "Walk fastslow");
        assert_eq!(file_name("***"), DEFAULT_NAME);
        assert_eq!(file_name(""), DEFAULT_NAME);
    }

    #[test]
    fn saving_twice_never_overwrites() {
        let folder = scratch("unique");
        let anim = Animation::default();
        let first = save(&folder, "Walk", &anim).unwrap();
        let second = save(&folder, "Walk", &anim).unwrap();
        assert_eq!((first.name.as_str(), second.name.as_str()), ("Walk", "Walk 2"));
        let _ = std::fs::remove_dir_all(folder);
    }

    #[test]
    fn saved_animations_come_back_newest_first() {
        let folder = scratch("list");
        let mut anim = Animation::default();
        anim.tracks[1].set(3, Pose::IDENTITY, Handle::AutoClamped);
        save(&folder, "Old", &Animation::default()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        save(&folder, "New", &anim).unwrap();
        std::fs::write(folder.join("notes.txt"), "not an animation").unwrap();
        std::fs::write(folder.join("broken.wanim"), "{").unwrap();
        let found = list(&folder);
        assert_eq!(found.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["New", "Old"]);
        assert_eq!(found[0].anim, anim);
        let _ = std::fs::remove_dir_all(folder);
    }

    #[test]
    fn renaming_refuses_a_name_in_use() {
        let folder = scratch("rename");
        let anim = Animation::default();
        let walk = save(&folder, "Walk", &anim).unwrap();
        save(&folder, "Run", &anim).unwrap();
        assert!(rename(&walk.path, "run").is_err());
        let renamed = rename(&walk.path, "Walk").unwrap();
        assert_eq!(renamed, walk.path);
        let renamed = rename(&walk.path, "Stroll").unwrap();
        assert!(renamed.exists() && !walk.path.exists());
        let _ = std::fs::remove_dir_all(folder);
    }
}
