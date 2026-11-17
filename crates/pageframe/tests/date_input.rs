mod scaled;
mod support;

use pageframe::App;
use scaled::render_at_scale;
use support::{WIDTH, open, pixel, render};

const FIXED_SIZE: &str = "<!doctype html><style>body{margin:0;background:#fff}\
input{display:block;margin:0;width:80px;height:16px}</style>\
<input id=a type=date><input id=b type=time><input id=c type=date disabled>";

const AUTO_SIZE: &str = "<!doctype html><style>body{margin:0;background:#fff}\
input{display:block;margin:0}</style>\
<input id=date type=date value=2024-03-09><input id=disabled type=date disabled>\
<input id=time type=time><input id=seconds type=time step=1>";

fn widths(page: &'static str, selectors: &[&str]) -> Vec<f32> {
    let harness = open(App::new((), page));
    let base = harness.base();
    let widths = selectors
        .iter()
        .map(|selector| {
            let id = base
                .query_selector_all(selector)
                .expect("the selector parses")[0];
            base.get_node(id)
                .expect("the node exists")
                .final_layout()
                .size
                .width
        })
        .collect();
    drop(base);
    widths
}

fn ink_columns(
    buffer: &[u8],
    stride: usize,
    columns: std::ops::Range<usize>,
    rows: std::ops::Range<usize>,
) -> (usize, usize) {
    let columns: Vec<usize> = columns
        .filter(|&column| {
            rows.clone().any(|row| {
                let start = (row * stride + column) * 4;
                buffer[start] < 128
            })
        })
        .collect();
    (
        *columns.first().expect("the row has ink"),
        *columns.last().expect("the row has ink"),
    )
}

#[test]
fn date_icons_snap_to_device_pixels_like_chrome() {
    let mut harness = open(App::new((), FIXED_SIZE));
    let buffer = render(&mut harness);
    let samples = [
        ((71, 5), [0x00, 0x00, 0x00]),
        ((69, 9), [0x7f, 0x7f, 0x7f]),
        ((70, 9), [0x7f, 0x7f, 0x7f]),
        ((72, 9), [0xff, 0xff, 0xff]),
        ((71, 14), [0x7f, 0x7f, 0x7f]),
        ((72, 15), [0xff, 0xff, 0xff]),
        ((75, 30), [0xff, 0xff, 0xff]),
        ((71, 45), [0xf8, 0xf8, 0xf8]),
        ((69, 49), [0xf8, 0xf8, 0xf8]),
    ];
    for ((column, row), expected) in samples {
        let [red, green, blue, _] = pixel(&buffer, column, row);
        assert_eq!(
            [red, green, blue],
            expected,
            "pixel at ({column}, {row}) differs from Chrome"
        );
    }

    let scaled = render_at_scale(&mut harness, 2);
    let width = usize::try_from(WIDTH).expect("test width fits in usize") * 2;
    let samples = [
        ((142, 9), [0x00, 0x00, 0x00]),
        ((139, 11), [0x00, 0x00, 0x00]),
        ((142, 7), [0xff, 0xff, 0xff]),
        ((141, 20), [0xff, 0xff, 0xff]),
    ];
    for ((column, row), expected) in samples {
        let start = (row * width + column) * 4;
        assert_eq!(
            scaled[start..start + 3],
            expected,
            "pixel at ({column}, {row}) at 2x differs from Chrome"
        );
    }
}

#[test]
fn disabled_dates_keep_the_icon_space_and_seconds_widen_times() {
    let widths = widths(AUTO_SIZE, &["#date", "#disabled", "#time", "#seconds"]);
    assert_eq!(widths[0], widths[1], "disabled date width");
    assert!(
        widths[3] > widths[2],
        "a time with step=1 shows seconds, so it is wider: {widths:?}"
    );
}

#[test]
fn date_text_follows_a_scale_change() {
    let mut harness = open(App::new((), AUTO_SIZE));
    let buffer = render(&mut harness);
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    let (start, end) = ink_columns(&buffer, width, 3..80, 4..17);
    let scaled = render_at_scale(&mut harness, 2);
    let (scaled_start, scaled_end) = ink_columns(&scaled, width * 2, 6..160, 8..34);
    let ink = end - start;
    let scaled_ink = scaled_end - scaled_start;
    assert!(
        scaled_ink + 3 >= ink * 2 && scaled_ink <= ink * 2 + 3,
        "text is {ink}px wide at 1x and {scaled_ink}px at 2x"
    );
}
