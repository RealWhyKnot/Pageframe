mod scaled;
mod support;

use pageframe::{App, EventKind};
use scaled::render_at_scale;
use support::{HEIGHT, WIDTH, open, pixel, render};

const SWATCH: &str = r#"<html><body style="margin:0;background:white">
<div id="swatch" style="width:40px;height:40px;background:rgb(255,0,0)"></div>
</body></html>"#;

#[test]
fn css_paints_to_pixels() {
    let mut harness = open(App::new((), SWATCH));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 20, 20), [255, 0, 0, 255]);
    assert_eq!(pixel(&buffer, 80, 60), [255, 255, 255, 255]);
}

#[test]
fn click_handler_change_is_painted() {
    let app = App::new((), SWATCH)
        .stylesheet(".on { background: rgb(0, 0, 255) !important; }")
        .on("#swatch", EventKind::Click, |(), ui, _| {
            ui.select("#swatch").add_class("on");
        });
    let mut harness = open(app);
    harness.click("#swatch");
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 20, 20), [0, 0, 255, 255]);
}

const FIXED_LAYOUT: &str = r#"<html><body style="margin:0">
<input id="field" style="width:100px">
<div id="label" style="width:100px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">Short</div>
<div id="after" style="height:10px"></div>
</body></html>"#;

#[test]
fn oversized_input_keeps_layout_fixed() {
    let mut harness = open(App::new((), FIXED_LAYOUT));
    let field = harness.layout_rect("#field");
    let label = harness.layout_rect("#label");
    let after = harness.layout_rect("#after");

    let long = "W".repeat(20_000);
    let mut ui = harness.doc.ui();
    ui.select("#field").set_value(&long);
    ui.select("#label").set_text(&long);
    harness.pump();

    assert_eq!(harness.layout_rect("#field"), field);
    assert_eq!(harness.layout_rect("#label"), label);
    assert_eq!(harness.layout_rect("#after").y, after.y);
    assert_eq!(
        harness.doc.ui().select("#field").value().map(|v| v.len()),
        Some(20_000)
    );
}

#[test]
fn typed_overflow_keeps_input_width() {
    let mut harness = open(App::new((), FIXED_LAYOUT));
    let field = harness.layout_rect("#field");
    harness.click("#field");
    harness.type_text(&"x".repeat(300));
    harness.pump();
    assert_eq!(harness.layout_rect("#field"), field);
}

const INLINE_SVG: &str = r#"<html><body style="margin:0;background:white">
<svg width="40" height="40" viewBox="0 0 40 40"><rect width="40" height="40" fill="rgb(0,128,0)"/></svg>
</body></html>"#;

#[test]
fn inline_svg_is_painted() {
    let mut harness = open(App::new((), INLINE_SVG));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 20, 20), [0, 128, 0, 255]);
    assert_eq!(pixel(&buffer, 80, 60), [255, 255, 255, 255]);
}

const SVG_CSS_PAINT: &str = r#"<html><head><style>
#fill { fill: rgb(0, 0, 255); }
</style></head><body style="margin:0;background:white">
<svg width="120" height="40" viewBox="0 0 120 40">
<rect id="fill" width="40" height="40" fill="rgb(0,128,0)"/>
<rect x="65" y="5" width="30" height="30" fill="none" style="stroke: rgb(0, 0, 255); stroke-width: 10"/>
</svg>
</body></html>"#;

#[test]
fn svg_takes_fill_and_stroke_from_css() {
    let mut harness = open(App::new((), SVG_CSS_PAINT));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 20, 20), [0, 0, 255, 255]);
    assert_eq!(pixel(&buffer, 65, 20), [0, 0, 255, 255]);
    assert_eq!(pixel(&buffer, 80, 20), [255, 255, 255, 255]);
}

const SVG_GRADIENT: &str = r#"<html><body style="margin:0;background:white">
<svg width="40" height="40" viewBox="0 0 40 40">
<defs><linearGradient id="solid"><stop offset="0" stop-color="rgb(0,0,255)"/><stop offset="1" stop-color="rgb(0,0,255)"/></linearGradient></defs>
<rect width="40" height="40" fill="url(#solid)"/>
</svg>
</body></html>"#;

