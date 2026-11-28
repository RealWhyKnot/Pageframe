use windows::Win32::Media::MediaFoundation::{
    IMF2DBuffer2, IMFSample, IMFSourceReader, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_SIZE,
    MF_MT_MINIMUM_DISPLAY_APERTURE, MF_MT_SUBTYPE, MF_SOURCE_READER_FIRST_VIDEO_STREAM,
    MF2DBuffer_LockFlags_Read, MFVideoFormat_RGB32,
};
use windows_core::Interface;

use super::stream_index;

#[derive(Clone, Copy)]
pub(super) struct VideoFormat {
    width: u32,
    height: u32,
    stride: i32,
    pub(super) crop: (u32, u32, u32, u32),
}

impl VideoFormat {
    pub(super) fn read(reader: &IMFSourceReader) -> windows_core::Result<Self> {
        let stream = stream_index(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0);
        // SAFETY: the reader and the media type it returns are live COM objects; the blob buffer is local.
        unsafe {
            let media_type = reader.GetCurrentMediaType(stream)?;
            if media_type.GetGUID(&MF_MT_SUBTYPE)? != MFVideoFormat_RGB32 {
                return Err(invalid());
            }
            let size = media_type.GetUINT64(&MF_MT_FRAME_SIZE)?;
            let width = u32::try_from(size >> 32).unwrap_or(0);
            let height = u32::try_from(size & 0xffff_ffff).unwrap_or(0);
            let packed = width
                .checked_mul(4)
                .and_then(|bytes| i32::try_from(bytes).ok())
                .filter(|bytes| *bytes > 0 && height > 0)
                .ok_or_else(invalid)?;
            let stride = media_type
                .GetUINT32(&MF_MT_DEFAULT_STRIDE)
                .map(u32::cast_signed)
                .unwrap_or(packed);
            let mut area = [0_u8; 16];
            let crop = match media_type.GetBlob(&MF_MT_MINIMUM_DISPLAY_APERTURE, &mut area, None) {
                Ok(()) => aperture(&area, width, height),
                Err(_) => (0, 0, width, height),
            };
            Ok(Self {
                width,
                height,
                stride,
                crop,
            })
        }
    }
}

fn aperture(area: &[u8; 16], width: u32, height: u32) -> (u32, u32, u32, u32) {
    let x = i16::from_le_bytes([area[2], area[3]]);
    let y = i16::from_le_bytes([area[6], area[7]]);
    let w = i32::from_le_bytes([area[8], area[9], area[10], area[11]]);
    let h = i32::from_le_bytes([area[12], area[13], area[14], area[15]]);
    let x = u32::try_from(x).unwrap_or(0).min(width);
    let y = u32::try_from(y).unwrap_or(0).min(height);
    let w = u32::try_from(w).unwrap_or(0).min(width - x);
    let h = u32::try_from(h).unwrap_or(0).min(height - y);
    if w == 0 || h == 0 {
        (0, 0, width, height)
    } else {
        (x, y, w, h)
    }
}

fn invalid() -> windows_core::Error {
    windows::Win32::Foundation::E_UNEXPECTED.into()
}

fn convert_rows(
    bytes: &[u8],
    first_row: usize,
    pitch: isize,
    format: &VideoFormat,
) -> Option<Vec<u8>> {
    let (x, y, width, height) = format.crop;
    let row_bytes = usize::try_from(width).ok()?.checked_mul(4)?;
    let left = usize::try_from(x).ok()?.checked_mul(4)?;
    let stride = pitch.unsigned_abs();
    if stride < usize::try_from(format.width).ok()?.checked_mul(4)? {
        return None;
    }
    let row_at = |row| {
        let offset = isize::try_from(row).ok()?.checked_mul(pitch)?;
        first_row.checked_add_signed(offset)?.checked_add(left)
    };
    let first = row_at(y)?;
    let last = row_at(y.checked_add(height.checked_sub(1)?)?)?;
    bytes.get(first.min(last)..first.max(last).checked_add(row_bytes)?)?;
    let size = row_bytes.checked_mul(usize::try_from(height).ok()?)?;
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(size).ok()?;
    for row in y..y.checked_add(height)? {
        let start = row_at(row)?;
        for pixel in bytes.get(start..start + row_bytes)?.as_chunks::<4>().0 {
            rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
        }
    }
    Some(rgba)
}

