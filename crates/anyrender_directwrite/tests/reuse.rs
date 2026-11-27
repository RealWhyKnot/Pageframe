#![cfg(target_os = "windows")]

mod support;

use anyrender::{Glyph, NullScenePainter, PaintScene};
use anyrender_directwrite::{FontSource, GlyphRun, supported};
use kurbo::Affine;
use peniko::{Color, FontData};
use std::sync::Barrier;
use std::thread;
use support::{Recorder, system_font};

fn frame(scene: &mut Recorder, font: &FontData, colors: &[Color], oblique: bool) -> Vec<u64> {
    scene.reset();
    for (row, &color) in (0u8..).zip(colors) {
        let glyphs = (36..44).map(|id| Glyph {
            id,
            x: f32::from(u8::try_from(id - 36).unwrap_or(0)) * 9.25,
            y: 0.0,
        });
        let run = GlyphRun {
            font,
            font_size: 16.0,
            transform: Affine::translate((4.0, 20.0 + f64::from(row) * 20.0)),
            color,
            coords: &[],
            bold: false,
            oblique,
        };
        let source = || {
            Some(FontSource {
                gasp_version: None,
                ranges: Vec::new(),
                strikes: Vec::new(),
                hinted: true,
                axes: Box::new(|_| Vec::new()),
            })
        };
        assert!(run.draw(scene, source, glyphs), "row {row} is painted");
    }
    scene.images.iter().map(|image| image.data.id()).collect()
}

#[test]
fn probe_tells_native_scenes_apart() {
    let font = system_font("arial.ttf");
    assert!(supported(&mut Recorder::default(), &font));
    assert!(!supported(&mut NullScenePainter, &font));
    assert!(!supported(&mut anyrender::Scene::new(), &font));
}

#[test]
fn repeated_frames_reuse_every_glyph_image() {
    let font = system_font("arial.ttf");
    let colors = [
        Color::BLACK,
        Color::from_rgb8(200, 30, 30),
        Color::BLACK,
        Color::from_rgb8(30, 30, 200),
        Color::from_rgb8(30, 140, 30),
        Color::from_rgb8(90, 90, 90),
        Color::from_rgb8(220, 120, 0),
    ];
    let mut scene = Recorder::default();
    let first = frame(&mut scene, &font, &colors, false);
    let second = frame(&mut scene, &font, &colors, false);
    assert_eq!(first.len(), colors.len() * 8);
    assert_eq!(first, second);
    assert_eq!(first[..8], first[16..24]);
    assert!(first[..8].iter().all(|id| !first[8..16].contains(id)));
}

#[test]
fn synthetic_oblique_runs_get_their_own_glyph_images() {
    let font = system_font("arial.ttf");
    let mut scene = Recorder::default();
    let upright = frame(&mut scene, &font, &[Color::BLACK], false);
    let oblique = frame(&mut scene, &font, &[Color::BLACK], true);
    assert!(upright.iter().all(|id| !oblique.contains(id)));
}

#[test]
fn memory_fonts_paint_on_every_thread() {
    let font = system_font("arial.ttf");
    let barrier = Barrier::new(2);
    let render = || {
        barrier.wait();
        frame(&mut Recorder::default(), &font, &[Color::BLACK], false)
    };
    thread::scope(|scope| {
        let other = scope.spawn(render);
        assert_eq!(render().len(), 8);
        assert_eq!(other.join().expect("render thread finishes").len(), 8);
    });
}
