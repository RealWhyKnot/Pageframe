mod scaled;
mod support;

use pageframe::App;
use scaled::render_at_scale;
use support::{WIDTH, open, pixel, render};

const NINE: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAB4AAAAeCAIAAAC0Ujn1AAAAgElEQVR4nGO8o6HBgBv4VOCRZGDo2IJHkomBZoBp1GhUMBogwyJAGJ+dkMMjLWn+EKSIkZGBgeH///9o7JOaJylyNSMOcyFs8o1mJNdcolz9H4e5EDb5Rv8n19zRsB4uYc0od+IEHuk7u3zwyCbccBt2xRPTqNGoYDRA0AANAwQArdxVL4XYK2kAAAAASUVORK5CYII=";
const GREY: [u8; 4] = [221, 221, 221, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const REGIONS: [((usize, usize), [u8; 4]); 8] = [
    ((5, 5), [220, 40, 40, 255]),
    ((40, 5), [40, 160, 40, 255]),
    ((75, 5), [40, 40, 220, 255]),
    ((5, 25), [230, 200, 30, 255]),
    ((75, 25), [200, 40, 200, 255]),
    ((5, 45), [30, 200, 200, 255]),
    ((40, 45), [250, 130, 20, 255]),
    ((75, 45), [90, 90, 90, 255]),
];

fn page(width: u32, image: &str) -> String {
    format!(
        "<!doctype html><style>html,body{{margin:0;background:white}}div{{width:{width}px;height:30px;background:#ddd;border:10px solid black;border-image:{image}}}</style><div></div>"
    )
}

fn close(actual: [u8; 4], expected: [u8; 4]) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(&a, e)| a.abs_diff(e) <= 2)
}

#[test]
fn nine_slices_land_in_their_border_areas_at_1x_and_2x() {
    let html = page(60, &format!("url({NINE}) 10"));
    let buffer = render(&mut open(App::new((), &html)));
    for ((x, y), color) in REGIONS {
        let actual = pixel(&buffer, x, y);
        assert!(close(actual, color), "({x},{y}) {actual:?} at 1x");
    }
    assert_eq!(pixel(&buffer, 40, 25), GREY);

    let buffer = render_at_scale(&mut open(App::new((), &html)), 2);
    let stride = usize::try_from(WIDTH).expect("test width fits in usize") * 2 * 4;
    for ((x, y), color) in REGIONS {
        let start = y * 2 * stride + x * 2 * 4;
        let mut actual = [0; 4];
        actual.copy_from_slice(&buffer[start..start + 4]);
        assert!(close(actual, color), "({x},{y}) {actual:?} at 2x");
    }
}

#[test]
fn round_tiles_meet_without_blending_across_the_seam() {
    let checker = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAHElEQVR4nGP4f4LhPzJmYEDDdFCALoBFA60VAAAT2nihz+AndwAAAABJRU5ErkJggg==";
    let html = page(100, &format!("url({checker}) 2 round"));
    let buffer = render(&mut open(App::new((), &html)));
    let yellow = [255, 200, 0, 255];
    assert!(close(pixel(&buffer, 15, 5), yellow));
    assert!(close(pixel(&buffer, 20, 5), [102, 80, 0, 255]));
    assert!(close(pixel(&buffer, 29, 5), BLACK));
    assert!(close(pixel(&buffer, 30, 5), yellow));
}

#[test]
fn fill_paints_the_middle_with_a_gradient_source() {
    let green = [0, 170, 0, 255];
    let html = page(60, "linear-gradient(#0a0,#0a0) 1 fill");
    let buffer = render(&mut open(App::new((), &html)));
    for (x, y) in [(5, 5), (40, 5), (40, 25), (75, 45)] {
        assert_eq!(pixel(&buffer, x, y), green, "({x},{y})");
    }
}

#[test]
fn space_leaves_an_edge_blank_when_no_tile_fits() {
    let html = page(5, &format!("url({NINE}) 10 space"));
    let buffer = render(&mut open(App::new((), &html)));
    assert!(close(pixel(&buffer, 5, 5), REGIONS[0].1));
    assert_eq!(pixel(&buffer, 12, 5), GREY);
    assert_eq!(pixel(&buffer, 12, 45), GREY);
}

#[test]
fn a_source_that_fails_to_load_keeps_the_plain_border() {
    let html = page(60, "url(data:image/png;base64,AAAA) 10");
    let buffer = render(&mut open(App::new((), &html)));
    assert_eq!(pixel(&buffer, 5, 5), BLACK);
    assert_eq!(pixel(&buffer, 40, 45), BLACK);
}

#[test]
fn long_edges_with_tiny_tiles_reach_the_far_corner() {
    let stripe = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAQAAAADCAYAAAC09K7GAAAAH0lEQVR42l3EsQ0AMAgDMKf//xwGlgoPDlrEHtTnOQajVQQDRUvUBQAAAABJRU5ErkJggg==";
    let html = format!(
        "<!doctype html><style>html,body{{margin:0;background:white}}div{{position:absolute;left:-8950px;top:0;width:8998px;height:10px;border:1px solid black;border-image:url({stripe}) 1 repeat}}</style><div></div>"
    );
    let buffer = render(&mut open(App::new((), &html)));
    let (red, blue) = ([255, 0, 0, 255], [0, 0, 255, 255]);
    assert!(close(pixel(&buffer, 10, 0), blue));
    assert!(close(pixel(&buffer, 11, 0), red));
    assert!(close(pixel(&buffer, 40, 0), blue));
    assert!(close(pixel(&buffer, 41, 0), red));
}

#[test]
fn outset_width_and_percentage_slices_match_chrome() {
    let html = format!(
        "<!doctype html><style>html,body{{margin:0;background:white}}div{{position:absolute;width:20px;height:10px;background:#ddd;border:10px solid black}}</style>\
<div style=\"left:10px;top:10px;border-image:url({NINE}) 10;border-image-outset:5px\"></div>\
<div style=\"left:65px;top:10px;border-image:url({NINE}) 10;border-image-width:15px\"></div>\
<div style=\"left:10px;top:45px;border-image:url({NINE}) 50%\"></div>"
    );
    let buffer = render(&mut open(App::new((), &html)));
    let (red, green, grey) = ([220, 40, 40, 255], [40, 160, 40, 255], [90, 90, 90, 255]);
    let white = [255, 255, 255, 255];
    let orange = [250, 130, 20, 255];
    for ((x, y), chrome) in [
        ((4, 4), white),
        ((6, 6), red),
        ((30, 7), green),
        ((52, 42), grey),
        ((30, 25), GREY),
        ((79, 11), red),
        ((81, 11), green),
        ((79, 24), red),
        ((12, 47), red),
        ((18, 47), green),
        ((22, 47), GREY),
        ((40, 70), orange),
        ((38, 73), GREY),
    ] {
        let actual = pixel(&buffer, x, y);
        assert!(
            close(actual, chrome),
            "({x},{y}) {actual:?}, Chrome draws {chrome:?}"
        );
    }
}