#[test]
fn svg_gradient_fill_still_resolves() {
    let mut harness = open(App::new((), SVG_GRADIENT));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 20, 20), [0, 0, 255, 255]);
}

const SVG_USE_AND_FALLBACK: &str = r##"<html><head><style>
#css-use { fill: rgb(0,0,255) }
</style></head><body style="margin:0;background:white">
<svg width="120" height="40" viewBox="0 0 120 40">
<defs><rect id="box" width="40" height="40"/></defs>
<use href="#box" fill="rgb(255,0,0)"/>
<use id="css-use" href="#box" x="40"/>
<rect x="80" width="40" height="40" fill="url(#missing) rgb(0,128,0)"/>
</svg>
</body></html>"##;

#[test]
fn svg_use_inherits_paint_and_missing_url_falls_back() {
    let mut harness = open(App::new((), SVG_USE_AND_FALLBACK));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 20, 20), [255, 0, 0, 255]);
    assert_eq!(pixel(&buffer, 60, 20), [0, 0, 255, 255]);
    assert_eq!(pixel(&buffer, 100, 20), [0, 128, 0, 255]);
}

const SVG_USE_DECLARED_PAINT: &str = r##"<html><head><style>
.solid { fill: rgb(0,0,0) }
</style></head><body style="margin:0;background:white">
<svg width="80" height="40" viewBox="0 0 80 40">
<defs>
<rect id="faded" width="40" height="40" style="fill-opacity:1"/>
<rect id="solid" class="solid" width="40" height="40"/>
</defs>
<use href="#faded" fill="rgb(255,0,0)"/>
<use href="#solid" x="40" fill="rgb(255,0,0)"/>
</svg>
</body></html>"##;

#[test]
fn svg_use_keeps_paint_declared_on_the_referenced_element() {
    let mut harness = open(App::new((), SVG_USE_DECLARED_PAINT));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 20, 20), [255, 0, 0, 255]);
    assert_eq!(pixel(&buffer, 60, 20), [0, 0, 0, 255]);
}

fn shadow_page(shadow: &str) -> String {
    format!(
        r#"<html><body style="margin:0;background:white">
<div style="font-size:40px;line-height:60px;padding:20px;color:transparent;text-shadow:{shadow}">HHH</div>
</body></html>"#
    )
}

fn painted_pixels(buffer: &[u8]) -> Vec<[u8; 4]> {
    buffer
        .as_chunks::<4>()
        .0
        .iter()
        .copied()
        .filter(|px| *px != [255, 255, 255, 255])
        .collect()
}

#[test]
fn text_shadow_paints_offset_glyphs() {
    let mut harness = open(App::new((), &shadow_page("none")));
    assert_eq!(painted_pixels(&render(&mut harness)), Vec::<[u8; 4]>::new());

    let mut harness = open(App::new((), &shadow_page("10px 10px 0 rgb(255,0,0)")));
    let sharp = painted_pixels(&render(&mut harness));
    assert!(sharp.contains(&[255, 0, 0, 255]));
    assert!(sharp.iter().all(|px| px[1] == px[2] && px[0] == 255));

    let mut harness = open(App::new((), &shadow_page("10px 10px 8px rgb(255,0,0)")));
    let blurred = painted_pixels(&render(&mut harness));
    assert!(blurred.len() > sharp.len());
    assert!(!blurred.contains(&[255, 0, 0, 255]));
}

#[test]
fn first_text_shadow_is_painted_on_top() {
    let page = shadow_page("10px 10px 0 rgb(255,0,0), 10px 10px 0 rgb(0,0,255)");
    let mut harness = open(App::new((), &page));
    let pixels = painted_pixels(&render(&mut harness));
    assert!(pixels.contains(&[255, 0, 0, 255]));
    assert!(!pixels.contains(&[0, 0, 255, 255]));
}

