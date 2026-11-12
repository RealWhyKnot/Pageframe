mod support;

use pageframe::{App, EventKind};
use support::{open, pixel, render};

const FILL: [u8; 4] = [204, 68, 0, 255];
const PAGE: [u8; 4] = [255; 4];

fn clipped(clip: &str, style: &str) -> Vec<u8> {
    let html = format!(
        r#"<!DOCTYPE html><html><head><style>html,body{{margin:0;background:white}}</style></head><body><svg width="0" height="0" style="position:absolute"><defs>{clip}</defs></svg><div style="position:absolute;left:0;top:0;width:100px;height:60px;background:#c40;clip-path:url(#c);{style}"></div></body></html>"#
    );
    let mut harness = open(App::new((), &html));
    render(&mut harness)
}

fn colors(clip: &str, points: [(usize, usize); 3]) -> [[u8; 4]; 3] {
    let buffer = clipped(clip, "");
    points.map(|(x, y)| pixel(&buffer, x, y))
}

#[test]
fn basic_shapes_clip_in_user_space() {
    let cases = [
        (
            r#"<clipPath id="c"><circle cx="50" cy="30" r="28"/></clipPath>"#,
            [(50, 30), (5, 5), (90, 30)],
            [FILL, PAGE, PAGE],
        ),
        (
            r#"<clipPath id="c"><rect x="10" y="5" width="80" height="50" rx="12"/></clipPath>"#,
            [(50, 30), (11, 6), (5, 30)],
            [FILL, PAGE, PAGE],
        ),
        (
            r#"<clipPath id="c"><rect x="20" y="10" width="60" height="40" ry="30"/></clipPath>"#,
            [(50, 30), (23, 13), (77, 47)],
            [FILL, PAGE, PAGE],
        ),
        (
            r#"<clipPath id="c"><ellipse cx="50" cy="30" rx="45" ry="20"/></clipPath>"#,
            [(10, 30), (50, 5), (10, 15)],
            [FILL, PAGE, PAGE],
        ),
        (
            r#"<clipPath id="c"><polygon points="50,0 100,60 0,60"/></clipPath>"#,
            [(50, 30), (10, 10), (90, 10)],
            [FILL, PAGE, PAGE],
        ),
        (
            r#"<clipPath id="c"><polyline points="0,0 100,0 50,60"/><line x1="0" y1="0" x2="100" y2="60"/></clipPath>"#,
            [(50, 20), (10, 50), (90, 50)],
            [FILL, PAGE, PAGE],
        ),
        (
            r#"<clipPath id="c"><path d="M10 10 H90 V50 Q50 20 10 50 Z"/></clipPath>"#,
            [(50, 20), (50, 45), (5, 30)],
            [FILL, PAGE, PAGE],
        ),
        (
            r#"<clipPath id="c"><rect x="0" y="0" width="40" height="30"/><circle cx="75" cy="35" r="20"/></clipPath>"#,
            [(20, 15), (75, 35), (50, 45)],
            [FILL, FILL, PAGE],
        ),
    ];
    for (clip, points, expected) in cases {
        assert_eq!(colors(clip, points), expected, "{clip}");
    }
}

#[test]
fn transforms_move_the_clip() {
    let cases = [
        (
            r#"<clipPath id="c"><rect width="40" height="30" transform="translate(30 15)"/></clipPath>"#,
            [(50, 30), (20, 30), (50, 50)],
            [FILL, PAGE, PAGE],
        ),
        (
            r#"<clipPath id="c" transform="scale(0.5) translate(20,10)"><rect width="100" height="60"/></clipPath>"#,
            [(30, 20), (70, 30), (5, 20)],
            [FILL, PAGE, PAGE],
        ),
        (
            r#"<clipPath id="c"><rect width="40" height="30" transform="matrix(1 0 0 1 50 20)"/></clipPath>"#,
            [(70, 35), (40, 35), (70, 10)],
            [FILL, PAGE, PAGE],
        ),
        (
            r#"<clipPath id="c"><rect x="-90" y="10" width="40" height="30" transform="scale(-1 1)"/></clipPath>"#,
            [(70, 25), (30, 25), (95, 25)],
            [FILL, PAGE, PAGE],
        ),
    ];
    for (clip, points, expected) in cases {
        assert_eq!(colors(clip, points), expected, "{clip}");
    }
}

#[test]
fn bounding_box_units_scale_to_the_border_box() {
    let circle = r#"<clipPath id="c" clipPathUnits="objectBoundingBox"><circle cx="0.5" cy="0.5" r="0.5"/></clipPath>"#;
    assert_eq!(
        colors(circle, [(4, 30), (50, 2), (4, 4)]),
        [FILL, FILL, PAGE]
    );

    let rect = r#"<clipPath id="c" clipPathUnits="objectBoundingBox"><rect x="0.25" y="0.25" width="0.5" height="0.5"/></clipPath>"#;
    let buffer = clipped(rect, "width:80px;height:40px;border:10px solid #c40");
    assert_eq!(pixel(&buffer, 27, 17), FILL);
    assert_eq!(pixel(&buffer, 72, 42), FILL);
    assert_eq!(pixel(&buffer, 22, 30), PAGE);
    assert_eq!(pixel(&buffer, 50, 47), PAGE);
}

#[test]
fn user_space_starts_at_the_border_box() {
    let clip = r#"<clipPath id="c"><rect width="20" height="20"/></clipPath>"#;
    let buffer = clipped(clip, "border:10px solid #06c;padding:5px");
    assert_eq!(pixel(&buffer, 5, 5), [0, 102, 204, 255]);
    assert_eq!(pixel(&buffer, 15, 15), FILL);
    assert_eq!(pixel(&buffer, 25, 15), PAGE);
}

