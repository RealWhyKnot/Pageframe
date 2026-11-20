#![cfg(target_os = "windows")]

use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use pageframe::App;
use pageframe_paint::paint_scene;
use pageframe_test_harness::Harness;
use pageframe_traits::shell::Viewport;

const ROWS: &str = r#"<!doctype html><meta charset=utf-8><style>*{margin:0}body{background:white;color:black}div{font:14px/24px "MS Gothic"}p{font:16px/24px Arial}</style><div>あいabc漢字</div><p>g</p>"#;

fn check_row(scale: u16, top: u32, png: &[u8]) {
    let width = 120 * u32::from(scale);
    let height = 60 * u32::from(scale);
    let page = App::new((), ROWS)
        .page(120, 60)
        .expect("fixture page opens");
    let mut harness = Harness::wrap(page);
    harness.pump();
    {
        let mut doc = harness.base_mut();
        let scheme = doc.get_viewport().color_scheme;
        doc.set_viewport(Viewport::new(width, height, f32::from(scale), scheme));
    }
    harness.pump();
    let actual = render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| {
            paint_scene(
                scene,
                &mut harness.base_mut(),
                scale.into(),
                width,
                height,
                0,
                0,
            )
        },
        width,
        height,
    );
    let expected = image::load_from_memory(png)
        .expect("Chrome row is a PNG")
        .to_rgba8();
    let scale = u32::from(scale);
    for (x, y, pixel) in expected.enumerate_pixels() {
        let offset =
            usize::try_from(((top * scale + y) * width + x) * 4).expect("row fits the capture");
        assert_eq!(&actual[offset..offset + 4], &pixel.0, "pixel ({x}, {y})");
    }
}

#[test]
fn ms_gothic_bitmap_matches_chrome() {
    check_row(
        1,
        0,
        include_bytes!("fixtures/directwrite/ms_gothic_14.png"),
    );
}

#[test]
fn arial_outline_matches_chrome() {
    check_row(1, 24, include_bytes!("fixtures/directwrite/arial_16.png"));
}

#[test]
fn ms_gothic_outline_at_double_scale_matches_chrome() {
    check_row(
        2,
        0,
        include_bytes!("fixtures/directwrite/ms_gothic_28.png"),
    );
}
