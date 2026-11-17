mod scaled;
mod support;

use pageframe::App;
use scaled::{render_at_ratio, render_at_scale};
use support::{HEIGHT, WIDTH, open, pixel, render};

const BLUE: [u8; 4] = [0, 0, 255, 255];
const LIME: [u8; 4] = [0, 255, 0, 255];

#[test]
fn a_scale_change_recascades_fractional_double_border_widths() {
    let mut harness = open(App::new(
        (),
        "<style>html,body{margin:0}div{width:40px;height:24px;background:lime;border:2.5px double blue}</style><div id='box'></div>",
    ));
    let identifier = harness
        .base_mut()
        .query_selector_all("#box")
        .expect("box selector")[0];
    assert_eq!(
        harness
            .base_mut()
            .get_node(identifier)
            .unwrap()
            .final_layout()
            .size
            .width,
        44.0
    );
    assert_eq!(pixel(&render(&mut harness), 20, 1), BLUE);
    let buffer = render_at_scale(&mut harness, 2);
    let width = usize::try_from(WIDTH).expect("test width fits in usize") * 2;
    assert_eq!(
        harness
            .base_mut()
            .get_node(identifier)
            .unwrap()
            .final_layout()
            .size
            .width,
        45.0
    );
    for (row, expected) in [BLUE, BLUE, LIME, BLUE, BLUE, LIME].into_iter().enumerate() {
        let offset = (row * width + 20) * 4;
        assert_eq!(&buffer[offset..offset + 4], &expected, "row {row}");
    }
}

#[test]
fn one_device_pixel_groove_and_ridge_borders_use_the_border_color() {
    for style in ["groove", "ridge"] {
        let html = format!(
            "<style>html,body{{margin:0}}div{{width:40px;height:30px;background:lime;border:1px {style} blue}}</style><div></div>"
        );
        let mut harness = open(App::new((), &html));
        let buffer = render(&mut harness);
        for (horizontal, vertical) in [(20, 0), (0, 15), (41, 15), (20, 31)] {
            assert_eq!(pixel(&buffer, horizontal, vertical), BLUE, "{style}");
        }
    }
}

#[test]
fn one_device_pixel_groove_and_ridge_outlines_use_the_outline_color() {
    for style in ["groove", "ridge"] {
        let html = format!(
            "<style>html,body{{margin:0;background:white}}div{{margin:12px;width:40px;height:30px;background:lime;outline:1px {style} blue}}</style><div></div>"
        );
        let mut harness = open(App::new((), &html));
        let buffer = render(&mut harness);
        for (horizontal, vertical) in [(25, 11), (11, 25), (52, 25), (25, 42)] {
            assert_eq!(pixel(&buffer, horizontal, vertical), BLUE, "{style}");
        }
    }
}

#[test]
fn rounded_grooves_cover_the_background_between_shades() {
    let mut harness = open(App::new(
        (),
        "<style>html,body{margin:0;background:white}div{width:40px;height:30px;background:lime;border:4px groove blue;border-radius:8px}</style><div></div>",
    ));
    let buffer = render(&mut harness);
    for (horizontal, vertical, blue) in [(2, 5, 203), (5, 2, 201), (45, 5, 223), (5, 35, 224)] {
        let actual = pixel(&buffer, horizontal, vertical);
        for (channel, expected) in actual.into_iter().zip([0, 0, blue, 255]) {
            assert!(channel.abs_diff(expected) <= 2);
        }
    }
}

#[test]
fn mixed_groove_widths_leave_no_background_in_the_corners() {
    let mut harness = open(App::new(
        (),
        "<style>html,body{margin:0}div{width:40px;height:30px;background:lime;border-style:groove;border-color:blue;border-width:3px 2px}</style><div></div>",
    ));
    let buffer = render(&mut harness);
    for (horizontal, vertical, blue) in [(0, 34, 170), (1, 33, 226), (43, 0, 200), (42, 2, 199)] {
        let actual = pixel(&buffer, horizontal, vertical);
        for (channel, expected) in actual.into_iter().zip([0, 0, blue, 255]) {
            assert!(
                channel.abs_diff(expected) <= 6,
                "({horizontal}, {vertical}) {actual:?}"
            );
        }
    }
}

