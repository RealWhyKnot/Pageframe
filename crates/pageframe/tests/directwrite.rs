#![cfg(target_os = "windows")]

use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use pageframe::App;
use pageframe_paint::paint_scene;
use pageframe_test_harness::Harness;
use pageframe_traits::shell::Viewport;

const ROWS: &str = r#"<!doctype html><meta charset=utf-8><style>*{margin:0}body{background:white;color:black}div{font:14px/24px "MS Gothic"}p{font:16px/24px Arial}</style><div>あいabc漢字</div><p>g</p>"#;

fn check_row(html: &str, viewport_width: u32, scale: u16, top: u32, png: &[u8]) {
    let width = viewport_width * u32::from(scale);
    let height = 60 * u32::from(scale);
    let page = App::new((), html)
        .page(viewport_width, 60)
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
        ROWS,
        120,
        1,
        0,
        include_bytes!("fixtures/directwrite/ms_gothic_14.png"),
    );
}

#[test]
fn arial_outline_matches_chrome() {
    check_row(
        ROWS,
        120,
        1,
        24,
        include_bytes!("fixtures/directwrite/arial_16.png"),
    );
}

#[test]
fn ms_gothic_outline_at_double_scale_matches_chrome() {
    check_row(
        ROWS,
        120,
        2,
        0,
        include_bytes!("fixtures/directwrite/ms_gothic_28.png"),
    );
}

#[test]
fn web_font_matches_chrome() {
    let windows = std::env::var_os("WINDIR").expect("Windows directory");
    let font = std::path::Path::new(&windows).join("Fonts/consola.ttf");
    let url = url::Url::from_file_path(font).expect("absolute font path");
    let html = format!(
        r#"<!doctype html><style>@font-face{{font-family:FileProbe;src:url("{url}")}}*{{margin:0}}body{{background:white}}div{{font:18px/32px FileProbe,serif}}</style><div>Minimum glyph 0123</div>"#
    );
    check_row(
        &html,
        200,
        1,
        0,
        include_bytes!("fixtures/directwrite/web_consolas_18.png"),
    );
}

#[test]
fn rotated_arial_matches_chrome() {
    let html = r#"<!doctype html><style>*{margin:0}body{background:white}div{position:absolute;left:8px;top:0;font:18px/24px Arial;transform-origin:0 0;transform:rotate(7deg)}</style><div>Glyph 0123</div>"#;
    check_row(
        html,
        120,
        1,
        0,
        include_bytes!("fixtures/directwrite/arial_rotate_7.png"),
    );
}
