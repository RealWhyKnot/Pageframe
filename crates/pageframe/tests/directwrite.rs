#![cfg(target_os = "windows")]

use anyrender::{ImageRenderer, render_to_buffer};
use anyrender_vello_cpu::VelloCpuImageRenderer;
use pageframe::App;
use pageframe_paint::paint_scene;
use pageframe_test_harness::Harness;
use pageframe_traits::shell::Viewport;

const ROWS: &str = r#"<!doctype html><meta charset=utf-8><style>*{margin:0}body{background:white;color:black}div{font:14px/24px "MS Gothic"}p{font:16px/24px Arial}</style><div>あいabc漢字</div><p>g</p>"#;

const PAGE: &str = "<!doctype html><meta charset=utf-8><style>*{margin:0}body{background:white;color:black}</style>";

#[derive(Clone, Copy)]
struct Slack {
    level: u8,
    pixels: usize,
    max: u8,
}

const EXACT: Slack = Slack {
    level: 0,
    pixels: 0,
    max: 0,
};

struct Row {
    html: String,
    viewport_width: u32,
    scale: u16,
    top: u32,
    png: &'static [u8],
    slack: Slack,
}

fn ms_gothic_bitmap() -> Row {
    Row {
        html: ROWS.into(),
        viewport_width: 120,
        scale: 1,
        top: 0,
        png: include_bytes!("fixtures/directwrite/ms_gothic_14.png"),
        slack: EXACT,
    }
}

fn arial_outline() -> Row {
    Row {
        html: ROWS.into(),
        viewport_width: 120,
        scale: 1,
        top: 24,
        png: include_bytes!("fixtures/directwrite/arial_16.png"),
        slack: EXACT,
    }
}

fn ms_gothic_double() -> Row {
    Row {
        html: ROWS.into(),
        viewport_width: 120,
        scale: 2,
        top: 0,
        png: include_bytes!("fixtures/directwrite/ms_gothic_28.png"),
        slack: EXACT,
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
        slack: EXACT,
    }
}

fn rotated_arial() -> Row {
    Row {
        html: r#"<!doctype html><style>*{margin:0}body{background:white}div{position:absolute;left:8px;top:0;font:18px/24px Arial;transform-origin:0 0;transform:rotate(7deg)}</style><div>Glyph 0123</div>"#.into(),
        viewport_width: 120,
        scale: 1,
        top: 0,
        png: include_bytes!("fixtures/directwrite/arial_rotate_7.png"),
        slack: EXACT,
    }
}

fn styled(body: &str, scale: u16, png: &'static [u8], slack: Slack) -> Row {
    Row {
        html: format!("{PAGE}{body}"),
        viewport_width: 260,
        scale,
        top: 0,
        png,
        slack,
    }
}

const OBLIQUE: &str = r#"<p style="font:italic 16px/24px Sylfaen">Sylfaen italic Hamburg</p><p style="font:oblique 13px/20px 'Ink Free';color:#16a">Ink Free oblique blue</p>"#;

const NEAR: Slack = Slack {
    level: 6,
    pixels: 0,
    max: 6,
};

fn synthetic_oblique() -> Row {
    styled(
        OBLIQUE,
        1,
        include_bytes!("fixtures/directwrite/oblique_sylfaen_16.png"),
        NEAR,
    )
}

fn synthetic_oblique_double() -> Row {
    styled(
        OBLIQUE,
        2,
        include_bytes!("fixtures/directwrite/oblique_sylfaen_32.png"),
        NEAR,
    )
}

fn synthetic_bold() -> Row {
    styled(
        r#"<p style="font:bold 16px/24px Sylfaen">Sylfaen bold Hamburg 0123</p><p style="font:bold 13px/20px 'Ink Free';color:#a11">Ink Free bold red text</p>"#,
        1,
        include_bytes!("fixtures/directwrite/bold_sylfaen_16.png"),
        Slack {
            level: 64,
            pixels: 24,
            max: 128,
        },
    )
}

