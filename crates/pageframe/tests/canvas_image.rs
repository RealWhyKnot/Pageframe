mod support;

use pageframe::{App, Canvas, Image};
use support::{open, pixel, render};

const WHITE: [u8; 4] = [255, 255, 255, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 244, 34, 127, 138, 0, 0, 0, 14, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 0, 66, 255,
    1, 15, 249, 3, 253, 133, 17, 153, 118, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

const TURNED: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 244, 34, 127, 138, 0, 0, 0, 26, 101, 88, 73, 102, 77, 77, 0, 42, 0, 0, 0, 8, 0, 1, 1, 18,
    0, 3, 0, 0, 0, 1, 0, 6, 0, 0, 0, 0, 0, 0, 214, 103, 75, 105, 0, 0, 0, 14, 73, 68, 65, 84, 120,
    156, 99, 248, 207, 192, 0, 66, 255, 1, 15, 249, 3, 253, 133, 17, 153, 118, 0, 0, 0, 0, 73, 69,
    78, 68, 174, 66, 96, 130,
];

const PAGE: &str = r#"<html><head><style>
body{margin:0;background:white}
canvas{display:block}
</style></head><body>
<canvas id="c" width="100" height="60"></canvas>
</body></html>"#;

fn drawn(paint: impl Fn(&mut Canvas)) -> Vec<u8> {
    let mut harness = open(App::new((), PAGE));
    harness.doc.ui().select("#c").draw(paint);
    harness.pump();
    render(&mut harness)
}

fn halves() -> Image {
    let pixels = (0..400_u32)
        .flat_map(|index| if index % 20 < 10 { RED } else { BLUE })
        .collect();
    Image::from_rgba(20, 20, pixels).expect("a 20 by 20 image")
}

#[test]
fn an_image_is_drawn_at_its_own_size() {
    let buffer = drawn(|canvas| {
        canvas.draw_image(&halves(), 10.0, 5.0);
    });
    assert_eq!(pixel(&buffer, 5, 10), WHITE);
    assert_eq!(pixel(&buffer, 12, 10), RED);
    assert_eq!(pixel(&buffer, 27, 10), BLUE);
    assert_eq!(pixel(&buffer, 32, 10), WHITE);
    assert_eq!(pixel(&buffer, 15, 3), WHITE);
    assert_eq!(pixel(&buffer, 15, 23), RED);
    assert_eq!(pixel(&buffer, 15, 27), WHITE);
}

#[test]
fn an_image_stretches_to_the_size_asked_for() {
    let buffer = drawn(|canvas| {
        canvas.draw_image_scaled(&halves(), 20.0, 10.0, 60.0, 40.0);
    });
    assert_eq!(pixel(&buffer, 15, 30), WHITE);
    assert_eq!(pixel(&buffer, 25, 30), RED);
    assert_eq!(pixel(&buffer, 45, 30), RED);
    assert_eq!(pixel(&buffer, 55, 30), BLUE);
    assert_eq!(pixel(&buffer, 75, 30), BLUE);
    assert_eq!(pixel(&buffer, 85, 30), WHITE);
    assert_eq!(pixel(&buffer, 50, 55), WHITE);
}

#[test]
fn part_of_an_image_can_fill_the_canvas() {
    let buffer = drawn(|canvas| {
        let source = (10.0, 0.0, 10.0, 20.0);
        canvas.draw_image_part(&halves(), source, (0.0, 0.0, 100.0, 60.0));
    });
    assert_eq!(pixel(&buffer, 10, 30), BLUE);
    assert_eq!(pixel(&buffer, 97, 57), BLUE);
}

#[test]
fn a_source_past_the_image_edge_leaves_that_part_empty() {
    let buffer = drawn(|canvas| {
        let source = (-10.0, 0.0, 30.0, 20.0);
        canvas.draw_image_part(&halves(), source, (0.0, 0.0, 90.0, 60.0));
    });
    assert_eq!(pixel(&buffer, 15, 30), WHITE);
    assert_eq!(pixel(&buffer, 45, 30), RED);
    assert_eq!(pixel(&buffer, 75, 30), BLUE);
    assert_eq!(pixel(&buffer, 95, 30), WHITE);
}

#[test]
fn negative_sizes_do_not_flip_the_image() {
    let buffer = drawn(|canvas| {
        canvas.draw_image_scaled(&halves(), 60.0, 40.0, -40.0, -20.0);
    });
    assert_eq!(pixel(&buffer, 25, 30), RED);
    assert_eq!(pixel(&buffer, 55, 30), BLUE);
    assert_eq!(pixel(&buffer, 65, 30), WHITE);
    assert_eq!(pixel(&buffer, 40, 45), WHITE);
}

