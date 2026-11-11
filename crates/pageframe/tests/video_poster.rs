mod support;

use pageframe::App;
use support::{open, pixel, render};

const TALL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAoAAAAeCAIAAACaFxhnAAAAHUlEQVR4nGP4z8CAB+GTG5UelNKD2nGj0gykSgMA7ngq5Bho73QAAAAASUVORK5CYII=";
const WIDE: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACgAAAAUCAIAAABwJOjsAAAAKUlEQVR4nGP4z8BANqJA639KtI5aPGrxqMWjFo9aPGrxqMWjFjMMLYsBYO0dDqhir7sAAAAASUVORK5CYII=";
const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const GREEN: [u8; 4] = [0, 204, 0, 255];

fn page(attributes: &str) -> String {
    format!(
        "<!doctype html><style>html,body{{margin:0;background:white}}video{{background:#0c0}}</style><video id='video' {attributes}></video>"
    )
}

fn size(attributes: &str) -> (f32, f32) {
    let harness = open(App::new((), &page(attributes)));
    let doc = harness.base();
    let id = doc
        .query_selector_all("#video")
        .expect("the selector parses")[0];
    let size = doc
        .get_node(id)
        .expect("the node exists")
        .final_layout()
        .size;
    (size.width, size.height)
}

#[test]
fn a_video_without_a_poster_keeps_the_default_size() {
    assert_eq!(size(""), (300.0, 150.0));
    assert_eq!(size("width='100'"), (100.0, 150.0));
    assert_eq!(size("poster='' width='50'"), (50.0, 150.0));
}

#[test]
fn a_poster_gives_the_video_its_size() {
    assert_eq!(size(&format!("poster='{WIDE}'")), (40.0, 20.0));
    assert_eq!(
        size(&format!("poster='{WIDE}' style='width:100px'")),
        (100.0, 50.0)
    );
    assert_eq!(
        size(&format!(
            "poster='{TALL}' style='display:block;height:60px'"
        )),
        (20.0, 60.0)
    );
    assert_eq!(
        size(&format!("poster='{TALL}' style='max-width:5px'")),
        (5.0, 15.0)
    );
}

#[test]
fn a_broken_poster_keeps_the_default_size() {
    assert_eq!(size("poster='data:,bad'"), (300.0, 150.0));
    assert_eq!(
        size("poster='data:,bad' width='120' height='60'"),
        (120.0, 60.0)
    );
}

#[test]
fn a_poster_is_fitted_inside_the_video_box() {
    let mut harness = open(App::new(
        (),
        &page(&format!("poster='{TALL}' width='120' height='60'")),
    ));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 10, 30), GREEN);
    assert_eq!(pixel(&buffer, 110, 30), GREEN);
    assert_eq!(pixel(&buffer, 51, 5), RED);
    assert_eq!(pixel(&buffer, 68, 55), BLUE);
}

#[test]
fn object_fit_and_position_apply_to_the_poster() {
    for (style, points) in [
        ("object-fit:cover", [(2, 5, RED), (117, 55, BLUE)]),
        ("object-fit:fill", [(2, 2, RED), (117, 57, BLUE)]),
        ("object-position:left top", [(2, 5, RED), (60, 30, GREEN)]),
    ] {
        let mut harness = open(App::new(
            (),
            &page(&format!(
                "poster='{TALL}' style='width:120px;height:60px;{style}'"
            )),
        ));
        let buffer = render(&mut harness);
        for (x, y, expected) in points {
            assert_eq!(pixel(&buffer, x, y), expected, "{style} at {x},{y}");
        }
    }
}

#[test]
fn changing_the_poster_resizes_the_video() {
    let mut harness = open(App::new((), &page(&format!("poster='{WIDE}'"))));
    let mut sizes = Vec::new();
    for step in 0..3 {
        match step {
            1 => {
                harness.doc.ui().select("#video").set_attr("poster", TALL);
            }
            2 => {
                harness.doc.ui().select("#video").remove_attr("poster");
            }
            _ => {}
        }
        harness.pump();
        let id = harness
            .base()
            .query_selector_all("#video")
            .expect("the selector parses")[0];
        let size = harness
            .base()
            .get_node(id)
            .expect("the node exists")
            .final_layout()
            .size;
        sizes.push((size.width, size.height));
    }
    assert_eq!(sizes, [(40.0, 20.0), (10.0, 30.0), (300.0, 150.0)]);
}
