use crate::clip::{shrink, Clip};
use crate::clip_video;
use eframe::egui;
use image::codecs::gif::GifDecoder;
use image::AnimationDecoder;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

pub const EXTENSIONS: [&str; 7] = ["gif", "mp4", "m4v", "mov", "wmv", "avi", "webm"];
const MIN_GIF_SECONDS: f64 = 0.02;
const DEFAULT_GIF_SECONDS: f64 = 0.1;

enum Message {
    Frame { start: f64, duration: f64, size: [usize; 2], rgba: Vec<u8> },
    Done(Result<(), String>),
}

pub struct Loader {
    receiver: Receiver<Message>,
}

impl Loader {
    pub fn start(path: PathBuf, ctx: egui::Context) -> Loader {
        let (sender, receiver) = channel();
        std::thread::spawn(move || {
            let emit = |start, duration, size, rgba: Vec<u8>| {
                let (rgba, size) = shrink(rgba, size);
                let sent = sender.send(Message::Frame { start, duration, size, rgba }).is_ok();
                ctx.request_repaint();
                sent
            };
            let result = if is_gif(&path) { decode_gif(&path, emit) } else { clip_video::decode(&path, emit) };
            let _ = sender.send(Message::Done(result));
            ctx.request_repaint();
        });
        Loader { receiver }
    }

    pub fn poll(&self, clip: &mut Clip) -> Option<String> {
        for message in self.receiver.try_iter() {
            match message {
                Message::Frame { start, duration, size, rgba } => {
                    if !clip.push(start, duration, size, rgba) {
                        clip.finished = true;
                        return None;
                    }
                }
                Message::Done(result) => {
                    clip.finished = true;
                    return result.err().filter(|_| clip.frames.is_empty());
                }
            }
        }
        None
    }
}

pub fn is_media(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn is_gif(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("gif"))
}

fn gif_seconds(milliseconds: f64) -> f64 {
    if milliseconds / 1000.0 < MIN_GIF_SECONDS { DEFAULT_GIF_SECONDS } else { milliseconds / 1000.0 }
}

fn decode_gif(path: &Path, mut emit: impl FnMut(f64, f64, [usize; 2], Vec<u8>) -> bool) -> Result<(), String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let decoder = GifDecoder::new(BufReader::new(file)).map_err(|e| e.to_string())?;
    let mut start = 0.0;
    for frame in decoder.into_frames() {
        let frame = frame.map_err(|e| e.to_string())?;
        let (numer, denom) = frame.delay().numer_denom_ms();
        let duration = gif_seconds(numer as f64 / denom.max(1) as f64);
        let image = frame.into_buffer();
        let size = [image.width() as usize, image.height() as usize];
        if !emit(start, duration, size, image.into_raw()) {
            return Ok(());
        }
        start += duration;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gif_delays_that_are_too_small_use_the_usual_tenth_of_a_second() {
        assert_eq!(gif_seconds(0.0), 0.1);
        assert_eq!(gif_seconds(10.0), 0.1);
        assert_eq!(gif_seconds(40.0), 0.04);
    }

    #[test]
    fn only_gifs_and_common_videos_count_as_media() {
        assert!(is_media(Path::new("a/b.GIF")));
        assert!(is_media(Path::new("clip.mp4")));
        assert!(!is_media(Path::new("song.mp3")));
        assert!(!is_media(Path::new("pose.wanim")));
        assert!(!is_media(Path::new("noextension")));
    }
}
