mod scaled;
mod support;

use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use scaled::{render_at_ratio, render_at_scale};
use support::{WIDTH, open, pixel, render};

const BLOCKS: &str = "<!doctype html><style>html,body{margin:0;background:white;font:14px Arial}div{background:#000}</style>\
<div id=h style=\"height:30.31px;width:10.99px\"></div>\
<div id=m style=\"margin-top:0.99px;margin-left:5.49px;height:2px;width:3px\"></div>\
<div id=p style=\"padding:1.99px 0.49px;width:3px;height:3px\"></div>\
<div style=\"width:31px\"><div id=pc style=\"width:50.7%;height:1px\"></div></div>\
<div id=mn style=\"min-height:7.49px;max-width:4.99px;width:9px\"></div>\
<div style=\"position:relative;height:10px\"><div id=ab style=\"position:absolute;left:3.99px;top:2.49px;width:1px;height:1px\"></div></div>\
<div style=\"display:flex;column-gap:2.99px;height:3px\"><div id=f1 style=\"width:1.49px\"></div><div id=f2 style=\"width:1px\"></div></div>";

const GRID_AND_INLINE: &str = "<!doctype html><style>html,body{margin:0;font:14px Arial}#g>div{background:#000}</style>\
<div id=g style=\"display:grid;grid-template-columns:30.31px 10px;grid-template-rows:5.49px;column-gap:0\"><div id=g1></div><div id=g2></div></div>\
<div style=\"width:200px\">ab<span id=sp style=\"padding-left:0.49px;padding-right:1.99px;margin-left:0.99px\">cd</span><span id=sq>ef</span></div>";

const HUNG_SPACES: &str = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}body{width:300px}div{margin:0}</style>\
<div dir=\"rtl\" style=\"unicode-bidi:plaintext;width:60px\"><i id=\"h0\">&#1488;</i><br><i id=\"hn1\">12</i><i id=\"hd1\"> - </i><i id=\"hn2\">34</i><i id=\"hd2\"> - </i><i id=\"hn3\">56</i><i id=\"hd3\"> - </i><i id=\"hn4\">78</i> <i id=\"hs\">abc</i></div>\
<div dir=\"rtl\" style=\"unicode-bidi:plaintext\"><i id=\"i0\">&#1488;</i><br><i id=\"in1\">12</i><i id=\"id1\"> - </i><i id=\"in2\">34</i><i id=\"id2\"> - </i><i id=\"in3\">56</i><i id=\"id3\"> - </i><i id=\"in4\">78</i> <i id=\"is\">abc</i></div>\
<div dir=\"ltr\" style=\"unicode-bidi:plaintext;width:60px\"><i id=\"k0\">abc</i><br><i id=\"kn1\">12</i><i id=\"kd1\"> - </i><i id=\"kn2\">34</i><i id=\"kd2\"> - </i><i id=\"kn3\">56</i><i id=\"kd3\"> - </i><i id=\"kn4\">78</i> <i id=\"ks\">&#1488;</i></div>\
<div dir=\"rtl\" style=\"unicode-bidi:plaintext;width:60px\"><i id=\"ln1\">12</i><i id=\"ld1\"> - </i><i id=\"ln2\">34</i><i id=\"ld2\"> - </i><i id=\"ln3\">56</i><i id=\"ld3\"> - </i><i id=\"ln4\">78</i> <i id=\"ls\">abc</i></div>\
<div dir=\"ltr\" style=\"unicode-bidi:plaintext;width:60px\"><i id=\"m0\">&#1488;</i><br><i id=\"mn1\">12</i><i id=\"md1\"> - </i><i id=\"mn2\">34</i><i id=\"md2\"> - </i><i id=\"mn3\">56</i><i id=\"md3\"> - </i><i id=\"mn4\">78</i> <i id=\"ms\"></i></div>";

const PRESERVED_SPACES: &str = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}body{width:300px}div{margin:0}</style>\
<div style=\"white-space:pre-wrap\"><i id=\"a\"> - </i><br><i id=\"b\">x</i></div>\
<div style=\"white-space:pre-wrap;width:20px\"><i id=\"c\"> - </i><i id=\"d\">xx</i></div>\
<div style=\"white-space:pre-line;width:20px\"><i id=\"e\"> - </i><i id=\"f\">xx</i></div>\
<div style=\"white-space:break-spaces;width:20px\"><i id=\"g\"> - </i><i id=\"h\">xx</i></div>";

