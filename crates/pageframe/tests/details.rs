mod support;

use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use support::{HEIGHT, WIDTH, open, pixel, render};

const RED: [u8; 3] = [255, 0, 0];
const GREEN: [u8; 3] = [0, 128, 0];
const BLUE: [u8; 3] = [0, 0, 255];

const PAGE: &str = r#"<html><head><style>
html,body{margin:0;background:white;font:10px/10px monospace;color:rgb(0,128,0)}
details{background:rgb(0,128,0)}
summary{background:rgb(255,0,0);color:rgb(255,0,0)}
.after{height:10px;background:rgb(0,0,255)}
</style></head><body>
<details id="bare"><summary>s</summary>bare</details>
<div class="after"></div>
<details id="mixed">before<summary>s</summary>bare <b>bold</b> tail</details>
<div class="after"></div>
<details id="first" open>before <summary>s</summary>after</details>
<div class="after"></div>
<details id="blocks" open><p style="margin:0">p</p><summary>s</summary></details>
</body></html>"#;

const MARKER_PAGE: &str = r#"<html><head><style>
html,body{margin:0;background:white;font:40px/40px monospace;color:black}
i{display:inline-block;width:20px;height:40px;background:rgb(255,0,0)}
</style></head><body>
<details id="marker"><summary><i></i></summary></details>
</body></html>"#;

const MARKER_SIDE: f32 = 0.66 * 40.0;
const MARKER_ALTITUDE: f32 = MARKER_SIDE * 0.866;
const MARKER_AREA: f32 = MARKER_SIDE * MARKER_ALTITUDE / 2.0;

fn frame(harness: &mut Harness<Page<()>>) -> Vec<u8> {
    harness.pump();
    render(harness)
}

fn check(buffer: &[u8], x: usize, y: usize, expected: [u8; 3]) {
    let actual = pixel(buffer, x, y);
    assert_eq!(
        [actual[0], actual[1], actual[2]],
        expected,
        "pixel ({x}, {y})"
    );
}

fn size() -> (usize, usize) {
    (
        usize::try_from(WIDTH).expect("test width fits in usize"),
        usize::try_from(HEIGHT).expect("test height fits in usize"),
    )
}

fn marker_ink(buffer: &[u8]) -> (f32, usize) {
    let (width, height) = size();
    let mut area = 0.0;
    let mut right = 0;
    for y in 0..height {
        for x in 0..width {
            let [red, green, _, _] = pixel(buffer, x, y);
            if red == green && red < 255 {
                area += 1.0 - f32::from(red) / 255.0;
                right = right.max(x);
            }
        }
    }
    (area, right)
}

fn first_red_column(buffer: &[u8]) -> Option<usize> {
    let (width, _) = size();
    (0..width).find(|x| {
        let [red, green, _, _] = pixel(buffer, *x, 20);
        red == 255 && green < 30
    })
}

#[test]
fn closed_details_hides_bare_text() {
    let mut harness = open(App::new((), PAGE));
    let buffer = frame(&mut harness);
    check(&buffer, 100, 5, RED);
    check(&buffer, 100, 15, BLUE);
    check(&buffer, 100, 25, RED);
    check(&buffer, 100, 35, BLUE);
}

#[test]
fn bare_text_follows_the_open_attribute() {
    let mut harness = open(App::new((), PAGE));
    check(&frame(&mut harness), 100, 15, BLUE);
    harness.doc.ui().select("#bare").set_attr("open", "");
    let buffer = frame(&mut harness);
    check(&buffer, 100, 15, GREEN);
    check(&buffer, 100, 25, BLUE);
    harness.doc.ui().select("#bare").remove_attr("open");
    check(&frame(&mut harness), 100, 15, BLUE);
}

#[test]
fn summary_comes_before_earlier_content() {
    let mut harness = open(App::new((), PAGE));
    let buffer = frame(&mut harness);
    check(&buffer, 100, 45, RED);
    check(&buffer, 100, 55, GREEN);
    check(&buffer, 100, 65, BLUE);
    check(&buffer, 100, 75, RED);
}

#[test]
fn disclosure_marker_matches_chrome_geometry() {
    let mut harness = open(App::new((), MARKER_PAGE));
    let closed = frame(&mut harness);
    let (area, right) = marker_ink(&closed);
    assert!((area - MARKER_AREA).abs() < 6.0, "closed area {area}");
    assert_eq!(right, 22);
    assert_eq!(first_red_column(&closed), Some(42));

    harness.doc.ui().select("#marker").set_attr("open", "");
    let opened = frame(&mut harness);
    let (area, right) = marker_ink(&opened);
    assert!((area - MARKER_AREA).abs() < 6.0, "open area {area}");
    assert_eq!(right, 26);
    assert_eq!(first_red_column(&opened), Some(42));
}
