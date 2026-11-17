mod support;

use pageframe::App;
use pageframe_dom::GaugeRegion;
use support::{open, pixel, render};

const BASELINES: &str = "<!doctype html><style>html,body{margin:0;font:16px Arial}</style>\
<div style='width:1000px'><progress id=p></progress>\
<span id=s1 style='display:inline-block;width:5px;height:10px'></span>\
<meter id=m value=0.5></meter><progress id=p2 value=0.5 style='height:40px'></progress>\
<meter id=m2 style='vertical-align:baseline'></meter>\
<span id=s2 style='display:inline-block;width:5px;height:10px'></span>\
<progress id=p3 style='vertical-align:baseline'>fallback text</progress></div>";

const BARS: &str = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}\
progress,meter{display:block;width:100px;margin:2px 4px}</style>\
<progress value=0.3></progress><progress value=1 style='accent-color:#c00'></progress>\
<meter value=0.3></meter><meter min=0 max=100 low=30 high=70 optimum=80 value=50></meter>\
<meter min=0 max=100 low=30 high=70 optimum=80 value=20></meter>";

const PROGRESS_VALUES: &str = "<!doctype html><progress></progress><progress value=0.3></progress>\
<progress value=70 max=100></progress><progress value=2></progress>\
<progress value=-1></progress><progress value=5 max=0></progress>\
<progress value=5 max=-3></progress><progress value=abc></progress>\
<progress value=0.5 max=abc></progress><progress value=' 0.5'></progress>\
<progress value=1e1 max=40></progress>";

const METER_VALUES: &str = "<!doctype html><meter value=' 0.5'></meter>\
<meter value='0.5 '></meter><meter value=' 0.5 '></meter><meter value='&#10;0.5'></meter>\
<meter value='&#12;0.5'></meter><meter value='0.5x'></meter><meter value='+0.5'></meter>\
<meter value='.5'></meter><meter value='5.'></meter><meter value='-.5' min=-1></meter>\
<meter value='1e-1'></meter><meter value='0.5e'></meter><meter value='&#160;0.5'></meter>\
<meter value='0x1'></meter><meter value='  .25'></meter>";

const REGIONS: &str = "<!doctype html><meter value=0.5></meter>\
<meter min=0 max=100 low=30 high=70 optimum=80 value=50></meter>\
<meter min=0 max=100 low=30 high=70 optimum=80 value=20></meter>\
<meter min=0 max=100 low=30 high=70 optimum=80 value=90></meter>\
<meter min=0 max=100 low=30 high=70 optimum=10 value=50></meter>\
<meter min=0 max=100 low=30 high=70 optimum=10 value=80></meter>\
<meter min=0 max=100 low=30 high=70 value=80></meter>";

#[test]
fn progress_and_meter_size_and_align_like_chrome() {
    let harness = open(App::new((), BASELINES));
    let base = harness.base();
    let layout = |selector: &str| {
        let id = base
            .query_selector_all(selector)
            .expect("the selector parses")[0];
        let layout = base.get_node(id).expect("the node exists").final_layout();
        (
            layout.location.x,
            layout.location.y + layout.size.height,
            layout.size.width,
            layout.size.height,
        )
    };
    let (mark_x, baseline, _, _) = layout("#s1");
    let below_baseline = 0.2 * 16.0;
    let chrome = [
        ("#p", mark_x - 160.0, baseline + below_baseline, 160.0, 16.0),
        ("#m", mark_x + 5.0, baseline + below_baseline, 80.0, 16.0),
        ("#p2", mark_x + 85.0, baseline + below_baseline, 160.0, 40.0),
        ("#m2", mark_x + 245.0, baseline, 80.0, 16.0),
        ("#s2", mark_x + 325.0, baseline, 5.0, 10.0),
        ("#p3", mark_x + 330.0, baseline, 160.0, 16.0),
    ];
    for (selector, left, bottom, width, height) in chrome {
        let ours = layout(selector);
        let expected = (left, bottom, width, height);
        let off = [
            ours.0 - left,
            ours.1 - bottom,
            ours.2 - width,
            ours.3 - height,
        ]
        .into_iter()
        .map(f32::abs)
        .fold(0.0, f32::max);
        assert!(
            off <= 0.25,
            "{selector} is {ours:?}, Chrome has {expected:?}"
        );
    }
}