type Ratio = (u16, u16);

fn boxes_in_units<S: 'static>(harness: &mut Harness<Page<S>>, ids: &[&str]) -> Vec<[f64; 4]> {
    let doc = harness.base_mut();
    ids.iter()
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

fn mismatches(found: &[[f64; 4]], chrome: &[[f64; 4]], ids: &[&str], ratio: Ratio) -> Vec<String> {
    found
        .iter()
        .zip(chrome)
        .zip(ids)
        .filter(|((found, chrome), _)| {
            found
                .iter()
                .zip(chrome.iter())
                .any(|(have, want)| (have - want).abs() >= 0.01)
        })
        .map(|((found, chrome), id)| {
            format!(
                "scale {}/{} #{id}: {found:?}, Chrome {chrome:?}",
                ratio.0, ratio.1
            )
        })
        .collect()
}

fn render_at(harness: &mut Harness<Page<()>>, ratio: Ratio) {
    match ratio {
        (scale, 1) => render_at_scale(harness, scale),
        ratio => render_at_ratio(harness, ratio, WIDTH),
    };
}

fn check(page: &str, ids: &[&str], cases: &[(Ratio, Vec<[f64; 4]>)]) -> Vec<String> {
    let mut harness = open(App::new((), page));
    let mut wrong = Vec::new();
    for (ratio, chrome) in cases {
        render_at(&mut harness, *ratio);
        let found = boxes_in_units(&mut harness, ids);
        wrong.extend(mismatches(&found, chrome, ids, *ratio));
    }
    wrong
}