pub(super) fn frame_pixels(
    sample: &IMFSample,
    format: &VideoFormat,
) -> windows_core::Result<Vec<u8>> {
    // SAFETY: the returned buffer stays locked until after the bounded byte slice has been copied.
    unsafe {
        let buffer = if sample.GetBufferCount()? == 1 {
            sample.GetBufferByIndex(0)?
        } else {
            sample.ConvertToContiguousBuffer()?
        };
        if let Ok(planar) = buffer.cast::<IMF2DBuffer2>() {
            let mut scanline = std::ptr::null_mut();
            let mut pitch = 0_i32;
            let mut start = std::ptr::null_mut();
            let mut length = 0;
            planar.Lock2DSize(
                MF2DBuffer_LockFlags_Read,
                &raw mut scanline,
                &raw mut pitch,
                &raw mut start,
                &raw mut length,
            )?;
            let length = usize::try_from(length).unwrap_or(0);
            let pixels = if start.is_null() || length > isize::MAX.unsigned_abs() {
                None
            } else {
                scanline.addr().checked_sub(start.addr()).and_then(|first| {
                    convert_rows(
                        std::slice::from_raw_parts(start, length),
                        first,
                        pitch as isize,
                        format,
                    )
                })
            };
            planar.Unlock2D()?;
            return pixels.ok_or_else(invalid);
        }
        let mut data = std::ptr::null_mut();
        let mut length = 0_u32;
        buffer.Lock(&raw mut data, None, Some(&raw mut length))?;
        let pitch = format.stride as isize;
        let length = usize::try_from(length).unwrap_or(0);
        let pixels = if data.is_null() || length > isize::MAX.unsigned_abs() {
            None
        } else {
            let first = if pitch < 0 {
                pitch
                    .unsigned_abs()
                    .checked_mul(usize::try_from(format.height - 1).unwrap_or(0))
            } else {
                Some(0)
            };
            first.and_then(|first| {
                convert_rows(
                    std::slice::from_raw_parts(data, length),
                    first,
                    pitch,
                    format,
                )
            })
        };
        buffer.Unlock()?;
        pixels.ok_or_else(invalid)
    }
}

pub(super) fn audio_frames(
    sample: &IMFSample,
    channels: usize,
) -> windows_core::Result<Vec<[f32; 2]>> {
    // SAFETY: the buffer is locked while its `length` bytes are copied and unlocked afterwards.
    unsafe {
        let buffer = sample.ConvertToContiguousBuffer()?;
        let mut data = std::ptr::null_mut();
        let mut length = 0_u32;
        buffer.Lock(&raw mut data, None, Some(&raw mut length))?;
        let length = usize::try_from(length).unwrap_or(0);
        let Some(frame_bytes) = channels.checked_mul(4).filter(|bytes| *bytes > 0) else {
            buffer.Unlock()?;
            return Err(invalid());
        };
        if length == 0 {
            buffer.Unlock()?;
            return Ok(Vec::new());
        }
        if data.is_null() || length > isize::MAX.unsigned_abs() || length % frame_bytes != 0 {
            buffer.Unlock()?;
            return Err(invalid());
        }
        let bytes = std::slice::from_raw_parts(data.cast_const(), length);
        let frames = bytes
            .chunks_exact(frame_bytes)
            .map(|frame| {
                let left = f32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]);
                let right = if channels >= 2 {
                    f32::from_le_bytes([frame[4], frame[5], frame[6], frame[7]])
                } else {
                    left
                };
                [left, right]
            })
            .collect();
        buffer.Unlock()?;
        Ok(frames)
    }
}
