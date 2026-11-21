#![cfg(target_os = "windows")]

mod support;

use anyrender::Glyph;
use anyrender_directwrite::{FontSource, GlyphRun};
use kurbo::Affine;
use peniko::Color;
use support::{Recorder, system_font};

const VERDANA_V_20PX: [u8; 225] = [
    0, 133, 255, 128, 0, 0, 0, 0, 0, 0, 0, 4, 167, 254, 68, 0, 57, 249, 203, 17, 0, 0, 0, 0, 0, 0,
    34, 231, 203, 17, 0, 15, 200, 251, 60, 0, 0, 0, 0, 0, 0, 91, 255, 128, 0, 0, 0, 128, 255, 146,
    0, 0, 0, 0, 0, 8, 181, 249, 57, 0, 0, 0, 51, 245, 213, 20, 0, 0, 0, 0, 41, 237, 200, 15, 0, 0,
    0, 9, 184, 254, 68, 0, 0, 0, 0, 103, 255, 119, 0, 0, 0, 0, 0, 108, 255, 148, 1, 0, 0, 9, 184,
    239, 44, 0, 0, 0, 0, 0, 41, 237, 215, 23, 0, 0, 44, 239, 184, 9, 0, 0, 0, 0, 0, 4, 172, 254,
    72, 0, 0, 108, 255, 103, 0, 0, 0, 0, 0, 0, 0, 91, 255, 159, 1, 9, 184, 231, 34, 0, 0, 0, 0, 0,
    0, 0, 33, 228, 224, 27, 51, 245, 172, 4, 0, 0, 0, 0, 0, 0, 0, 4, 167, 255, 88, 119, 255, 88, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 78, 255, 186, 206, 228, 33, 0, 0, 0, 0, 0, 0, 0, 0, 0, 23, 215, 254,
    255, 159, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 148, 255, 254, 72, 0, 0, 0, 0, 0,
];

#[test]
fn installed_fonts_rasterize_like_chrome() {
    let font = system_font("verdana.ttf");
    let mut scene = Recorder::default();
    let run = GlyphRun {
        font: &font,
        font_size: 20.0,
        transform: Affine::translate((0.0, 130.0)),
        color: Color::BLACK,
        coords: &[],
        bold: false,
        oblique: false,
    };
    let source = || {
        Some(FontSource {
            gasp_version: Some(1),
            ranges: vec![(8, 10), (u16::MAX, 15)],
            strikes: Vec::new(),
            hinted: true,
            axes: Box::new(|_| Vec::new()),
        })
    };
    let glyph = Glyph {
        id: 57,
        x: 0.0,
        y: 0.0,
    };
    assert!(run.draw(&mut scene, source, std::iter::once(glyph)));
    let [image] = scene.images.as_slice() else {
        panic!("one glyph image, got {}", scene.images.len());
    };
    assert_eq!((image.width, image.height), (15, 15));
    let (pixels, _) = image.data.as_ref().as_chunks::<4>();
    let alpha: Vec<u8> = pixels.iter().map(|px| px[3]).collect();
    assert_eq!(alpha, VERDANA_V_20PX);
}