#[test]
fn lengths_truncate_to_layout_units_like_chrome() {
    let mut harness = open(App::new((), BLOCKS));
    let frame = render(&mut harness);
    assert_eq!(
        pixel(&frame, 5, 15),
        [0, 0, 0, 255],
        "inside the 30.31px box"
    );
    assert_eq!(pixel(&frame, 2, 31), [255, 255, 255, 255], "below it");
    let wrong = check(
        BLOCKS,
        &["h", "m", "p", "mn", "ab", "f1", "f2"],
        &[
            (
                (1, 1),
                vec![
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
                vec![
                    [0.0, 0.0, 702.8572, 1939.4287],
                    [350.8571, 2002.2858, 192.0, 128.0],
                    [0.0, 2130.2859, 253.7143, 445.7143],
                    [0.0, 2640.0, 318.8571, 478.8572],
                    [254.8571, 3277.7144, 64.0, 64.0],
                    [0.0, 3758.8574, 94.8571, 192.0],
                    [285.7143, 3758.8574, 64.0, 192.0],
                ],
            ),
            (
                (2, 1),
                vec![
                    [0.0, 0.0, 703.0, 1939.5],
                    [351.0, 2002.5, 192.0, 128.0],
                    [0.0, 2130.5, 254.0, 446.0],
                    [0.0, 2640.5, 319.0, 479.0],
                    [255.0, 3278.5, 64.0, 64.0],
                    [0.0, 3759.5, 95.0, 192.0],
                    [286.0, 3759.5, 64.0, 192.0],
                ],
            ),
        ],
    );
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn grid_tracks_and_inline_edges_truncate_like_chrome() {
    let wrong = check(
        GRID_AND_INLINE,
        &["g1", "g2"],
        &[
            (
                (1, 1),
                vec![[0.0, 0.0, 1939.0, 351.0], [1939.0, 0.0, 640.0, 351.0]],
            ),
            (
                (7, 4),
                vec![
                    [0.0, 0.0, 1939.4287, 350.8571],
                    [1939.4287, 0.0, 640.0, 350.8571],
                ],
            ),
            (
                (2, 1),
                vec![[0.0, 0.0, 1939.5, 351.0], [1939.5, 0.0, 640.0, 351.0]],
            ),
        ],
    );
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let mut harness = open(App::new((), GRID_AND_INLINE));
    let chrome = [
        ((1, 1), [[1060.0, 1105.0], [2165.0, 748.0]]),
        ((7, 4), [[1060.0, 1104.5715], [2164.5715, 747.4286]]),
        ((2, 1), [[1060.0, 1104.5], [2164.5, 747.5]]),
    ];
    for (ratio, spans) in chrome {
        render_at(&mut harness, ratio);
        let found: Vec<[f64; 4]> = boxes_in_units(&mut harness, &["sp", "sq"])
            .into_iter()
            .map(|[left, _, width, _]| [left, 0.0, width, 0.0])
            .collect();
        let want: Vec<[f64; 4]> = spans
            .iter()
            .map(|&[left, width]| [left, 0.0, width, 0.0])
            .collect();
        let wrong = mismatches(&found, &want, &["sp", "sq"], ratio);
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }
}

#[test]
fn text_before_a_hung_space_snaps_up_to_a_layout_unit_like_chrome() {
    let wrong = check(
        HUNG_SPACES,
        &[
            "hn1", "hd1", "hn2", "hd2", "kn1", "kd1", "kn2", "kd2", "ld2", "md2",
        ],
        &[
            (
                (1, 1),
                vec![
                    [2342.0, 1024.0, 997.0, 1024.0],
                    [1545.0, 1024.0, 797.0, 1024.0],
                    [548.0, 1024.0, 997.0, 1024.0],
                    [0.0, 1024.0, 548.0, 1024.0],
                    [501.0, 7168.0, 997.0, 1024.0],
                    [1498.0, 7168.0, 797.0, 1024.0],
                    [2295.0, 7168.0, 997.0, 1024.0],
                    [3292.0, 7168.0, 548.0, 1024.0],
                    [2791.0, 9216.0, 548.0, 1024.0],
                    [0.0, 13312.0, 548.0, 1024.0],
                ],
            ),
            (
                (7, 4),
                vec![
                    [2341.1431, 1024.0, 997.1429, 987.4286],
                    [1544.5715, 1024.0, 796.5715, 987.4286],
                    [547.4286, 1024.0, 997.1429, 987.4286],
                    [0.0, 1024.0, 547.4286, 987.4286],
                    [501.7143, 7168.0005, 997.1429, 987.4286],
                    [1498.8572, 7168.0005, 796.5715, 987.4286],
                    [2295.4287, 7168.0005, 997.1429, 987.4286],
                    [3292.5715, 7168.0005, 547.4286, 987.4286],
                    [2790.8572, 9216.0, 547.4286, 987.4286],
                    [0.0, 13312.001, 547.4286, 987.4286],
                ],
            ),
            (
                (2, 1),
                vec![
                    [2341.0, 1024.0, 997.0, 992.0],
                    [1544.5, 1024.0, 796.5, 992.0],
                    [547.5, 1024.0, 997.0, 992.0],
                    [0.0, 1024.0, 547.5, 992.0],
                    [502.0, 7168.0, 997.0, 992.0],
                    [1499.0, 7168.0, 796.5, 992.0],
                    [2295.5, 7168.0, 997.0, 992.0],
                    [3292.5, 7168.0, 547.5, 992.0],
                    [2790.5, 9216.0, 547.5, 992.0],
                    [0.0, 13312.0, 547.5, 992.0],
                ],
            ),
        ],
    );
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn preserved_spaces_keep_their_item_rounding_like_chrome() {
    let wrong = check(
        PRESERVED_SPACES,
        &["a", "c", "e", "g"],
        &[
            (
                (1, 1),
                vec![
                    [0.0, 0.0, 797.0, 1024.0],
                    [0.0, 2048.0, 797.0, 1024.0],
                    [0.0, 4096.0, 299.0, 1024.0],
                    [0.0, 6144.0, 797.0, 1024.0],
                ],
            ),
            (
                (7, 4),
                vec![
                    [0.0, 0.0, 796.5715, 987.4286],
                    [0.0, 2048.0, 796.5715, 987.4286],
                    [0.0, 4096.0, 298.8571, 987.4286],
                    [0.0, 6144.0005, 796.5715, 987.4286],
                ],
            ),
            (
                (2, 1),
                vec![
                    [0.0, 0.0, 796.5, 992.0],
                    [0.0, 2048.0, 796.5, 992.0],
                    [0.0, 4096.0, 298.5, 992.0],
                    [0.0, 6144.0, 796.5, 992.0],
                ],
            ),
        ],
    );
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
