const MAX_SIDE: usize = 640;
const MAX_BYTES: usize = 400 * 1024 * 1024;

pub struct Frame {
    pub start: f64,
    pub size: [usize; 2],
    pub rgba: Vec<u8>,
}

#[derive(Default)]
pub struct Clip {
    pub frames: Vec<Frame>,
    pub end: f64,
    pub finished: bool,
    pub cut_short: bool,
    bytes: usize,
}

impl Clip {
    pub fn push(&mut self, start: f64, duration: f64, size: [usize; 2], rgba: Vec<u8>) -> bool {
        if self.bytes + rgba.len() > MAX_BYTES {
            self.cut_short = true;
            return false;
        }
        self.bytes += rgba.len();
        self.end = start + duration;
        self.frames.push(Frame { start, size, rgba });
        true
    }

    pub fn index_at(&self, time: f64) -> usize {
        self.frames.partition_point(|f| f.start <= time).saturating_sub(1)
    }

    pub fn neighbor_start(&self, time: f64, forward: bool) -> f64 {
        let index = self.index_at(time);
        let next = if forward { (index + 1).min(self.frames.len().saturating_sub(1)) } else { index.saturating_sub(1) };
        self.frames.get(next).map_or(0.0, |f| f.start)
    }
}

pub fn shrink(rgba: Vec<u8>, size: [usize; 2]) -> (Vec<u8>, [usize; 2]) {
    let [width, height] = size;
    let longest = width.max(height);
    if longest <= MAX_SIDE || width == 0 || height == 0 {
        return (rgba, size);
    }
    let out = [(width * MAX_SIDE / longest).max(1), (height * MAX_SIDE / longest).max(1)];
    let mut shrunk = Vec::with_capacity(out[0] * out[1] * 4);
    for y in 0..out[1] {
        let rows = y * height / out[1]..((y + 1) * height / out[1]).max(y * height / out[1] + 1);
        for x in 0..out[0] {
            let columns = x * width / out[0]..((x + 1) * width / out[0]).max(x * width / out[0] + 1);
            let mut sum = [0u32; 4];
            for row in rows.clone() {
                for column in columns.clone() {
                    let at = (row * width + column) * 4;
                    for (total, value) in sum.iter_mut().zip(&rgba[at..at + 4]) {
                        *total += *value as u32;
                    }
                }
            }
            let count = (rows.len() * columns.len()) as u32;
            shrunk.extend(sum.iter().map(|total| (total / count) as u8));
        }
    }
    (shrunk, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(starts: &[f64]) -> Clip {
        let mut clip = Clip::default();
        for (i, start) in starts.iter().enumerate() {
            let next = starts.get(i + 1).copied().unwrap_or(start + 0.1);
            clip.push(*start, next - start, [1, 1], vec![0; 4]);
        }
        clip
    }

    #[test]
    fn the_frame_shown_is_the_last_one_that_has_started() {
        let clip = clip(&[0.0, 0.1, 0.5]);
        assert_eq!(clip.index_at(0.0), 0);
        assert_eq!(clip.index_at(0.099), 0);
        assert_eq!(clip.index_at(0.1), 1);
        assert_eq!(clip.index_at(0.49), 1);
        assert_eq!(clip.index_at(9.0), 2);
        assert_eq!(Clip::default().index_at(1.0), 0);
    }

    #[test]
    fn stepping_lands_on_the_neighbor_frame_and_stops_at_the_ends() {
        let clip = clip(&[0.0, 0.1, 0.5]);
        assert_eq!(clip.neighbor_start(0.2, true), 0.5);
        assert_eq!(clip.neighbor_start(0.2, false), 0.0);
        assert_eq!(clip.neighbor_start(0.0, false), 0.0);
        assert_eq!(clip.neighbor_start(0.7, true), 0.5);
    }

    #[test]
    fn the_end_follows_the_last_frame() {
        let clip = clip(&[0.0, 0.1, 0.5]);
        assert!((clip.end - 0.6).abs() < 1e-9);
    }

    #[test]
    fn small_images_stay_and_large_ones_shrink_in_proportion() {
        let (same, size) = shrink(vec![7; 4 * 4 * 4], [4, 4]);
        assert_eq!((same.len(), size), (64, [4, 4]));
        let big = vec![200; 1280 * 720 * 4];
        let (small, size) = shrink(big, [1280, 720]);
        assert_eq!(size, [640, 360]);
        assert_eq!(small.len(), 640 * 360 * 4);
        assert!(small.iter().all(|v| *v == 200));
    }

    #[test]
    fn shrinking_averages_the_pixels_it_merges() {
        let mut rgba = vec![0u8; 1280 * 2 * 4];
        for pixel in rgba.chunks_mut(4).step_by(2) {
            pixel.copy_from_slice(&[100, 100, 100, 255]);
        }
        let (small, size) = shrink(rgba, [1280, 2]);
        assert_eq!(size, [640, 1]);
        assert_eq!(&small[0..4], &[50, 50, 50, 127]);
    }
}
