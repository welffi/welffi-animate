use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::core::PCWSTR;

const VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
const TICKS_PER_SECOND: f64 = 10_000_000.0;
const GUESSED_FRAME_SECONDS: f64 = 1.0 / 30.0;

struct Session;

impl Session {
    fn start() -> windows::core::Result<Session> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
        }
        Ok(Session)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
        }
    }
}

struct Format {
    size: [usize; 2],
    stride: i32,
    area: [usize; 4],
}

pub fn decode(path: &Path, mut emit: impl FnMut(f64, f64, [usize; 2], Vec<u8>) -> bool) -> Result<(), String> {
    let _session = Session::start().map_err(|_| "Windows can't start its video player".to_string())?;
    unsafe { read_all(path, &mut emit) }.map_err(|_| "Can't play this file. It needs a video track Windows can decode".to_string())
}

unsafe fn read_all(path: &Path, emit: &mut impl FnMut(f64, f64, [usize; 2], Vec<u8>) -> bool) -> windows::core::Result<()> {
    unsafe {
        let reader = open(path)?;
        let mut format = format_of(&reader)?;
        let mut base = None;
        loop {
            let (mut flags, mut stamp, mut sample) = (0u32, 0i64, None);
            reader.ReadSample(VIDEO, 0, None, Some(&mut flags), Some(&mut stamp), Some(&mut sample))?;
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                return Ok(());
            }
            if flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
                format = format_of(&reader)?;
            }
            let Some(sample) = sample else { continue };
            let seconds = sample.GetSampleDuration().map(|d| d as f64 / TICKS_PER_SECOND).ok().filter(|d| *d > 0.0).unwrap_or(GUESSED_FRAME_SECONDS);
            let buffer = sample.ConvertToContiguousBuffer()?;
            let (mut data, mut length) = (std::ptr::null_mut(), 0u32);
            buffer.Lock(&mut data, None, Some(&mut length))?;
            let rgba = bgra_to_rgba(std::slice::from_raw_parts(data, length as usize), &format);
            buffer.Unlock()?;
            let start = *base.get_or_insert(stamp);
            if let Some(rgba) = rgba {
                if !emit((stamp - start) as f64 / TICKS_PER_SECOND, seconds, [format.area[2], format.area[3]], rgba) {
                    return Ok(());
                }
            }
        }
    }
}

unsafe fn open(path: &Path) -> windows::core::Result<IMFSourceReader> {
    unsafe {
        let mut attributes = None;
        MFCreateAttributes(&mut attributes, 1)?;
        let attributes = attributes.ok_or(windows::core::Error::empty())?;
        attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)?;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let reader = MFCreateSourceReaderFromURL(PCWSTR(wide.as_ptr()), &attributes)?;
        reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
        reader.SetStreamSelection(VIDEO, true)?;
        let wanted = MFCreateMediaType()?;
        wanted.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        wanted.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
        reader.SetCurrentMediaType(VIDEO, None, &wanted)?;
        Ok(reader)
    }
}

unsafe fn format_of(reader: &IMFSourceReader) -> windows::core::Result<Format> {
    unsafe {
        let current = reader.GetCurrentMediaType(VIDEO)?;
        let packed = current.GetUINT64(&MF_MT_FRAME_SIZE)?;
        let size = [(packed >> 32) as usize, (packed & 0xffff_ffff) as usize];
        let stride = current.GetUINT32(&MF_MT_DEFAULT_STRIDE).map_or(-(size[0] as i32 * 4), |s| s as i32);
        let mut blob = [0u8; 16];
        let area = current.GetBlob(&MF_MT_MINIMUM_DISPLAY_APERTURE, &mut blob, None).ok().and_then(|()| display_area(&blob, size)).unwrap_or([0, 0, size[0], size[1]]);
        Ok(Format { size, stride, area })
    }
}

fn display_area(blob: &[u8; 16], size: [usize; 2]) -> Option<[usize; 4]> {
    let short = |at: usize| i16::from_le_bytes([blob[at], blob[at + 1]]) as i64;
    let long = |at: usize| i32::from_le_bytes([blob[at], blob[at + 1], blob[at + 2], blob[at + 3]]) as i64;
    let (x, y, width, height) = (short(2), short(6), long(8), long(12));
    let fits = x >= 0 && y >= 0 && width > 0 && height > 0 && x + width <= size[0] as i64 && y + height <= size[1] as i64;
    fits.then_some([x as usize, y as usize, width as usize, height as usize])
}

fn bgra_to_rgba(bgra: &[u8], format: &Format) -> Option<Vec<u8>> {
    let height = format.size[1];
    let [left, top, width, shown] = format.area;
    let pitch = format.stride.unsigned_abs() as usize;
    let mut rgba = Vec::with_capacity(width * shown * 4);
    for y in top..top + shown {
        let row = if format.stride < 0 { height - 1 - y } else { y };
        let line = bgra.get(row * pitch + left * 4..row * pitch + (left + width) * 4)?;
        for pixel in line.chunks_exact(4) {
            rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
        }
    }
    Some(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottom_up_rows_are_flipped_and_colors_reordered() {
        let top = [1u8, 2, 3, 0];
        let bottom = [4u8, 5, 6, 0];
        let bgra: Vec<u8> = bottom.iter().chain(&top).copied().collect();
        let rgba = bgra_to_rgba(&bgra, &Format { size: [1, 2], stride: -4, area: [0, 0, 1, 2] }).unwrap();
        assert_eq!(rgba, vec![3, 2, 1, 255, 6, 5, 4, 255]);
        let upright = bgra_to_rgba(&bgra, &Format { size: [1, 2], stride: 4, area: [0, 0, 1, 2] }).unwrap();
        assert_eq!(upright, vec![6, 5, 4, 255, 3, 2, 1, 255]);
    }

    #[test]
    fn the_display_area_crops_the_padding_rows() {
        let bgra: Vec<u8> = (0..4u8).flat_map(|row| [row, 0, 0, 0]).collect();
        let format = Format { size: [1, 4], stride: 4, area: [0, 1, 1, 2] };
        assert_eq!(bgra_to_rgba(&bgra, &format).unwrap(), vec![0, 0, 1, 255, 0, 0, 2, 255]);
    }

    #[test]
    fn the_display_area_is_read_from_the_aperture_bytes() {
        let mut blob = [0u8; 16];
        blob[8..12].copy_from_slice(&320i32.to_le_bytes());
        blob[12..16].copy_from_slice(&180i32.to_le_bytes());
        assert_eq!(display_area(&blob, [320, 192]), Some([0, 0, 320, 180]));
        assert_eq!(display_area(&blob, [320, 176]), None);
        assert_eq!(display_area(&[0; 16], [320, 192]), None);
    }

    #[test]
    fn a_short_buffer_gives_no_frame() {
        assert!(bgra_to_rgba(&[0; 4], &Format { size: [1, 2], stride: 4, area: [0, 0, 1, 2] }).is_none());
    }
}