fn overflow_page(text_overflow: &str) -> String {
    format!(
        r#"<html><body style="margin:0;background:white">
<div style="width:100px;font-size:20px;line-height:30px;white-space:nowrap;overflow:hidden;text-overflow:{text_overflow}">HHHHHHHHHHHHHHHH</div>
</body></html>"#
    )
}

fn inked_rows(buffer: &[u8], columns: std::ops::Range<usize>) -> Vec<usize> {
    let height = usize::try_from(HEIGHT).expect("test height fits in usize");
    (0..height)
        .filter(|&y| {
            columns
                .clone()
                .any(|x| pixel(buffer, x, y) != [255, 255, 255, 255])
        })
        .collect()
}

fn inked_columns(buffer: &[u8]) -> Vec<usize> {
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    let height = usize::try_from(HEIGHT).expect("test height fits in usize");
    (0..width)
        .filter(|&x| (0..height).any(|y| pixel(buffer, x, y) != [255, 255, 255, 255]))
        .collect()
}

#[test]
fn text_overflow_ellipsis_replaces_the_cut_off_end() {
    let mut harness = open(App::new((), &overflow_page("clip")));
    let clipped = render(&mut harness);
    let mut harness = open(App::new((), &overflow_page("ellipsis")));
    let ellipsis = render(&mut harness);

    let clipped_rows = inked_rows(&clipped, 0..100);
    let midline = clipped_rows[clipped_rows.len() / 2];
    let right = *inked_columns(&ellipsis)
        .last()
        .expect("the ellipsis line has ink");
    assert!(right < 100);
    assert_eq!(inked_columns(&clipped).last(), Some(&99));

    let dots = inked_rows(&ellipsis, right - 4..right + 1);
    assert_ne!(dots, Vec::<usize>::new());
    assert!(dots.iter().all(|&y| y > midline));
    assert!(inked_rows(&clipped, right - 4..right + 1).contains(&midline));
}

#[test]
fn text_overflow_ellipsis_shows_on_a_scaled_page() {
    let mut harness = open(App::new((), &overflow_page("clip")));
    let clipped = render_at_scale(&mut harness, 2);
    let mut harness = open(App::new((), &overflow_page("ellipsis")));
    let ellipsis = render_at_scale(&mut harness, 2);
    assert!(clipped != ellipsis, "the ellipsis is cut off");
}

#[test]
fn right_to_left_text_overflow_ellipsis_cuts_the_left_end() {
    let page = overflow_page("ellipsis").replace("<div style", r#"<div dir="rtl" style"#);
    let mut harness = open(App::new((), &page));
    let buffer = render(&mut harness);

    let rows = inked_rows(&buffer, 0..100);
    let midline = rows[rows.len() / 2];
    let left = *inked_columns(&buffer)
        .first()
        .expect("the ellipsis line has ink");
    assert!(left > 0);
    assert!(
        inked_columns(&buffer)
            .last()
            .is_some_and(|&right| right > 90)
    );

    let dots = inked_rows(&buffer, left..left + 5);
    assert_ne!(dots, Vec::<usize>::new());
    assert!(dots.iter().all(|&y| y > midline));
}

const LINE_CLAMP: &str = r#"<html><body style="margin:0;background:white">
<div style="width:100px;font-size:20px;line-height:20px;display:-webkit-box;-webkit-box-orient:vertical;overflow:hidden;-webkit-line-clamp:2">HH HH HH HH HH HH HH HH HH</div>
<div style="height:10px;background:rgb(255,0,0)"></div>
</body></html>"#;

const CLIPPED_LINE_CLAMP: &str = r#"<html><body style="margin:0;background:white">
<div style="width:100px;font-size:20px;line-height:20px;display:-webkit-box;-webkit-box-orient:vertical;overflow:hidden;-webkit-line-clamp:2;background:rgb(0,0,255);background-clip:text;color:transparent">HH HH HH HH HH HH HH HH HH</div>
</body></html>"#;

fn assert_second_line_ends_in_ellipsis(buffer: &[u8]) {
    let first_line: Vec<usize> = inked_rows(buffer, 0..100)
        .into_iter()
        .filter(|&y| y < 20)
        .collect();
    let second_midline = first_line[first_line.len() / 2] + 20;
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    let right = (0..width)
        .rev()
        .find(|&x| (20..40).any(|y| pixel(buffer, x, y) != [255, 255, 255, 255]))
        .expect("the second line has ink");
    let dots: Vec<usize> = inked_rows(buffer, right - 4..right + 1)
        .into_iter()
        .filter(|y| (20..40).contains(y))
        .collect();
    assert_ne!(dots, Vec::<usize>::new());
    assert!(dots.iter().all(|&y| y > second_midline));
}

#[test]
fn line_clamp_keeps_the_first_lines() {
    let mut harness = open(App::new((), LINE_CLAMP));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 110, 45), [255, 0, 0, 255]);
    assert_ne!(pixel(&buffer, 110, 38), [255, 0, 0, 255]);
    assert_eq!(pixel(&buffer, 110, 51), [255, 255, 255, 255]);
    assert_eq!(inked_rows(&buffer, 0..120).last(), Some(&49));
    assert_second_line_ends_in_ellipsis(&buffer);
}

