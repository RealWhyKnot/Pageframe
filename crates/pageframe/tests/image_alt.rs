mod support;

use pageframe::App;
use support::{open, pixel, render};

fn size(attributes: &str) -> (f32, f32) {
    image_size(attributes, false)
}

fn image_size(attributes: &str, standards: bool) -> (f32, f32) {
    let doctype = if standards { "<!doctype html>" } else { "" };
    let html = format!(
        "{doctype}<style>html,body{{margin:0;background:white;font:16px/20px Arial}}</style><img id='image' {attributes}>"
    );
    let harness = open(App::new((), &html));
    let id = harness
        .base()
        .query_selector_all("#image")
        .expect("the selector parses")[0];
    let size = harness
        .base()
        .get_node(id)
        .expect("the node exists")
        .final_layout()
        .size;
    (size.width, size.height)
}

#[test]
fn failed_image_without_alt_reserves_the_icon_box() {
    assert_eq!(size("src='data:image/png;base64,AAAA'"), (16.0, 16.0));
}

#[test]
fn empty_alt_collapses_an_unsized_failed_image() {
    assert_eq!(size("src='data:image/png;base64,AAAA' alt=''"), (0.0, 0.0));
}

#[test]
fn absent_source_without_alt_collapses() {
    assert_eq!(size(""), (0.0, 0.0));
    assert_eq!(size("width='80'"), (80.0, 0.0));
}

#[test]
fn empty_source_with_one_dimension_is_square() {
    assert_eq!(size("src='' width='80'"), (80.0, 80.0));
    assert_eq!(size("src='' height='30'"), (30.0, 30.0));
}

#[test]
fn failed_image_with_one_dimension_is_square() {
    assert_eq!(
        size("src='data:image/png;base64,AAAA' width='80'"),
        (80.0, 80.0)
    );
    assert_eq!(
        size("src='data:image/png;base64,AAAA' height='30'"),
        (30.0, 30.0)
    );
}

#[test]
fn alt_text_gives_an_unsized_image_a_text_box() {
    let (width, height) = size("alt='Hello'");
    assert!((width - 52.46875).abs() < 0.95, "width {width}");
    assert_eq!(height, 20.0);
}

#[test]
fn both_dimensions_override_fallback_size() {
    for alt in ["", "alt=''", "alt='Hello'"] {
        assert_eq!(
            size(&format!(
                "src='data:image/png;base64,AAAA' width='80' height='30' {alt}"
            )),
            (80.0, 30.0)
        );
    }
}

#[test]
fn block_fallback_uses_the_available_width() {
    assert_eq!(
        size("src='data:image/png;base64,AAAA' style='display:block'"),
        (120.0, 16.0)
    );
    assert_eq!(
        size("src='data:image/png;base64,AAAA' alt='' style='display:block'"),
        (120.0, 0.0)
    );
    assert_eq!(size("alt='Hello' style='display:block'"), (120.0, 20.0));
}

#[test]
fn fallback_paints_text_and_the_sized_frame() {
    let mut harness = open(App::new(
        (),
        "<style>html,body{margin:0;background:white;font:16px/20px Arial;color:red}</style><img alt='Hello' width='80' height='30'>",
    ));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 40, 0), [192, 192, 192, 255]);
    assert!((3..20).any(|y| (18..75).any(|x| {
        let p = pixel(&buffer, x, y);
        p[0] > 180 && p[1] < 120 && p[2] < 120
    })));
    assert_eq!(pixel(&buffer, 81, 10), [255, 255, 255, 255]);
}

#[test]
fn oversized_alt_stays_inside_a_sized_box() {
    let text = "long alternative text ".repeat(4096);
    let mut harness = open(App::new(
        (),
        &format!(
            "<style>html,body{{margin:0;background:white;font:16px/20px Arial}}</style><img alt='{text}' width='80' height='30'>"
        ),
    ));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 81, 10), [255, 255, 255, 255]);
    assert_eq!(pixel(&buffer, 10, 31), [255, 255, 255, 255]);
}

#[test]
fn missing_file_uses_the_failed_fallback() {
    assert_eq!(
        size("src='file:///C:/pageframe-nonexistent-alt-probe.png'"),
        (16.0, 16.0)
    );
}

