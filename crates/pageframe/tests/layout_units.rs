mod scaled;
mod support;

use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use scaled::{render_at_ratio, render_at_scale};
use support::{WIDTH, open, pixel, render};

const PAGE: &str = "<!doctype html><style>html,body{margin:0;background:white;font:14px Arial}div{background:#000}</style>\
<div id=h style=\"height:30.31px;width:10.99px\"></div>\
<div id=m style=\"margin-top:0.99px;margin-left:5.49px;height:2px;width:3px\"></div>\
<div id=p style=\"padding:1.99px 0.49px;width:3px;height:3px\"></div>\
<div style=\"width:31px\"><div id=pc style=\"width:50.7%;height:1px\"></div></div>\
<div id=mn style=\"min-height:7.49px;max-width:4.99px;width:9px\"></div>\
<div style=\"position:relative;height:10px\"><div id=ab style=\"position:absolute;left:3.99px;top:2.49px;width:1px;height:1px\"></div></div>\
<div style=\"display:flex;column-gap:2.99px;height:3px\"><div id=f1 style=\"width:1.49px\"></div><div id=f2 style=\"width:1px\"></div></div>";

const IDS: [&str; 7] = ["h", "m", "p", "mn", "ab", "f1", "f2"];

type Boxes = [[f64; 4]; 7];

fn boxes_in_units<S: 'static>(harness: &mut Harness<Page<S>>) -> Vec<[f64; 4]> {
    let doc = harness.base_mut();
    IDS.iter()
        .map(|id| {
            let node = doc
                .query_selector(&format!("#{id}"))
                .expect("valid selector")
                .expect("the box exists");
            let rect = doc
                .get_client_bounding_rect(node)
                .expect("the element has a box");
            [rect.x, rect.y, rect.width, rect.height].map(|value| value * 64.0)
        })
        .collect()
}

fn mismatches(found: &[[f64; 4]], chrome: &[[f64; 4]], what: &str) -> Vec<String> {
    found
        .iter()
        .zip(chrome)
        .zip(IDS)
        .filter(|((found, chrome), _)| {
            found
                .iter()
                .zip(chrome.iter())
                .any(|(have, want)| (have - want).abs() >= 0.5)
        })
        .map(|((found, chrome), id)| format!("{what} #{id}: {found:?}, Chrome {chrome:?}"))
        .collect()
}

#[test]
fn lengths_truncate_to_layout_units_like_chrome() {
    let mut harness = open(App::new((), PAGE));
    let cases: [((u16, u16), Boxes); 3] = [
        (
            (1, 1),
            [
                [0.0, 0.0, 703.0, 1939.0],
                [351.0, 2002.0, 192.0, 128.0],
                [0.0, 2130.0, 254.0, 446.0],
                [0.0, 2640.0, 319.0, 479.0],
                [255.0, 3278.0, 64.0, 64.0],
                [0.0, 3759.0, 95.0, 192.0],
                [286.0, 3759.0, 64.0, 192.0],
            ],
        ),
        (
            (7, 4),
            [
                [0.0, 0.0, 702.857, 1939.429],
                [350.857, 2002.286, 192.0, 128.0],
                [0.0, 2130.286, 253.714, 445.714],
                [0.0, 2640.0, 318.857, 478.857],
                [254.857, 3277.714, 64.0, 64.0],
                [0.0, 3758.857, 94.857, 192.0],
                [285.714, 3758.857, 64.0, 192.0],
            ],
        ),
        (
            (2, 1),
            [
                [0.0, 0.0, 703.0, 1939.5],
                [351.0, 2002.5, 192.0, 128.0],
                [0.0, 2130.5, 254.0, 446.0],
                [0.0, 2640.5, 319.0, 479.0],
                [255.0, 3278.5, 64.0, 64.0],
                [0.0, 3759.5, 95.0, 192.0],
                [286.0, 3759.5, 64.0, 192.0],
            ],
        ),
    ];
    let frame = render(&mut harness);
    assert_eq!(
        pixel(&frame, 5, 15),
        [0, 0, 0, 255],
        "inside the 30.31px box"
    );
    assert_eq!(pixel(&frame, 2, 31), [255, 255, 255, 255], "below it");
    let mut wrong = Vec::new();
    for (ratio, chrome) in cases {
        match ratio {
            (scale, 1) => render_at_scale(&mut harness, scale),
            ratio => render_at_ratio(&mut harness, ratio, WIDTH),
        };
        wrong.extend(mismatches(
            &boxes_in_units(&mut harness),
            &chrome,
            &format!("scale {}/{}", ratio.0, ratio.1),
        ));
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