fn inline_box_page(line_clamp: &str) -> String {
    format!(
        r#"<html><body style="margin:0;background:white">
<div style="width:100px;font-size:20px;line-height:20px;display:-webkit-box;-webkit-box-orient:vertical;overflow:hidden;-webkit-line-clamp:{line_clamp}">HH HH HH HH HH HH HH HH HH <span style="display:inline-block;width:20px;height:10px;background:rgb(0,200,0)"></span></div>
</body></html>"#
    )
}

fn has_green(buffer: &[u8]) -> bool {
    buffer.as_chunks::<4>().0.contains(&[0, 200, 0, 255])
}

#[test]
fn line_clamp_hides_inline_boxes_on_hidden_lines() {
    let mut harness = open(App::new((), &inline_box_page("none")));
    assert!(has_green(&render(&mut harness)));
    let mut harness = open(App::new((), &inline_box_page("2")));
    assert!(!has_green(&render(&mut harness)));
}

#[test]
fn line_clamp_ellipsis_shows_through_background_clip_text() {
    let mut harness = open(App::new((), CLIPPED_LINE_CLAMP));
    let buffer = render(&mut harness);
    assert!(inked_rows(&buffer, 0..120).last().is_some_and(|&y| y < 40));
    assert_second_line_ends_in_ellipsis(&buffer);
}

fn painted_span(buffer: &[u8], width: u32) -> usize {
    let width = usize::try_from(width).expect("test width fits in usize");
    let columns: Vec<usize> = buffer
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(_, px)| **px != [255, 255, 255, 255])
        .map(|(index, _)| index % width)
        .collect();
    let first = columns.iter().min().expect("the shadow paints something");
    let last = columns.iter().max().expect("the shadow paints something");
    last - first + 1
}

#[test]
fn blurred_text_shadow_scales_with_the_display() {
    let blurred = shadow_page("10px 10px 8px rgb(255,0,0)");
    let sharp = shadow_page("10px 10px 0 rgb(255,0,0)");
    let span = |page: &str, scale: u16| {
        let mut harness = open(App::new((), page));
        let buffer = if scale == 1 {
            render(&mut harness)
        } else {
            render_at_scale(&mut harness, scale)
        };
        painted_span(&buffer, WIDTH * u32::from(scale))
    };
    let halo_1x = span(&blurred, 1) - span(&sharp, 1);
    let halo_2x = span(&blurred, 2) - span(&sharp, 2);
    assert!(halo_1x > 0);
    assert!(
        (halo_1x * 16..halo_1x * 24).contains(&(halo_2x * 10)),
        "the blur reaches {halo_1x}px past the sharp shadow at 1x and {halo_2x}px at 2x"
    );
    let at_2x = painted_pixels(&render_at_scale(&mut open(App::new((), &blurred)), 2));
    assert!(!at_2x.contains(&[255, 0, 0, 255]));
}

const CHECKER_PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAHElEQVR4nGP4f4LhPzJmYEDDdFCALoBFA60VAAAT2nihz+AndwAAAABJRU5ErkJggg==";