#[test]
fn changing_and_removing_alt_rebuilds_the_fallback() {
    let mut harness = open(App::new(
        (),
        "<style>html,body{margin:0;background:white;font:16px/20px Arial}</style><img id='image' alt='Hello'>",
    ));
    let id = harness
        .base()
        .query_selector_all("#image")
        .expect("the selector parses")[0];
    let name = pageframe_dom::QualName::new(
        None,
        pageframe_dom::ns!(),
        pageframe_dom::local_name!("alt"),
    );
    harness
        .base_mut()
        .mutate()
        .set_attribute(id, name.clone(), "");
    harness.pump();
    assert_eq!(
        harness
            .base()
            .get_node(id)
            .expect("the node exists")
            .final_layout()
            .size
            .width,
        0.0
    );
    harness
        .base_mut()
        .mutate()
        .set_attribute(id, name.clone(), "Hello");
    harness.pump();
    assert!(
        harness
            .base()
            .get_node(id)
            .expect("the node exists")
            .final_layout()
            .size
            .width
            > 50.0
    );
    harness.base_mut().mutate().clear_attribute(id, name);
    harness.pump();
    assert_eq!(
        harness
            .base()
            .get_node(id)
            .expect("the node exists")
            .final_layout()
            .size
            .width,
        0.0
    );
}

#[test]
fn standards_inline_alt_ignores_width_and_height() {
    for dimensions in [
        "width='80'",
        "height='30'",
        "width='80' height='30'",
        "style='width:80px;height:30px'",
    ] {
        let (width, height) =
            image_size(&format!("src='data:,bad' alt='Hello' {dimensions}"), true);
        assert!((width - 52.46875).abs() < 0.95, "width {width}");
        assert_eq!(height, 20.0);
    }
}

#[test]
fn standards_missing_alt_needs_both_dimensions() {
    assert_eq!(image_size("src='data:,bad' width='80'", true), (16.0, 16.0));
    assert_eq!(
        image_size("src='data:,bad' width='80' height='30'", true),
        (80.0, 30.0)
    );
    assert_eq!(image_size("src='' width='80'", true), (0.0, 0.0));
}

#[test]
fn standards_block_and_inline_block_alt_keep_their_sizes() {
    for display in ["block", "inline-block"] {
        assert_eq!(
            image_size(
                &format!(
                    "src='data:,bad' alt='Hello' style='display:{display};width:80px;height:30px'"
                ),
                true
            ),
            (80.0, 30.0)
        );
    }
}

#[test]
fn icon_on_its_own_line_uses_sixteen_pixels() {
    for standards in [false, true] {
        assert_eq!(
            image_size(
                "src='data:,bad' alt='Hello' style='max-width:30px'",
                standards
            ),
            (30.0, 36.0)
        );
    }
}

fn dark_pixels(buffer: &[u8]) -> usize {
    buffer
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|rgba| rgba[0] < 96 && rgba[1] < 96 && rgba[2] < 96)
        .count()
}

#[test]
fn alt_text_grows_with_the_viewport_scale() {
    let mut harness = open(App::new(
        (),
        "<!doctype html><style>html,body{margin:0;background:white;font:16px/20px Arial}</style><img id='image' alt='Hello' src='data:,bad'>",
    ));
    let before = dark_pixels(&render(&mut harness));
    let scheme = harness.base().get_viewport().color_scheme;
    harness
        .base_mut()
        .set_viewport(pageframe_traits::shell::Viewport::new(
            support::WIDTH * 2,
            support::HEIGHT * 2,
            2.0,
            scheme,
        ));
    harness.pump();
    let mut doc = harness.base_mut();
    let after = dark_pixels(&anyrender::render_to_buffer::<
        anyrender_vello_cpu::VelloCpuImageRenderer,
        _,
    >(
        |scene| {
            pageframe_paint::paint_scene(
                scene,
                &mut doc,
                2.0,
                support::WIDTH * 2,
                support::HEIGHT * 2,
                0,
                0,
            );
        },
        support::WIDTH * 2,
        support::HEIGHT * 2,
    ));
    assert!(before > 20, "{before}");
    assert!(after > before * 3, "{before} {after}");
}

