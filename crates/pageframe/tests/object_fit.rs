mod scaled;
mod support;

use pageframe::App;
use scaled::render_at_scale;
use support::{WIDTH, open, pixel, render};

const WIDE: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACgAAAAUCAIAAABwJOjsAAAAKUlEQVR4nGP4z8BANqJA639KtI5aPGrxqMWjFo9aPGrxqMWjFjMMLYsBYO0dDqhir7sAAAAASUVORK5CYII=";
const SVG: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='40' height='20'%3E%3Crect width='40' height='20' fill='blue'/%3E%3C/svg%3E";
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn page(source: &str, fit: &str) -> String {
    format!(
        "<!doctype html><style>html,body{{margin:0;background:white}}img{{display:block;width:100px;height:60px;object-fit:{fit};object-position:left top}}</style><img src=\"{source}\">"
    )
}

fn painted_width(source: &str, fit: &str) -> usize {
    let buffer = render(&mut open(App::new((), &page(source, fit))));
    (0..100)
        .take_while(|&x| pixel(&buffer, x, 5) != WHITE)
        .count()
}

fn painted_width_at_scale_two(source: &str, fit: &str) -> usize {
    let buffer = render_at_scale(&mut open(App::new((), &page(source, fit))), 2);
    let row = 10 * usize::try_from(WIDTH).expect("test width fits in usize") * 2 * 4;
    (0..200)
        .take_while(|x| buffer[row + x * 4..row + x * 4 + 4] != WHITE)
        .count()
}

#[test]
fn natural_size_images_keep_their_css_size_at_every_scale() {
    for fit in ["none", "scale-down"] {
        for source in [WIDE, SVG] {
            assert_eq!(painted_width(source, fit), 40, "{fit} at 1x");
            assert_eq!(painted_width_at_scale_two(source, fit), 80, "{fit} at 2x");
        }
    }
}
