mod scaled;
mod support;

use pageframe::App;
use scaled::render_at_scale;
use support::{WIDTH, open, pixel, render};

const WIDE: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACgAAAAUCAIAAABwJOjsAAAAKUlEQVR4nGP4z8BANqJA639KtI5aPGrxqMWjFo9aPGrxqMWjFjMMLYsBYO0dDqhir7sAAAAASUVORK5CYII=";
const SVG: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='40' height='20'%3E%3Crect width='40' height='20' fill='blue'/%3E%3C/svg%3E";
const TALL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABQAAAAoCAIAAABxU02MAAAAJUlEQVR42u3LsQEAMAgDoNj/f9Yf7CjsVNLZevkgy7Isy7J8Ow8C6AFPONt65gAAAABJRU5ErkJggg==";
const BIG_SVG: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='400' height='200'%3E%3Crect width='400' height='200' fill='blue'/%3E%3C/svg%3E";
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn page(source: &str, fit: &str) -> String {
    format!(
        "<!doctype html><style>html,body{{margin:0;background:white}}img{{display:block;width:100px;height:60px;object-fit:{fit};object-position:left top}}</style><img src=\"{source}\">"
    )
}

fn painted_size(source: &str, fit: &str) -> (usize, usize) {
    let buffer = render(&mut open(App::new((), &page(source, fit))));
    let width = (0..100)
        .take_while(|&x| pixel(&buffer, x, 5) != WHITE)
        .count();
    let height = (0..60)
        .take_while(|&y| pixel(&buffer, 5, y) != WHITE)
        .count();
    (width, height)
}

fn painted_size_at_scale_two(source: &str, fit: &str) -> (usize, usize) {
    let buffer = render_at_scale(&mut open(App::new((), &page(source, fit))), 2);
    let stride = usize::try_from(WIDTH).expect("test width fits in usize") * 2 * 4;
    let white = |x: usize, y: usize| buffer[y * stride + x * 4..y * stride + x * 4 + 4] == WHITE;
    let width = (0..200).take_while(|&x| !white(x, 10)).count();
    let height = (0..120).take_while(|&y| !white(10, y)).count();
    (width, height)
}

#[test]
fn natural_size_images_keep_their_css_size_at_1x_and_2x() {
    for fit in ["none", "scale-down"] {
        for source in [WIDE, SVG] {
            assert_eq!(painted_size(source, fit), (40, 20), "{fit} at 1x");
            assert_eq!(
                painted_size_at_scale_two(source, fit),
                (80, 40),
                "{fit} at 2x"
            );
        }
    }
}

#[test]
fn a_cover_image_fills_its_whole_content_box_at_seven_quarters_scale() {
    let html = format!(
        "<!doctype html><style>html,body{{margin:0;background:white}}img{{width:100px;height:60px;border:1px solid #000;margin:3px;object-fit:cover}}</style><img src=\"{TALL}\"><img src=\"{TALL}\"><img src=\"{TALL}\">"
    );
    let mut harness = open(App::new((), &html));
    let (width, height) = (1120, 210);
    {
        let mut doc = harness.base_mut();
        let color_scheme = doc.get_viewport().color_scheme;
        doc.set_viewport(pageframe_traits::shell::Viewport::new(
            width,
            height,
            1.75,
            color_scheme,
        ));
    }
    harness.pump();
    let buffer = {
        let mut doc = harness.base_mut();
        anyrender::render_to_buffer::<anyrender_vello_cpu::VelloCpuImageRenderer, _>(
            |scene| pageframe_paint::paint_scene(scene, &mut doc, 1.75, width, height, 0, 0),
            width,
            height,
        )
    };
    let stride = usize::try_from(width).expect("test width fits in usize");
    let at = |column: usize, row: usize| {
        let offset = (row * stride + column) * 4;
        buffer[offset..offset + 4].to_vec()
    };
    let blue = vec![0, 0, 255, 255];
    let black = vec![0, 0, 0, 255];
    for row in [7, 50, 110] {
        assert_eq!(at(380, row), black, "left border, row {row}");
        assert_eq!(at(381, row), blue, "first content column, row {row}");
        assert_eq!(at(555, row), blue, "last content column, row {row}");
        assert_eq!(at(556, row), black, "right border, row {row}");
    }
}

#[test]
fn scale_down_contains_images_larger_than_the_box_at_1x_and_2x() {
    assert_eq!(painted_size(BIG_SVG, "scale-down"), (100, 50));
    assert_eq!(painted_size_at_scale_two(BIG_SVG, "scale-down"), (200, 100));
}