fn variable_weights() -> Row {
    styled(
        r#"<p style="font:300 16px/24px Bahnschrift">Bahnschrift light 300</p><p style="font:700 16px/24px Bahnschrift">Bahnschrift bold 700</p>"#,
        1,
        include_bytes!("fixtures/directwrite/bahnschrift_weights_16.png"),
        Slack {
            level: 6,
            pixels: 16,
            max: 40,
        },
    )
}

fn georgia_verdana() -> Row {
    styled(
        r#"<p style="font:15px/22px Georgia;color:#a11">Georgia descenders gjpqy</p><p style="font:20px/28px Verdana">Verdana twenty</p>"#,
        1,
        include_bytes!("fixtures/directwrite/georgia_verdana.png"),
        Slack {
            level: 6,
            pixels: 12,
            max: 64,
        },
    )
}

fn render<R: ImageRenderer>(row: &Row) -> Vec<u8> {
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
    render_to_buffer::<R, _>(
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
    )
}

fn check_row<R: ImageRenderer>(row: &Row) {
    let scale = u32::from(row.scale);
    let width = row.viewport_width * scale;
    let actual = render::<R>(row);
    let expected = image::load_from_memory(row.png)
        .expect("Chrome row is a PNG")
        .to_rgba8();
    let mut mismatches = Vec::new();
    let mut worst = 0;
    for (x, y, pixel) in expected.enumerate_pixels() {
        let offset =
            usize::try_from(((row.top * scale + y) * width + x) * 4).expect("row fits the capture");
        let delta = actual[offset..offset + 4]
            .iter()
            .zip(pixel.0)
            .map(|(&a, e)| a.abs_diff(e))
            .max()
            .unwrap_or(0);
        worst = worst.max(delta);
        if delta > row.slack.level {
            mismatches.push((x, y, &actual[offset..offset + 4], pixel.0));
        }
    }
    assert!(
        mismatches.len() <= row.slack.pixels && worst <= row.slack.max,
        "{} pixels differ from Chrome by more than {}, worst {worst}, first {:?}",
        mismatches.len(),
        row.slack.level,
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

#[test]
fn synthetic_oblique_matches_chrome() {
    check_row::<VelloCpuImageRenderer>(&synthetic_oblique());
}

#[test]
fn synthetic_oblique_at_double_scale_matches_chrome() {
    check_row::<VelloCpuImageRenderer>(&synthetic_oblique_double());
}

#[test]
fn synthetic_bold_matches_chrome() {
    check_row::<VelloCpuImageRenderer>(&synthetic_bold());
}

#[test]
fn variable_weights_match_chrome() {
    check_row::<VelloCpuImageRenderer>(&variable_weights());
}

#[test]
fn georgia_and_verdana_match_chrome() {
    check_row::<VelloCpuImageRenderer>(&georgia_verdana());
}

#[test]
fn fresh_factories_render_identically() {
    let frames: Vec<_> = (0..3)
        .map(|_| {
            std::thread::spawn(|| render::<VelloCpuImageRenderer>(&georgia_verdana()))
                .join()
                .expect("render thread finishes")
        })
        .collect();
    assert!(frames.windows(2).all(|pair| pair[0] == pair[1]));
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

    #[test]
    fn synthetic_oblique_matches_chrome() {
        check_row(&synthetic_oblique());
    }

    #[test]
    fn synthetic_oblique_at_double_scale_matches_chrome() {
        check_row(&synthetic_oblique_double());
    }

    #[test]
    fn synthetic_bold_matches_chrome() {
        check_row(&synthetic_bold());
    }

    #[test]
    fn variable_weights_match_chrome() {
        check_row(&variable_weights());
    }

    #[test]
    fn georgia_and_verdana_match_chrome() {
        check_row(&georgia_verdana());
    }
}
