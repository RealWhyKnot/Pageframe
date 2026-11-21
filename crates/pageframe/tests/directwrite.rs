#![cfg(target_os = "windows")]

use anyrender::{ImageRenderer, render_to_buffer};
use anyrender_vello_cpu::VelloCpuImageRenderer;
use pageframe::App;
use pageframe_paint::paint_scene;
use pageframe_test_harness::Harness;
use pageframe_traits::shell::Viewport;

const ROWS: &str = r#"<!doctype html><meta charset=utf-8><style>*{margin:0}body{background:white;color:black}div{font:14px/24px "MS Gothic"}p{font:16px/24px Arial}</style><div>あいabc漢字</div><p>g</p>"#;

struct Row {
    html: String,
    viewport_width: u32,
    scale: u16,
    top: u32,
    png: &'static [u8],
}

fn ms_gothic_bitmap() -> Row {
    Row {
        html: ROWS.into(),
        viewport_width: 120,
        scale: 1,
        top: 0,
        png: include_bytes!("fixtures/directwrite/ms_gothic_14.png"),
    }
}

fn arial_outline() -> Row {
    Row {
        html: ROWS.into(),
        viewport_width: 120,
        scale: 1,
        top: 24,
        png: include_bytes!("fixtures/directwrite/arial_16.png"),
    }
}

fn ms_gothic_double() -> Row {
    Row {
        html: ROWS.into(),
        viewport_width: 120,
        scale: 2,
        top: 0,
        png: include_bytes!("fixtures/directwrite/ms_gothic_28.png"),
    }
}

fn web_consolas() -> Row {
    let windows = std::env::var_os("WINDIR").expect("Windows directory");
    let font = std::path::Path::new(&windows).join("Fonts/consola.ttf");
    let url = url::Url::from_file_path(font).expect("absolute font path");
    Row {
        html: format!(
            r#"<!doctype html><style>@font-face{{font-family:FileProbe;src:url("{url}")}}*{{margin:0}}body{{background:white}}div{{font:18px/32px FileProbe,serif}}</style><div>Minimum glyph 0123</div>"#
        ),
        viewport_width: 200,
        scale: 1,
        top: 0,
        png: include_bytes!("fixtures/directwrite/web_consolas_18.png"),
    }
}

fn rotated_arial() -> Row {
    Row {
        html: r#"<!doctype html><style>*{margin:0}body{background:white}div{position:absolute;left:8px;top:0;font:18px/24px Arial;transform-origin:0 0;transform:rotate(7deg)}</style><div>Glyph 0123</div>"#.into(),
        viewport_width: 120,
        scale: 1,
        top: 0,
        png: include_bytes!("fixtures/directwrite/arial_rotate_7.png"),
    }
}

fn check_row<R: ImageRenderer>(row: &Row) {
    let scale = u32::from(row.scale);
    let width = row.viewport_width * scale;
    let height = 60 * scale;
    let page = App::new((), row.html.as_str())
        .page(row.viewport_width, 60)
        .expect("fixture page opens");
    let mut harness = Harness::wrap(page);
    harness.pump();
    {
        let mut doc = harness.base_mut();
        let scheme = doc.get_viewport().color_scheme;
        doc.set_viewport(Viewport::new(width, height, f32::from(row.scale), scheme));
    }
    harness.pump();
    let actual = render_to_buffer::<R, _>(
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
    let expected = image::load_from_memory(row.png)
        .expect("Chrome row is a PNG")
        .to_rgba8();
    let mut mismatches = Vec::new();
    for (x, y, pixel) in expected.enumerate_pixels() {
        let offset =
            usize::try_from(((row.top * scale + y) * width + x) * 4).expect("row fits the capture");
        if actual[offset..offset + 4] != pixel.0 {
            mismatches.push((x, y, &actual[offset..offset + 4], pixel.0));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} pixels differ from Chrome, first {:?}",
        mismatches.len(),
        mismatches.first()
    );
}

#[test]
fn ms_gothic_bitmap_matches_chrome() {
    check_row::<VelloCpuImageRenderer>(&ms_gothic_bitmap());
}

#[test]
fn arial_outline_matches_chrome() {
    check_row::<VelloCpuImageRenderer>(&arial_outline());
}

#[test]
fn ms_gothic_outline_at_double_scale_matches_chrome() {
    check_row::<VelloCpuImageRenderer>(&ms_gothic_double());
}

#[test]
fn web_font_matches_chrome() {
    check_row::<VelloCpuImageRenderer>(&web_consolas());
}

#[test]
fn rotated_arial_matches_chrome() {
    check_row::<VelloCpuImageRenderer>(&rotated_arial());
}

#[cfg(feature = "gpu")]
mod gpu {
    use super::*;
    use anyrender_vello::{VelloImageRenderer, wgpu};

    fn check_row(row: &Row) {
        let instance = wgpu::Instance::default();
        if pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .is_err()
        {
            return;
        }
        super::check_row::<VelloImageRenderer>(row);
    }

    #[test]
    fn ms_gothic_bitmap_matches_chrome() {
        check_row(&ms_gothic_bitmap());
    }

    #[test]
    fn arial_outline_matches_chrome() {
        check_row(&arial_outline());
    }

    #[test]
    fn ms_gothic_outline_at_double_scale_matches_chrome() {
        check_row(&ms_gothic_double());
    }

    #[test]
    fn web_font_matches_chrome() {
        check_row(&web_consolas());
    }

    #[test]
    fn rotated_arial_matches_chrome() {
        check_row(&rotated_arial());
    }
}