#[test]
fn clip_rule_decides_what_a_path_keeps() {
    let cases = [
        (
            r#"<clipPath id="c"><path clip-rule="evenodd" d="M5 5 H95 V55 H5 Z M30 20 H70 V40 H30 Z"/></clipPath>"#,
            [(15, 30), (50, 30), (2, 30)],
            [FILL, PAGE, PAGE],
        ),
        (
            r#"<clipPath id="c" clip-rule="evenodd"><path d="M5 5 H95 V55 H5 Z M30 20 H70 V40 H30 Z"/></clipPath>"#,
            [(15, 30), (50, 30), (2, 30)],
            [FILL, PAGE, PAGE],
        ),
        (
            r#"<clipPath id="c"><path d="M5 5 H95 V55 H5 Z M30 20 H70 V40 H30 Z"/></clipPath>"#,
            [(15, 30), (50, 30), (2, 30)],
            [FILL, FILL, PAGE],
        ),
        (
            r#"<clipPath id="c"><path d="M5 5 H95 V55 H5 Z M30 20 V40 H70 V20 Z"/></clipPath>"#,
            [(15, 30), (50, 30), (2, 30)],
            [FILL, PAGE, PAGE],
        ),
    ];
    for (clip, points, expected) in cases {
        assert_eq!(colors(clip, points), expected, "{clip}");
    }
}

#[test]
fn overlapping_shapes_merge_the_way_chrome_does() {
    let cases = [
        (
            r#"<clipPath id="c"><rect width="60" height="40"/><polygon points="40,20 40,60 100,60 100,20"/></clipPath>"#,
            [(50, 30), (20, 10), (80, 10)],
            [FILL, FILL, PAGE],
        ),
        (
            r#"<clipPath id="c"><path clip-rule="evenodd" d="M5 5 H95 V55 H5 Z M30 20 H70 V40 H30 Z"/><rect x="20" y="10" width="30" height="20"/></clipPath>"#,
            [(25, 15), (40, 25), (80, 10)],
            [PAGE, FILL, FILL],
        ),
        (
            r#"<clipPath id="c"><rect x="20" y="10" width="30" height="20"/><path clip-rule="evenodd" d="M5 5 H95 V55 H5 Z M30 20 H70 V40 H30 Z"/></clipPath>"#,
            [(25, 15), (40, 25), (60, 30)],
            [FILL, FILL, PAGE],
        ),
        (
            r#"<clipPath id="c"><path d="M5 5 H95 V55 H5 Z M30 20 V40 H70 V20 Z"/><rect x="20" y="10" width="30" height="20"/></clipPath>"#,
            [(25, 15), (40, 25), (80, 10)],
            [PAGE, FILL, FILL],
        ),
        (
            r#"<clipPath id="c"><rect width="60" height="40"/><path d="M40 20 V60 H100 V20 Z M55 30 H85 V50 H55 Z"/></clipPath>"#,
            [(50, 25), (57, 35), (70, 40)],
            [FILL, FILL, PAGE],
        ),
        (
            r#"<clipPath id="c"><path clip-rule="evenodd" d="M5 5 H95 V55 H5 Z M30 20 H70 V40 H30 Z"/><circle cx="35" cy="20" r="12"/></clipPath>"#,
            [(35, 12), (38, 25), (80, 10)],
            [PAGE, FILL, FILL],
        ),
    ];
    for (clip, points, expected) in cases {
        assert_eq!(colors(clip, points), expected, "{clip}");
    }
}

#[test]
fn a_clip_with_nothing_to_draw_hides_the_box() {
    let clip = r#"<clipPath id="c"><g><rect width="30" height="30"/></g><rect x="60" y="20" width="30" height="30" style="display:none"/><rect x="40" y="35" width="30" height="20" visibility="hidden"/></clipPath>"#;
    assert_eq!(
        colors(clip, [(15, 15), (75, 35), (55, 45)]),
        [PAGE, PAGE, PAGE]
    );
    assert_eq!(
        colors(
            r#"<clipPath id="c"></clipPath>"#,
            [(15, 15), (75, 35), (55, 45)]
        ),
        [PAGE, PAGE, PAGE]
    );
}

#[test]
fn a_reference_to_something_else_does_not_clip() {
    let gradient = r#"<linearGradient id="c"></linearGradient>"#;
    assert_eq!(
        colors(gradient, [(2, 2), (50, 30), (98, 58)]),
        [FILL, FILL, FILL]
    );
}

#[test]
fn clicks_outside_the_clip_fall_through() {
    let html = r#"<!DOCTYPE html><html><head><style>html,body{margin:0;background:white}</style></head><body><svg width="0" height="0" style="position:absolute"><clipPath id="c"><circle cx="30" cy="30" r="20"/></clipPath></svg><div id="under" style="position:absolute;inset:0"></div><div id="front" style="position:absolute;left:0;top:0;width:100px;height:60px;background:#c40;clip-path:url(#c)"></div></body></html>"#;
    let app = App::new(Vec::new(), html)
        .on("#front", EventKind::Click, |seen, _, _| seen.push("front"))
        .on("#under", EventKind::Click, |seen, _, _| seen.push("under"));
    let mut harness = open(app);
    harness.click_at(80.0, 30.0);
    assert_eq!(harness.doc.state().last(), Some(&"under"));
    harness.click_at(30.0, 30.0);
    assert_eq!(harness.doc.state().last(), Some(&"front"));
}
