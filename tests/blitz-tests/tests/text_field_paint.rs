use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_html::HtmlDocument;
use blitz_paint::paint_scene;
use blitz_test_harness::{Harness, HarnessOptions};

const WIDTH: u32 = 400;
const HEIGHT: u32 = 200;

struct Frame {
    pixels: Vec<u8>,
    stride: usize,
}

impl Frame {
    fn red(&self, x: usize, y: usize) -> u8 {
        self.pixels[(y * self.stride + x) * 4]
    }

    fn grid(&self, x: usize, y: usize, size: usize) -> Vec<String> {
        (y..y + size)
            .map(|row| {
                (x..x + size)
                    .map(|column| format!("{:02x}", self.red(column, row)))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect()
    }

    fn dark_rows(&self, x: usize, from: usize, to: usize) -> Vec<usize> {
        (from..to)
            .filter(|&row| self.pixels[(row * self.stride + x) * 4..][..3] == [0, 0, 0])
            .collect()
    }
}

fn render(body: &str, scale: u32) -> Frame {
    let html = format!(r#"<html><body style="margin:0;background:#fff">{body}</body></html>"#);
    let mut harness = Harness::<HtmlDocument>::from_html_with(
        &html,
        HarnessOptions {
            width: WIDTH,
            height: HEIGHT,
            scale: f32::from(u16::try_from(scale).expect("small scale")),
            ..Default::default()
        },
    );
    let (width, height) = (WIDTH * scale, HEIGHT * scale);
    let mut doc = harness.base_mut();
    let field = doc
        .query_selector("#f")
        .expect("valid selector")
        .expect("field");
    doc.set_focus_to(field);
    doc.resolve(0.0);
    let pixels = render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| paint_scene(scene, &mut doc, f64::from(scale), width, height, 0, 0),
        width,
        height,
    );
    Frame {
        pixels,
        stride: usize::try_from(width).expect("fits"),
    }
}

fn caret_rows(body: &str, scale: u32, x: usize) -> (usize, usize) {
    let frame = render(body, scale);
    let rows = frame.dark_rows(x, 0, 100 * usize::try_from(scale).expect("fits"));
    (*rows.first().expect("caret"), *rows.last().expect("caret"))
}

const INPUT: &str = r#"<input id=f style="position:absolute;left:20px;top:20px">"#;
const EMPTY_TEXTAREA: &str =
    r#"<textarea id=f style="position:absolute;left:20px;top:20px;line-height:30px"></textarea>"#;
const TALL_TEXTAREA: &str = r#"<textarea id=f style="position:absolute;left:20px;top:20px;font-size:17px;line-height:25px"></textarea>"#;
const SHIFTED_INPUT: &str =
    r#"<input id=f style="position:absolute;left:20.3px;top:20.6px;font-size:20px">"#;

const RING_1X: [&str; 8] = [
    "ff ff ff ff ff ff ff ff",
    "ff e8 67 25 10 10 10 10",
    "ff 61 10 10 10 10 10 10",
    "ff 1f 10 c4 ff ff ff ff",
    "ff 10 10 ff ff 00 ff ff",
    "ff 10 10 ff ff 00 ff ff",
    "ff 10 10 ff ff 00 ff ff",
    "ff 10 10 ff ff 00 ff ff",
];

const RING_2X: [&str; 10] = [
    "ff ff ff ff ff ff ff ff ff ff",
    "ff ff ff ff ff ff ff ff ff ff",
    "ff ff ff ff ff a7 5c 28 10 10",
    "ff ff ff e8 4d 10 10 10 10 10",
    "ff ff ff 47 10 10 10 10 10 10",
    "ff ff a1 10 10 10 10 10 10 10",
    "ff ff 53 10 10 10 5e d9 ff ff",
    "ff ff 1e 10 10 10 d7 ff ff ff",
    "ff ff 10 10 10 10 ff ff ff ff",
    "ff ff 10 10 10 10 ff ff ff ff",
];

fn levels(row: &str) -> Vec<i16> {
    row.split(' ')
        .map(|level| i16::from_str_radix(level, 16).expect("hex level"))
        .collect()
}

fn assert_near(grid: &[String], expected: &[&str], tolerance: i16) {
    assert_eq!(grid.len(), expected.len());
    for (row, want_row) in grid.iter().zip(expected) {
        for (got, want) in levels(row).into_iter().zip(levels(want_row)) {
            assert!((got - want).abs() <= tolerance, "{grid:#?}");
        }
    }
}

#[test]
fn focus_ring_corner_is_within_skia_antialiasing_at_1x() {
    assert_near(&render(INPUT, 1).grid(19, 19, 8), &RING_1X, 3);
}

#[test]
fn focus_ring_corner_is_within_skia_antialiasing_at_2x() {
    assert_near(&render(INPUT, 2).grid(38, 38, 10), &RING_2X, 12);
}

#[test]
fn empty_textarea_caret_matches_chrome() {
    assert_eq!(caret_rows(EMPTY_TEXTAREA, 1, 23), (30, 44));
    assert_eq!(caret_rows(EMPTY_TEXTAREA, 2, 46), (60, 91));
}

#[test]
fn empty_caret_centres_odd_leading_like_chrome() {
    assert_eq!(caret_rows(TALL_TEXTAREA, 1, 23), (25, 44));
    assert_eq!(caret_rows(TALL_TEXTAREA, 2, 46), (51, 90));
}

#[test]
fn caret_snaps_at_fractional_positions() {
    assert_eq!(caret_rows(SHIFTED_INPUT, 1, 24), (24, 45));
}
