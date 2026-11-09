mod support;

use blitz_test_harness::Harness;
use pageframe::{App, Page};
use support::{open, pixel, render};

const RED: [u8; 3] = [255, 0, 0];
const GREEN: [u8; 3] = [0, 128, 0];
const WHITE: [u8; 3] = [255, 255, 255];

const PAGE: &str = r#"<html><head><style>
body{margin:0;background:white}
.row{position:absolute;left:0;height:10px}
.c{height:10px;background:rgb(255,0,0)}
#plain{top:0;width:110px;container-type:inline-size}
@container (min-width:100px){#plain .c{background:rgb(0,128,0)}}
#outer{top:12px;width:110px;container-type:inline-size}
#named{top:0;width:50px;container:card/inline-size}
@container card (max-width:60px){#named .c{background:rgb(0,128,0)}}
#padded{top:24px;width:100px;padding:0 25px;box-sizing:border-box;container-type:inline-size}
@container (max-width:60px){#padded .c{background:rgb(0,128,0)}}
#units{top:36px;width:100px;container-type:inline-size}
#units .c{width:50cqw;background:rgb(0,128,0)}
#sized{position:absolute;top:50px;left:0;width:40px;contain:size;background:rgb(255,0,0)}
#after{position:absolute;top:50px;left:60px;width:40px}
#after > div{height:10px;background:rgb(0,128,0)}
</style></head><body>
<div class="row" id="plain"><div class="c"></div></div>
<div class="row" id="outer"><div class="row" id="named"><div class="c"></div></div></div>
<div class="row" id="padded"><div class="c"></div></div>
<div class="row" id="units"><div class="c"></div></div>
<div id="sized"><div style="height:30px"></div></div>
<div id="after"><div></div></div>
</body></html>"#;

fn frame(harness: &mut Harness<Page<()>>) -> Vec<u8> {
    harness.pump();
    render(harness)
}

fn check(buffer: &[u8], x: usize, y: usize, expected: [u8; 3]) {
    let actual = pixel(buffer, x, y);
    assert_eq!(
        [actual[0], actual[1], actual[2]],
        expected,
        "pixel ({x}, {y})"
    );
}

#[test]
fn container_query_matches_container_width() {
    let buffer = frame(&mut open(App::new((), PAGE)));
    check(&buffer, 50, 5, GREEN);
}

#[test]
fn named_container_query_skips_unnamed_ancestors() {
    let buffer = frame(&mut open(App::new((), PAGE)));
    check(&buffer, 25, 17, GREEN);
}

#[test]
fn container_query_uses_content_box() {
    let buffer = frame(&mut open(App::new((), PAGE)));
    check(&buffer, 50, 29, GREEN);
    check(&buffer, 10, 29, WHITE);
}

#[test]
fn cq_units_resolve_against_container() {
    let buffer = frame(&mut open(App::new((), PAGE)));
    check(&buffer, 48, 41, GREEN);
    check(&buffer, 52, 41, WHITE);
}

#[test]
fn size_containment_ignores_children() {
    let buffer = frame(&mut open(App::new((), PAGE)));
    check(&buffer, 20, 55, WHITE);
    check(&buffer, 80, 55, GREEN);
}

#[test]
fn container_query_updates_when_resized() {
    let mut harness = open(App::new((), PAGE));
    check(&frame(&mut harness), 50, 5, GREEN);
    harness.doc.ui().select("#plain").set_style("width", "80px");
    check(&frame(&mut harness), 50, 5, RED);
}