#[test]
fn inline_alt_text_sits_on_the_line_baseline() {
    let harness = open(App::new(
        (),
        "<!doctype html><style>html,body{margin:0;background:white;font:16px/20px monospace}</style><div id='image'><img alt='Hello' src='data:,bad'></div>",
    ));
    let id = harness
        .base()
        .query_selector_all("#image")
        .expect("the selector parses")[0];
    let size = harness
        .base()
        .get_node(id)
        .expect("the node exists")
        .final_layout()
        .size;
    assert_eq!(size.height, 20.0);
}

#[test]
fn a_source_that_cannot_be_resolved_shows_the_alt_text() {
    let (width, height) = image_size("src='nope.png' alt='Hello'", true);
    assert!(width > 20.0, "{width}");
    assert_eq!(height, 20.0);
}

#[test]
fn changing_and_removing_the_source_swaps_image_and_fallback() {
    const TALL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAoAAAAeCAIAAACaFxhnAAAAHUlEQVR4nGP4z8CAB+GTG5UelNKD2nGj0gykSgMA7ngq5Bho73QAAAAASUVORK5CYII=";
    let mut harness = open(App::new(
        (),
        "<!doctype html><style>html,body{margin:0;background:white;font:16px/20px Arial}</style><img id='image' src='data:,bad'>",
    ));
    let id = harness
        .base()
        .query_selector_all("#image")
        .expect("the selector parses")[0];
    let name = pageframe_dom::QualName::new(
        None,
        pageframe_dom::ns!(),
        pageframe_dom::local_name!("src"),
    );
    let mut sizes = Vec::new();
    for step in 0..3 {
        match step {
            1 => harness
                .base_mut()
                .mutate()
                .set_attribute(id, name.clone(), TALL),
            2 => harness
                .base_mut()
                .mutate()
                .clear_attribute(id, name.clone()),
            _ => {}
        }
        harness.pump();
        let size = harness
            .base()
            .get_node(id)
            .expect("the node exists")
            .final_layout()
            .size;
        sizes.push((size.width, size.height));
    }
    assert_eq!(sizes, [(16.0, 16.0), (10.0, 30.0), (0.0, 0.0)]);
}

fn line_and_box(doctype: &str, content: &str) -> (f32, f32) {
    let html = format!(
        "{doctype}<style>html,body{{margin:0;background:white;font:16px/20px Arial}}</style><div id='line'>{content}</div>"
    );
    let harness = open(App::new((), &html));
    let base = harness.base();
    let layout = |selector: &str| {
        let id = base
            .query_selector_all(selector)
            .expect("the selector parses")[0];
        let layout = base.get_node(id).expect("the node exists").final_layout();
        (layout.size.height, layout.location.y)
    };
    (layout("#line").0, layout("#box").1)
}

#[test]
fn a_collapsed_image_still_takes_a_line_in_standards_mode() {
    assert_eq!(
        line_and_box("<!doctype html>", "<img id='box' src=''>"),
        (20.0, 15.0)
    );
}

#[test]
fn a_collapsed_image_takes_no_line_in_quirks_mode() {
    assert_eq!(line_and_box("", "<img id='box' src=''>"), (0.0, 0.0));
}

#[test]
fn an_empty_inline_block_takes_a_line() {
    assert_eq!(
        line_and_box(
            "<!doctype html>",
            "<span id='box' style='display:inline-block'></span>"
        ),
        (20.0, 15.0)
    );
    let input = "<input id='box' style='width:0;height:0;border:0;padding:0;margin:0'>";
    assert_eq!(line_and_box("<!doctype html>", input).0, 20.0);
}

#[test]
fn a_float_or_positioned_box_alone_takes_no_line() {
    for style in ["float:left", "position:absolute"] {
        let content = format!("<span id='box' style='{style}'></span>");
        assert_eq!(line_and_box("<!doctype html>", &content), (0.0, 0.0));
    }
}

#[test]
fn wrapped_alt_text_sits_on_its_last_line() {
    let content = "<img id='box' src='data:,bad' alt='Hello' style='max-width:30px'>";
    assert_eq!(line_and_box("<!doctype html>", content), (36.0, 0.0));
}