#[test]
fn translucent_grooves_paint_each_half_once() {
    for (extra, rows) in [
        ("border-width:3px 2px", 2..3),
        ("border-width:4px;border-radius:8px", 2..4),
    ] {
        let html = format!(
            "<style>html,body{{margin:0;background:white}}div{{width:40px;height:30px;background:lime;border-style:groove;border-color:rgba(0,0,255,.5);{extra}}}</style><div></div>"
        );
        let mut harness = open(App::new((), &html));
        let buffer = render(&mut harness);
        for vertical in rows {
            let actual = pixel(&buffer, 20, vertical);
            for (channel, expected) in actual.into_iter().zip([0, 127, 128, 255]) {
                assert!(
                    channel.abs_diff(expected) <= 2,
                    "{extra} row {vertical} {actual:?}"
                );
            }
        }
    }
}

#[test]
fn odd_groove_widths_keep_corner_halves_at_forty_five_degrees() {
    let mut harness = open(App::new(
        (),
        "<style>html,body{margin:0}div{width:40px;height:30px;background:lime;border:3px groove blue}</style><div></div>",
    ));
    let buffer = render(&mut harness);
    for (horizontal, vertical, blue) in [(45, 0, 213), (45, 1, 255), (43, 2, 213), (44, 2, 171)] {
        assert_eq!(pixel(&buffer, horizontal, vertical), [0, 0, blue, 255]);
    }
}

fn paint_at_seven_quarters<S: 'static>(
    harness: &mut pageframe_test_harness::Harness<pageframe::Page<S>>,
) -> impl Fn(usize, usize) -> [u8; 4] + use<S> {
    let buffer = render_at_ratio(harness, (7, 4), WIDTH);
    let stride = usize::try_from(WIDTH * 7 / 4).expect("test width fits in usize");
    move |column, row| {
        let offset = (row * stride + column) * 4;
        [
            buffer[offset],
            buffer[offset + 1],
            buffer[offset + 2],
            buffer[offset + 3],
        ]
    }
}

#[test]
fn odd_device_groove_widths_split_on_whole_pixels_at_seven_quarters_scale() {
    let mut harness = open(App::new(
        (),
        "<style>html,body{margin:0}div{position:absolute;left:8px;top:8px;width:40px;height:20px;background:lime;border:2px groove blue}</style><div></div>",
    ));
    let at = paint_at_seven_quarters(&mut harness);
    let sides = [
        [at(50, 14), at(50, 15), at(50, 16)],
        [at(14, 35), at(15, 35), at(16, 35)],
        [at(50, 52), at(50, 53), at(50, 54)],
        [at(87, 35), at(88, 35), at(89, 35)],
    ];
    let [dark, _, light] = sides[0];
    assert_ne!(dark, light);
    for side in sides {
        assert_eq!(side, [dark, dark, light]);
    }
    assert_eq!(at(50, 17), LIME);
    assert_eq!(at(50, 51), LIME);
    assert_eq!(at(90, 35), [0, 0, 0, 0]);
    assert_eq!(at(50, 55), [0, 0, 0, 0]);
}

fn top_and_bottom(style: &str) -> ([u8; 4], [u8; 4]) {
    let html = format!(
        "<style>html,body{{margin:0;background:white}}div{{box-sizing:border-box;width:40px;height:40px}}</style><div style='{style}'></div>"
    );
    let mut harness = open(App::new((), &html));
    let buffer = render(&mut harness);
    (pixel(&buffer, 20, 2), pixel(&buffer, 20, 37))
}

fn gray(level: u8) -> [u8; 4] {
    [level, level, level, 255]
}

#[test]
fn beveled_borders_shade_current_color_from_light_gray_like_chrome() {
    let cases = [
        ("color:gray;border:10px inset", gray(154), gray(238)),
        ("color:red;border:10px inset", gray(154), gray(238)),
        ("color:gray;border:10px outset", gray(238), gray(154)),
        ("color:gray;border:10px groove", gray(154), gray(238)),
        (
            "color:gray;border:10px inset color-mix(in srgb, currentcolor, white)",
            gray(162),
            gray(247),
        ),
        ("color:gray;border:10px inset black", gray(84), gray(168)),
        ("color:gray;border:10px solid", gray(128), gray(128)),
        (
            "color:gray;outline:10px inset;outline-offset:-10px",
            gray(44),
            gray(212),
        ),
        (
            "color:gray;border:10px inset;display:table",
            gray(44),
            gray(212),
        ),
        (
            "color:gray;border:10px inset;display:table-cell",
            gray(44),
            gray(212),
        ),
        (
            "color:gray;border:10px inset;display:table-caption",
            gray(44),
            gray(212),
        ),
        ("color:transparent;border:10px inset", gray(154), gray(238)),
    ];
    for (style, top, bottom) in cases {
        let (found_top, found_bottom) = top_and_bottom(style);
        for (found, expected) in [(found_top, top), (found_bottom, bottom)] {
            assert!(
                found
                    .iter()
                    .zip(expected)
                    .all(|(have, want)| have.abs_diff(want) <= 1),
                "{style}: {found:?} vs {expected:?}"
            );
        }
    }
}

