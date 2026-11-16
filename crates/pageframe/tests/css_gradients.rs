mod scaled;
mod support;

use pageframe::App;
use scaled::render_at_scale;
use support::{WIDTH, open, pixel, render};

const PAGE: &str = r#"<html><body style="margin:0;background:white">
<div style="position:absolute;left:0;top:0;width:60px;height:60px;background:repeating-radial-gradient(circle at 30px 30px,rgb(200,0,0) 0 5px,rgb(255,255,255) 5px 10px)"></div>
<div style="position:absolute;left:60px;top:0;width:60px;height:60px;background:conic-gradient(at 20px 20px,rgb(255,0,0) 0 25%,rgb(0,0,255) 25% 50%,rgb(0,160,0) 50% 75%,rgb(0,0,0) 75%)"></div>
</body></html>"#;

const RED: [u8; 4] = [200, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const CONIC: [((usize, usize), [u8; 4]); 4] = [
    ((75, 15), [0, 0, 0, 255]),
    ((85, 15), [255, 0, 0, 255]),
    ((75, 25), [0, 160, 0, 255]),
    ((85, 25), [0, 0, 255, 255]),
];

#[test]
fn radial_and_conic_lengths_are_css_pixels_at_1x() {
    let buffer = render(&mut open(App::new((), PAGE)));
    for ((column, row), chrome) in [
        ((32, 30), RED),
        ((37, 30), WHITE),
        ((42, 30), RED),
        ((47, 30), WHITE),
    ]
    .into_iter()
    .chain(CONIC)
    {
        assert_eq!(
            pixel(&buffer, column, row),
            chrome,
            "({column},{row}) at 1x"
        );
    }
}

#[test]
fn radial_and_conic_lengths_are_css_pixels_at_2x() {
    let buffer = render_at_scale(&mut open(App::new((), PAGE)), 2);
    let width = usize::try_from(WIDTH * 2).expect("test width fits in usize");
    let at = |column: usize, row: usize| {
        let start = (row * width + column) * 4;
        let mut rgba = [0; 4];
        rgba.copy_from_slice(&buffer[start..start + 4]);
        rgba
    };
    for ((column, row), chrome) in [
        ((64, 60), RED),
        ((74, 60), WHITE),
        ((84, 60), RED),
        ((94, 60), WHITE),
        ((104, 60), RED),
        ((60, 84), RED),
    ] {
        assert_eq!(at(column, row), chrome, "({column},{row}) at 2x");
    }
    for ((column, row), chrome) in CONIC {
        assert_eq!(at(column * 2, row * 2), chrome, "({column},{row}) at 2x");
    }
}
