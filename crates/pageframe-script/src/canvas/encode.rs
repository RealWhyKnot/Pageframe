use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::object::builtins::JsArray;
use boa_engine::value::JsValue;
use boa_engine::{Context, JsNativeError, JsResult, JsString};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder};
use pageframe_dom::canvas::Canvas2D;
use pageframe_dom::local_name;

use super::{bitmap_size, bytes_value, canvas_of};
use crate::dom::{dom_ctx, node_id_of_value, to_rust_string};

const DEFAULT_JPEG_QUALITY: f64 = 0.92;
const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Format {
    Png,
    Jpeg,
    Webp,
}

impl Format {
    pub(crate) fn from_mime(mime: &str) -> Self {
        match mime.to_ascii_lowercase().as_str() {
            "image/jpeg" => Self::Jpeg,
            "image/webp" => Self::Webp,
            _ => Self::Png,
        }
    }

    pub(crate) fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
        }
    }
}

fn jpeg_quality(quality: Option<f64>) -> u8 {
    let quality = quality
        .filter(|q| (0.0..=1.0).contains(q))
        .unwrap_or(DEFAULT_JPEG_QUALITY);
    (quality * 100.0).round().clamp(1.0, 100.0) as u8
}

pub(crate) fn encode(canvas: &Canvas2D, format: Format, quality: Option<f64>) -> Option<Vec<u8>> {
    let (width, height) = (canvas.width(), canvas.height());
    if width == 0 || height == 0 {
        return None;
    }
    let mut out = Vec::new();
    let result = match format {
        Format::Png => PngEncoder::new(&mut out).write_image(
            &canvas.to_straight_rgba(),
            width,
            height,
            ExtendedColorType::Rgba8,
        ),
        Format::Webp => WebPEncoder::new_lossless(&mut out).write_image(
            &canvas.to_straight_rgba(),
            width,
            height,
            ExtendedColorType::Rgba8,
        ),
        Format::Jpeg => {
            let rgb: Vec<u8> = canvas
                .premultiplied_pixels()
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|px| [px[0], px[1], px[2]])
                .collect();
            JpegEncoder::new_with_quality(&mut out, jpeg_quality(quality)).write_image(
                &rgb,
                width,
                height,
                ExtendedColorType::Rgb8,
            )
        }
    };
    result.ok().map(|()| out)
}

pub(crate) fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(BASE64[(n >> (18 - 6 * i) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn source_canvas(value: &JsValue, context: &mut Context) -> JsResult<Rc<RefCell<Canvas2D>>> {
    if let Ok(canvas) = canvas_of(value) {
        return Ok(canvas);
    }
    let ctx = dom_ctx(context)?;
    let node_id = node_id_of_value(value)
        .filter(|id| {
            ctx.doc
                .borrow()
                .get_node(*id)
                .and_then(|n| n.element_data())
                .is_some_and(|e| e.name.local == local_name!("canvas"))
        })
        .ok_or_else(|| JsNativeError::typ().with_message("Illegal invocation"))?;
    if let Some((_, canvas)) = ctx.state.borrow().canvas.contexts.get(&node_id) {
        return Ok(Rc::clone(canvas));
    }
    let (w, h) = bitmap_size(&ctx, node_id);
    Ok(Rc::new(RefCell::new(Canvas2D::new(w, h))))
}

fn encode_args(
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<(Rc<RefCell<Canvas2D>>, Format, Option<f64>)> {
    let canvas = source_canvas(&args.first().cloned().unwrap_or_default(), context)?;
    let format = match args.get(1) {
        Some(value) if !value.is_undefined() => Format::from_mime(&to_rust_string(value, context)?),
        _ => Format::Png,
    };
    let quality = args.get(2).and_then(JsValue::as_number);
    Ok((canvas, format, quality))
}

pub(super) fn to_data_url(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let (canvas, format, quality) = encode_args(args, context)?;
    let url = match encode(&canvas.borrow(), format, quality) {
        Some(bytes) => format!("data:{};base64,{}", format.mime(), base64(&bytes)),
        None => "data:,".to_string(),
    };
    Ok(JsString::from(url).into())
}

pub(super) fn to_bytes(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let (canvas, format, quality) = encode_args(args, context)?;
    let Some(bytes) = encode(&canvas.borrow(), format, quality) else {
        return Ok(JsValue::null());
    };
    let buffer = bytes_value(&bytes, context);
    Ok(JsArray::from_iter([JsString::from(format.mime()).into(), buffer], context).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_reference_vectors() {
        let cases = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (input, expected) in cases {
            assert_eq!(base64(input.as_bytes()), expected);
        }
        assert_eq!(base64(&[0xff, 0xfe, 0xfd, 0x00]), "//79AA==");
    }

    #[test]
    fn mime_types_are_case_insensitive_with_png_fallback() {
        assert_eq!(Format::from_mime("IMAGE/JPEG"), Format::Jpeg);
        assert_eq!(Format::from_mime("image/WebP"), Format::Webp);
        assert_eq!(Format::from_mime("image/gif"), Format::Png);
        assert_eq!(Format::from_mime(""), Format::Png);
    }

    #[test]
    fn jpeg_quality_defaults_and_clamps() {
        assert_eq!(jpeg_quality(None), 92);
        assert_eq!(jpeg_quality(Some(1.5)), 92);
        assert_eq!(jpeg_quality(Some(-0.1)), 92);
        assert_eq!(jpeg_quality(Some(0.0)), 1);
        assert_eq!(jpeg_quality(Some(0.5)), 50);
        assert_eq!(jpeg_quality(Some(1.0)), 100);
    }

    #[test]
    fn encoders_emit_format_signatures() {
        let canvas = Canvas2D::new(3, 2);
        let png = encode(&canvas, Format::Png, None).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let jpeg = encode(&canvas, Format::Jpeg, Some(0.5)).unwrap();
        assert_eq!(&jpeg[..3], &[0xff, 0xd8, 0xff]);
        assert_eq!(&jpeg[jpeg.len() - 2..], &[0xff, 0xd9]);
        let webp = encode(&canvas, Format::Webp, None).unwrap();
        assert_eq!(&webp[..4], b"RIFF");
        assert_eq!(&webp[8..12], b"WEBP");
        assert!(encode(&Canvas2D::new(0, 4), Format::Png, None).is_none());
    }
}