#[test]
fn progress_and_meter_paint_like_chrome() {
    let mut harness = open(App::new((), BARS));
    let buffer = render(&mut harness);
    let samples = [
        ((20, 9), [0x00, 0x75, 0xff]),
        ((60, 9), [0xef, 0xef, 0xef]),
        ((60, 6), [0xd0, 0xd0, 0xd0]),
        ((20, 5), [0xaf, 0xc9, 0xe7]),
        ((34, 9), [0xef, 0xef, 0xef]),
        ((50, 25), [0xcc, 0x00, 0x00]),
        ((50, 21), [0xdc, 0xaf, 0xaf]),
        ((15, 41), [0x10, 0x7c, 0x10]),
        ((60, 41), [0xef, 0xef, 0xef]),
        ((60, 38), [0xcb, 0xcb, 0xcb]),
        ((15, 57), [0xff, 0xb9, 0x00]),
        ((45, 57), [0xff, 0xb9, 0x00]),
        ((15, 73), [0xd8, 0x3b, 0x01]),
        ((60, 73), [0xef, 0xef, 0xef]),
        ((103, 73), [0xcc, 0xcc, 0xcc]),
    ];
    for ((column, row), expected) in samples {
        let [red, green, blue, _] = pixel(&buffer, column, row);
        let ours = [red, green, blue];
        let off = ours
            .iter()
            .zip(expected)
            .map(|(channel, chrome)| channel.abs_diff(chrome))
            .max()
            .expect("three channels");
        assert!(
            off <= 2,
            "pixel at ({column}, {row}) is {ours:?}, Chrome has {expected:?}"
        );
    }
}

#[test]
fn progress_positions_parse_like_chrome() {
    let harness = open(App::new((), PROGRESS_VALUES));
    let positions: Vec<Option<f64>> = {
        let base = harness.base();
        base.query_selector_all("progress")
            .expect("the selector parses")
            .into_iter()
            .map(|id| {
                base.get_node(id)
                    .expect("the node exists")
                    .element_data()
                    .expect("progress is an element")
                    .progress_position()
            })
            .collect()
    };
    let chrome = [
        None,
        Some(0.3),
        Some(0.7),
        Some(1.0),
        Some(0.0),
        Some(1.0),
        Some(1.0),
        Some(0.0),
        Some(0.5),
        Some(0.5),
        Some(0.25),
    ];
    assert_eq!(positions.len(), chrome.len());
    for (index, (ours, expected)) in positions.iter().zip(chrome).enumerate() {
        let close = match (ours, expected) {
            (Some(ours), Some(expected)) => (ours - expected).abs() < 1e-9,
            (None, None) => true,
            _ => false,
        };
        assert!(close, "progress {index}: {ours:?} vs Chrome {expected:?}");
    }
}

fn meter_gauges(content: &'static str) -> Vec<(f64, GaugeRegion)> {
    let harness = open(App::new((), content));
    let base = harness.base();
    let gauges = base
        .query_selector_all("meter")
        .expect("the selector parses")
        .into_iter()
        .map(|id| {
            base.get_node(id)
                .expect("the node exists")
                .element_data()
                .expect("meter is an element")
                .meter_gauge()
        })
        .collect();
    drop(base);
    gauges
}

#[test]
fn meter_values_parse_like_chrome() {
    let ratios: Vec<f64> = meter_gauges(METER_VALUES)
        .into_iter()
        .map(|(ratio, _)| ratio)
        .collect();
    let chrome = [
        0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 1.0, 0.25, 0.1, 0.5, 0.0, 0.0, 0.25,
    ];
    assert_eq!(ratios.len(), chrome.len());
    for (index, (ours, expected)) in ratios.iter().zip(chrome).enumerate() {
        assert!(
            (ours - expected).abs() < 1e-9,
            "meter {index}: {ours} vs Chrome {expected}"
        );
    }
}

#[test]
fn meter_regions_follow_low_high_and_optimum() {
    let regions: Vec<GaugeRegion> = meter_gauges(REGIONS)
        .into_iter()
        .map(|(_, region)| region)
        .collect();
    assert_eq!(
        regions,
        [
            GaugeRegion::Optimum,
            GaugeRegion::Suboptimum,
            GaugeRegion::EvenLessGood,
            GaugeRegion::Optimum,
            GaugeRegion::Suboptimum,
            GaugeRegion::EvenLessGood,
            GaugeRegion::Suboptimum,
        ]
    );
}

const LOWERED: &str = "<!doctype html><style>html,body{margin:0}</style>\
<p style=\"margin:0;font:14px Arial\">Xg<span id=box style=\"display:inline-block;\
width:20px;height:14px;vertical-align:-0.2em\"></span></p>";

fn lowered_top_at_scale(scale: f32) -> f32 {
    let mut harness = open(App::new((), LOWERED));
    let scheme = harness.base().get_viewport().color_scheme;
    harness
        .base_mut()
        .set_viewport(pageframe_traits::shell::Viewport::new(
            support::WIDTH * 2,
            support::HEIGHT * 2,
            scale,
            scheme,
        ));
    harness.pump();
    harness.layout_rect("#box").y
}

#[test]
fn length_vertical_align_lands_on_chromes_device_pixel_at_every_scale() {
    for (scale, chrome_top) in [(1.0, 1.796_875_f32), (2.0, 1.296_875)] {
        let top = lowered_top_at_scale(scale);
        assert_eq!(
            (top * scale).round(),
            (chrome_top * scale).round(),
            "at {scale}x our top is {top}, Chrome's is {chrome_top}"
        );
    }
}