#[test]
fn a_thin_groove_in_current_color_draws_light_gray() {
    let html = "<style>html,body{margin:0;background:white}</style><div style='color:gray;width:40px;height:30px;border:1px groove'></div><hr style='margin:0'>";
    let mut harness = open(App::new((), html));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 20, 0), gray(238));
    assert_eq!(pixel(&buffer, 20, 32), gray(154));
    assert_eq!(pixel(&buffer, 20, 33), gray(238));
}

#[test]
fn an_inline_inset_border_in_current_color_draws_light_gray() {
    let html = "<style>html,body{margin:0;background:white}</style><div style='font:20px/40px Arial;color:gray'><span style='border:10px inset'>&nbsp;&nbsp;</span></div>";
    let mut harness = open(App::new((), html));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 5, 20), gray(154));
}

#[test]
fn borders_snap_to_whole_device_pixels_at_seven_quarters_scale() {
    let mut harness = open(App::new(
        (),
        "<style>html,body{margin:0}div{position:absolute;left:10px;background:lime;border:0 solid blue}</style><div id='thin' style='top:10px;width:40px;height:20px;border-width:1px'></div><div id='thick' style='top:40px;width:0;height:10px;border-left-width:100px'></div>",
    ));
    let at = paint_at_seven_quarters(&mut harness);
    let device_width = |id: &str| {
        let doc = harness.base();
        let node = doc
            .query_selector_all(&format!("#{id}"))
            .expect("box selector")[0];
        doc.get_client_bounding_rect(node).expect("box rect").width * 1.75
    };
    assert!((device_width("thin") - 72.0).abs() < 1e-6);
    assert!((device_width("thick") - 175.0).abs() < 1e-6);
    for (column, row, expected) in [
        (40, 18, BLUE),
        (40, 19, LIME),
        (18, 35, BLUE),
        (19, 35, LIME),
        (88, 35, LIME),
        (89, 35, BLUE),
        (40, 53, LIME),
        (40, 54, BLUE),
    ] {
        assert_eq!(at(column, row), expected, "pixel {column},{row}");
    }
}

#[test]
fn computed_border_widths_report_whole_device_pixels_at_seven_quarters_scale() {
    let mut harness = open(App::new(
        (),
        "<style>div{border:solid #000;width:10px;height:10px;outline:3px solid red;columns:2;column-rule:solid 2px}</style><div id='one' style='border-width:1px'></div><div id='two' style='border-width:2px'></div><div id='zoomed' style='border-width:1px;zoom:2'></div><div id='none' style='border:5px none;outline:4px none'></div><div id='hidden' style='border:3px hidden'></div>",
    ));
    {
        let mut doc = harness.base_mut();
        let color_scheme = doc.get_viewport().color_scheme;
        doc.set_viewport(pageframe_traits::shell::Viewport::new(
            WIDTH * 7 / 4,
            HEIGHT * 7 / 4,
            1.75,
            color_scheme,
        ));
    }
    harness.pump();
    let doc = harness.base();
    let value = |id: &str, property: &str| {
        let node = doc
            .query_selector_all(&format!("#{id}"))
            .expect("box selector")[0];
        doc.resolved_style_value(node, property)
    };
    assert_eq!(value("one", "border-top-width"), "0.571429px");
    assert_eq!(value("one", "border-left-width"), "0.571429px");
    assert_eq!(value("two", "border-bottom-width"), "1.71429px");
    assert_eq!(value("two", "border-right-width"), "1.71429px");
    assert_eq!(value("one", "outline-width"), "2.85714px");
    assert_eq!(value("two", "column-rule-width"), "1.71429px");
    assert_eq!(value("one", "border-block-start-width"), "0.571429px");
    assert_eq!(value("zoomed", "border-top-width"), "0.857143px");
    assert_eq!(value("none", "border-top-width"), "0px");
    assert_eq!(value("hidden", "border-left-width"), "0px");
    assert_eq!(value("none", "outline-width"), "4px");
    assert_eq!(value("one", "border-width"), "0.571429px");
    assert_eq!(value("two", "border-top"), "1.71429px solid rgb(0, 0, 0)");
    assert_eq!(value("one", "outline"), "rgb(255, 0, 0) solid 2.85714px");
}
