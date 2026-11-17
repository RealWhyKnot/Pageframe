mod scaled;
mod support;

use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use scaled::{render_at_ratio, render_at_scale};
use support::{open, pixel, render};

const PAGE: &str = "<!doctype html><style>html,body{margin:0;background:white;font:14px Arial}div{margin-bottom:4px}\
.box{display:inline-block;width:40px;margin:2px;background:#36c}</style>\
<div>e <sub>sub</sub><sup>sup</sup></div>\
<div>e <sub>sub</sub> f</div>\
<div>e <sup>sup</sup> f</div>\
<div style=\"font-size:16px\">e <sub>sub</sub><sup>sup</sup></div>\
<div style=\"white-space:nowrap\">a <span class=box style=\"height:20px;vertical-align:top\"></span> b \
<span class=box style=\"height:30px;vertical-align:middle\"></span> c \
<span class=box style=\"height:40px\"></span> d</div>\
<div>after</div>";

fn heights_and_tops<S: 'static>(harness: &mut Harness<Page<S>>) -> Vec<[f64; 2]> {
    let doc = harness.base_mut();
    doc.query_selector_all("div")
        .expect("valid selector")
        .into_iter()
        .map(|id| {
            let rect = doc.get_client_bounding_rect(id).expect("the div has a box");
            [rect.y, rect.height]
        })
        .collect()
}

fn assert_close(found: &[[f64; 2]], chrome: &[[f64; 2]], what: &str) {
    assert_eq!(found.len(), chrome.len(), "{what}");
    for (found, chrome) in found.iter().zip(chrome) {
        assert!(
            found
                .iter()
                .zip(chrome)
                .all(|(have, want)| (have - want).abs() < 0.02),
            "{what}: {found:?}, Chrome {chrome:?}"
        );
    }
}

#[test]
fn sub_sup_and_middle_shift_lines_like_chrome() {
    let mut harness = open(App::new((), PAGE));
    let buffer = render(&mut harness);
    let lowest_sub_ink = (0..22)
        .rev()
        .find(|&down| (12..30).any(|across| pixel(&buffer, across, down)[0] < 128));
    assert_eq!(
        lowest_sub_ink,
        Some(19),
        "Chrome's subscript baseline is at 20.45, so its ink ends on row 19"
    );
    assert_close(
        &heights_and_tops(&mut harness),
        &[
            [0.0, 22.45],
            [26.45, 18.8],
            [49.25, 19.66],
            [72.91, 25.52],
            [102.42, 57.38],
            [163.8, 16.0],
        ],
        "lines at 1x",
    );

    render_at_scale(&mut harness, 2);
    assert_close(
        &heights_and_tops(&mut harness),
        &[
            [0.0, 21.96],
            [25.96, 18.8],
            [48.76, 19.16],
            [71.92, 25.02],
            [100.95, 57.37],
            [162.31, 16.0],
        ],
        "lines at 2x",
    );
}

#[test]
fn a_line_on_a_half_device_pixel_paints_half_up_at_seven_quarters_scale() {
    let mut harness = open(App::new(
        (),
        "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}</style><details open><summary>Open details</summary>Inside details</details><details><summary>Closed details</summary>hidden</details><dialog open>Open dialog</dialog><hr><mark>mark</mark> <kbd>kbd</kbd> <code>code</code> <abbr title=x>abbr</abbr> <small>small</small>",
    ));
    let buffer = render_at_ratio(&mut harness, (7, 4), 640);
    let at = |column: usize, row: usize| {
        let start = (row * 1120 + column) * 4;
        [buffer[start], buffer[start + 1], buffer[start + 2]]
    };
    let marked = |row: usize| at(10, row) == [255, 255, 0];
    assert_eq!(
        (marked(111), marked(112), marked(138), marked(139)),
        (false, true, true, false)
    );
    let ink = |row: usize| at(175, row).iter().all(|&channel| channel < 80);
    assert_eq!((ink(132), ink(133), ink(134)), (true, true, false));
}