#[test]
fn an_image_follows_alpha_and_the_transform() {
    let buffer = drawn(|canvas| {
        canvas
            .translate(50.0, 20.0)
            .scale(2.0, 1.0)
            .set_alpha(0.5)
            .draw_image(&halves(), 0.0, 0.0);
    });
    assert_eq!(pixel(&buffer, 45, 30), WHITE);
    let faded = pixel(&buffer, 60, 30);
    assert!(faded[0] == 255 && faded[1].abs_diff(128) <= 2 && faded[2].abs_diff(128) <= 2);
    let faded = pixel(&buffer, 80, 30);
    assert!(faded[2] == 255 && faded[0].abs_diff(128) <= 2 && faded[1].abs_diff(128) <= 2);
    assert_eq!(pixel(&buffer, 60, 45), WHITE);
}

#[test]
fn a_png_decodes_into_an_image() {
    let image = Image::decode(PNG).expect("a two pixel png");
    assert_eq!((image.width(), image.height()), (2, 1));
    let buffer = drawn(|canvas| {
        canvas.draw_image_scaled(&image, 0.0, 0.0, 100.0, 60.0);
    });
    assert_eq!(pixel(&buffer, 3, 30), RED);
    assert_eq!(pixel(&buffer, 96, 30), BLUE);
}

#[test]
fn a_turned_photo_is_drawn_upright() {
    let image = Image::decode(TURNED).expect("a png with a rotation tag");
    assert_eq!((image.width(), image.height()), (1, 2));
    let buffer = drawn(|canvas| {
        canvas.draw_image_scaled(&image, 0.0, 0.0, 40.0, 60.0);
    });
    assert_eq!(pixel(&buffer, 20, 5), RED);
    assert_eq!(pixel(&buffer, 20, 55), BLUE);
}

#[test]
fn a_transform_past_what_numbers_hold_draws_nothing() {
    let buffer = drawn(|canvas| {
        canvas
            .scale(1e200, 1e200)
            .draw_image_scaled(&halves(), 0.0, 0.0, 1e200, 1e200);
    });
    assert_eq!(pixel(&buffer, 10, 10), WHITE);
    let buffer = drawn(|canvas| {
        canvas
            .scale(1e150, 1e150)
            .draw_image_scaled(&halves(), 0.0, 0.0, 2e158, 2e158);
    });
    assert_eq!(pixel(&buffer, 10, 10), WHITE);
}

#[test]
fn bad_images_are_refused() {
    assert!(Image::from_rgba(2, 2, vec![0; 15]).is_none());
    assert!(Image::from_rgba(0, 2, Vec::new()).is_none());
    assert!(Image::from_rgba(2, 0, Vec::new()).is_none());
    assert!(Image::from_rgba(20_000, 1, vec![0; 80_000]).is_none());
    assert!(Image::from_rgba(u32::MAX, u32::MAX, Vec::new()).is_none());
    assert!(Image::decode(b"not an image").is_none());
    assert!(Image::decode(&PNG[..30]).is_none());
    assert!(Image::decode(&[]).is_none());
}

#[test]
fn bad_or_empty_rectangles_draw_nothing() {
    let buffer = drawn(|canvas| {
        let image = halves();
        let whole = (0.0, 0.0, 100.0, 60.0);
        canvas
            .draw_image(&image, f64::NAN, 0.0)
            .draw_image_scaled(&image, 0.0, 0.0, f64::INFINITY, 60.0)
            .draw_image_scaled(&image, 0.0, 0.0, 0.0, 60.0)
            .draw_image_part(&image, (5.0, 5.0, 0.0, 10.0), whole)
            .draw_image_part(&image, (30.0, 0.0, 10.0, 20.0), whole)
            .draw_image_part(&image, (0.0, 0.0, f64::NAN, 20.0), whole);
    });
    assert_eq!(pixel(&buffer, 50, 30), WHITE);
    assert_eq!(pixel(&buffer, 5, 5), WHITE);
}

#[test]
fn oversized_images_and_targets_still_paint() {
    let wide = Image::from_rgba(16_384, 1, RED.repeat(16_384)).expect("a wide image");
    let buffer = drawn(|canvas| {
        canvas
            .draw_image_scaled(&wide, 0.0, 0.0, 50.0, 60.0)
            .draw_image_scaled(&halves(), 50.0, 0.0, 1e12, 1e12);
    });
    assert_eq!(pixel(&buffer, 25, 30), RED);
    assert_eq!(pixel(&buffer, 75, 30), RED);
}
