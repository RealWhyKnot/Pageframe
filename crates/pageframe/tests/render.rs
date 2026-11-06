use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_paint::paint_scene;
use blitz_test_harness::Harness;
use pageframe::{App, EventKind, Page};

const WIDTH: u32 = 120;
const HEIGHT: u32 = 80;

fn open<S: 'static>(app: App<S>) -> Harness<Page<S>> {
    let page = app
        .page(WIDTH, HEIGHT)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness
}

fn render<S: 'static>(harness: &mut Harness<Page<S>>) -> Vec<u8> {
    let mut doc = harness.base_mut();
    render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| paint_scene(scene, &mut doc, 1.0, WIDTH, HEIGHT, 0, 0),
        WIDTH,
        HEIGHT,
    )
}

fn pixel(buffer: &[u8], x: usize, y: usize) -> [u8; 4] {
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    let start = (y * width + x) * 4;
    let mut rgba = [0; 4];
    rgba.copy_from_slice(&buffer[start..start + 4]);
    rgba
}

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