#[test]
fn background_image_from_a_data_url_is_painted() {
    let page = format!(
        r#"<html><body style="margin:0;background:white">
<div style="width:40px;height:40px;background:url({CHECKER_PNG});background-size:40px"></div>
</body></html>"#
    );
    let mut harness = open(App::new((), &page));
    let buffer = render(&mut harness);
    let painted = (0..40)
        .flat_map(|y| (0..40).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(&buffer, x, y) != [255, 255, 255, 255])
        .count();
    assert!(painted > 400, "only {painted} pixels painted");
}

const SQUARE_SVG: &str = "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'><rect width='4' height='4' fill='%23f00'/></svg>";
const DOT_SVG: &str = "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='2' height='2'><rect width='1' height='1' fill='%23f00'/></svg>";

#[test]
fn svg_background_images_tile_like_chrome() {
    let page = format!(
        r#"<html><body style="margin:0;background:white">
<div style="width:30px;height:20px;background:url(&quot;{SQUARE_SVG}&quot;) repeat"></div>
<div style="width:34px;height:10px;background:url(&quot;{SQUARE_SVG}&quot;) space"></div>
<div style="width:120px;height:50px;background:url(&quot;{DOT_SVG}&quot;) repeat"></div>
</body></html>"#
    );
    let mut harness = open(App::new((), &page));
    let buffer = render(&mut harness);
    let red_columns = |row: usize| {
        (0..40)
            .filter(|&column| pixel(&buffer, column, row) == [255, 0, 0, 255])
            .collect::<Vec<_>>()
    };
    let repeat_row = vec![0, 1, 2, 3, 10, 11, 12, 13, 20, 21, 22, 23];
    assert_eq!(red_columns(1), repeat_row);
    assert_eq!(red_columns(12), repeat_row);
    assert_eq!(red_columns(6), Vec::<usize>::new());
    assert_eq!(
        red_columns(21),
        vec![0, 1, 2, 3, 12, 13, 14, 15, 24, 25, 26, 27]
    );
    assert_eq!(red_columns(78), (0..40).step_by(2).collect::<Vec<_>>());
    assert_eq!(red_columns(79), Vec::<usize>::new());
}

#[test]
fn an_angled_gradient_is_centred_on_the_padding_box() {
    let html = r#"<html><body style="margin:0;background:white">
<div style="width:20px;height:10px;border:6px solid transparent;border-right-width:0;
background:linear-gradient(90deg,rgb(255,0,0) 50%,rgb(0,0,255) 50%)"></div>
</body></html>"#;
    let mut harness = open(App::new((), html));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 14, 10), [255, 0, 0, 255]);
    assert_eq!(pixel(&buffer, 17, 10), [0, 0, 255, 255]);
}

#[test]
fn floated_box_sits_at_the_right_edge() {
    let page = r#"<html><body style="margin:0;background:white">
<div style="float:right;width:20px;height:20px;background:rgb(200,0,0)"></div>
<div style="height:20px">text</div>
</body></html>"#;
    let mut harness = open(App::new((), page));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 110, 10), [200, 0, 0, 255]);
    assert_ne!(pixel(&buffer, 10, 10), [200, 0, 0, 255]);
}

#[test]
fn form_controls_size_from_their_font() {
    let page = r#"<html><body style="margin:0">
<div><input id="text"></div>
<div><input id="wide" size="40"></div>
<div><input id="number" type="number"></div>
<div><textarea id="area"></textarea></div>
<div><textarea id="big" cols="30" rows="4"></textarea></div>
<div><input id="check" type="checkbox"><input id="radio" type="radio"></div>
</body></html>"#;
    let harness = open(App::new((), page));
    let text = harness.layout_rect("#text");
    let wide = harness.layout_rect("#wide");
    let area = harness.layout_rect("#area");
    let big = harness.layout_rect("#big");

    assert!(text.width > 100.0 && text.width < 300.0, "{text:?}");
    assert_eq!(harness.layout_rect("#number").width, text.width);
    let avg = (wide.width - text.width) / 20.0;
    assert!(avg > 4.0 && avg < 10.0, "{avg}");
    assert!(big.width > area.width, "{area:?} {big:?}");
    assert!(
        (big.height - 6.0 - 2.0 * (area.height - 6.0)).abs() < 0.5,
        "{area:?} {big:?}"
    );

    for id in ["#check", "#radio"] {
        let rect = harness.layout_rect(id);
        assert_eq!((rect.width, rect.height), (13.0, 13.0), "{id}");
    }

    if cfg!(windows) {
        assert_eq!((text.width, text.height), (177.0, 21.0));
        assert_eq!(wide.width, 317.0);
        assert_eq!((area.width, area.height), (168.0, 36.0));
        assert_eq!((big.width, big.height), (241.0, 66.0));
    }
}