#[test]
fn standards_alt_with_one_dimension_has_no_frame() {
    for display in ["block", "inline-block"] {
        let attributes =
            format!("src='data:,bad' alt='Hello there you' style='display:{display};width:60px'");
        assert_eq!(image_size(&attributes, true), (60.0, 60.0));
    }
}

#[test]
fn sized_alt_text_sits_on_its_last_line_in_standards_mode() {
    let sized = "<img id='box' src='data:,bad' alt='Hello there you' style='display:inline-block;width:60px;height:50px'>";
    assert_eq!(line_and_box("<!doctype html>", sized), (60.0, 0.0));
    let padded = "<img id='box' src='data:,bad' alt='Hello' style='display:inline-block;width:80px;height:30px;padding:3px;border:2px solid red'>";
    assert_eq!(line_and_box("<!doctype html>", padded), (40.0, 0.0));
}

fn icon_outline(doctype: &str, attributes: &str, x: usize, y: usize) -> [u8; 4] {
    let mut harness = open(App::new(
        (),
        &format!(
            "{doctype}<style>html,body{{margin:0;background:white;font:16px/20px Arial}}</style><img {attributes}>"
        ),
    ));
    pixel(&render(&mut harness), x, y)
}

#[test]
fn a_framed_fallback_paints_its_icon_inside_the_frame() {
    let outline = [150, 150, 150, 255];
    let attributes = "src='data:,bad' alt='Hello' width='80' height='30'";
    assert_eq!(icon_outline("", attributes, 2, 12), outline);
    assert_eq!(icon_outline("", attributes, 17, 12), outline);
    assert_eq!(icon_outline("", "src='' width='80'", 2, 12), outline);
    assert_eq!(
        icon_outline("", "src='data:,bad' alt='' height='30'", 2, 12),
        outline
    );
}

#[test]
fn standards_alt_text_paints_its_icon_at_the_box_corner() {
    let outline = [150, 150, 150, 255];
    let attributes =
        "src='data:,bad' alt='Hello' width='80' height='30' style='display:inline-block'";
    assert_eq!(icon_outline("<!doctype html>", attributes, 0, 10), outline);
    assert_eq!(icon_outline("<!doctype html>", attributes, 15, 10), outline);
}

#[test]
fn standards_images_without_alt_text_need_both_dimensions_for_an_icon() {
    assert_eq!(image_size("src='' width='80'", true), (0.0, 0.0));
    assert_eq!(
        image_size(
            "src='data:,bad' alt='' style='display:inline-block;width:60px'",
            true
        ),
        (60.0, 0.0)
    );
    let outline = [150, 150, 150, 255];
    let attributes = "src='' width='80' height='30'";
    assert_eq!(icon_outline("<!doctype html>", attributes, 2, 12), outline);
}

fn link_pixel(attributes: &str, x: usize, y: usize) -> [u8; 4] {
    let mut harness = open(App::new(
        (),
        &format!(
            "<style>html,body{{margin:0;background:white;font:16px/20px Arial}}</style><a href='#x'><img src='data:,bad' alt='Hello link' {attributes}></a>"
        ),
    ));
    pixel(&render(&mut harness), x, y)
}

#[test]
fn alt_text_inside_a_link_is_underlined_past_the_icon() {
    let link = [0, 0, 238, 255];
    let white = [255, 255, 255, 255];
    assert_eq!(link_pixel("", 20, 16), link);
    assert_eq!(link_pixel("", 60, 16), link);
    assert_eq!(link_pixel("", 8, 16), white);
    assert_eq!(link_pixel("style='display:block'", 20, 16), link);
}

#[test]
fn an_atomic_alt_box_keeps_the_link_underline_out() {
    let white = [255, 255, 255, 255];
    assert_eq!(link_pixel("style='display:inline-block'", 20, 16), white);
    assert_eq!(link_pixel("width='120' height='40'", 22, 18), white);
    assert_eq!(link_pixel("style='float:left'", 20, 16), white);
}

#[test]
fn a_contents_wrapper_lets_the_link_underline_through() {
    let mut harness = open(App::new(
        (),
        "<style>html,body{margin:0;background:white;font:16px/20px Arial}</style><a href='#x'><span style='display:contents'><img src='data:,bad' alt='Hello link'></span></a>",
    ));
    assert_eq!(pixel(&render(&mut harness), 20, 16), [0, 0, 238, 255]);
}
