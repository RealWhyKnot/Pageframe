mod support;

use pageframe::{App, Canvas};
use support::{open, pixel, render};

const TEXT_PAGE: &str = r#"<html><head><style>
body{margin:0;background:white}
div{font:italic 50px Impact;line-height:60px;padding-left:20px}
</style></head><body><div>III</div></body></html>"#;

const CANVAS_PAGE: &str = r#"<html><head><style>
body{margin:0;background:white}
canvas{display:block}
</style></head><body>
<canvas id="c" width="100" height="60"></canvas>
</body></html>"#;

fn lean(buffer: &[u8], top: usize, foot: usize) -> (usize, usize) {
    let dark = |row: usize| (0..100).find(|&column| pixel(buffer, column, row)[0] < 128);
    dark(top).zip(dark(foot)).expect("ink on both rows")
}

#[test]
fn page_text_without_an_italic_face_leans_forward() {
    let mut harness = open(App::new((), TEXT_PAGE));
    let buffer = render(&mut harness);
    let (top, foot) = lean(&buffer, 20, 45);
    assert!(top >= foot + 4, "top {top} foot {foot}");
}

#[test]
fn canvas_text_without_an_italic_face_leans_forward() {
    let mut harness = open(App::new((), CANVAS_PAGE));
    harness.doc.ui().select("#c").draw(|canvas: &mut Canvas| {
        canvas
            .set_font("Impact", 50.0)
            .set_font_italic(true)
            .fill_text("III", 20.0, 50.0);
    });
    harness.pump();
    let buffer = render(&mut harness);
    let (top, foot) = lean(&buffer, 20, 45);
    assert!(top >= foot + 4, "top {top} foot {foot}");
}