#[test]
fn form_controls_sit_on_the_text_baseline() {
    let page = r#"<html><body style="margin:0;font:14px Arial;white-space:nowrap">
<p id="name" style="margin:0">Name <input id="field"></p>
<p id="remember" style="margin:0"><input id="check" type="checkbox"> Remember me</p>
<p id="monthly" style="margin:0"><input id="radio" type="radio"> Monthly</p>
</body></html>"#;
    let harness = open(App::new((), page));
    let name = harness.layout_rect("#name");
    let field = harness.layout_rect("#field");
    let remember = harness.layout_rect("#remember");
    let check = harness.layout_rect("#check");
    let monthly = harness.layout_rect("#monthly");
    let radio = harness.layout_rect("#radio");

    assert_eq!(field.y, name.y, "{name:?} {field:?}");
    assert_eq!(name.height, field.height, "{name:?} {field:?}");
    assert_eq!(check.y - remember.y, 3.0, "{remember:?} {check:?}");
    assert_eq!(radio.y - monthly.y, 3.0, "{monthly:?} {radio:?}");

    if cfg!(windows) {
        assert_eq!(remember.height, 19.0, "{remember:?}");
        assert_eq!(monthly.height, 19.0, "{monthly:?}");
    }
}

#[test]
fn checkboxes_fill_with_the_accent_color() {
    let page = r#"<html><body style="margin:0;background:white">
<input type="checkbox" checked style="position:absolute;left:0;top:0;width:20px;height:20px;margin:0">
<input type="checkbox" checked style="position:absolute;left:30px;top:0;width:20px;height:20px;margin:0;accent-color:#c00">
<input type="checkbox" style="position:absolute;left:60px;top:0;width:20px;height:20px;margin:0">
</body></html>"#;
    let mut harness = open(App::new((), page));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 3, 16), [0x00, 0x75, 0xff, 255]);
    assert_eq!(pixel(&buffer, 33, 16), [0xcc, 0x00, 0x00, 255]);
    assert_eq!(pixel(&buffer, 63, 16), [255, 255, 255, 255]);
    assert_eq!(pixel(&buffer, 60, 10), [0x76, 0x76, 0x76, 255]);
}

#[test]
fn appearance_none_drops_the_native_checkbox_and_radio() {
    let page = r#"<html><body style="margin:0;background:white">
<input id="bare" type="checkbox" checked style="appearance:none;position:absolute;left:0;top:0;margin:0">
<input id="dot" type="radio" checked style="appearance:none;position:absolute;left:30px;top:0;margin:0">
<input type="checkbox" checked style="appearance:none;position:absolute;left:60px;top:0;width:20px;height:20px;margin:0;background:rgb(255,165,0)">
</body></html>"#;
    let mut harness = open(App::new((), page));
    let buffer = render(&mut harness);
    let sizes = ["#bare", "#dot"].map(|selector| {
        let doc = harness.base_mut();
        let id = doc
            .query_selector(selector)
            .expect("valid selector")
            .expect("the input exists");
        doc.get_client_bounding_rect(id)
            .map(|rect| (rect.width, rect.height))
            .expect("the input has a box")
    });
    assert_eq!(sizes, [(0.0, 0.0), (0.0, 0.0)]);
    assert_eq!(pixel(&buffer, 63, 16), [255, 165, 0, 255]);
    assert_eq!(pixel(&buffer, 70, 10), [255, 165, 0, 255]);
}
