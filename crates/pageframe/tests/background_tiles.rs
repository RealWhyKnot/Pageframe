mod support;

use pageframe::App;
use support::{open, pixel, render};

const RED_BLUE: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAFklEQVR42mM4w8DAkHaGiYGBgYGBAQAUbgIBeglB6QAAAABJRU5ErkJggg==";
const GREEN: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAIAAAAmkwkpAAAAFElEQVR42mNkmGnMAANMDEgANwcAKKkA1Mae4jgAAAAASUVORK5CYII=";

fn check(body: &str, samples: &[(usize, usize, [u8; 3])]) {
    let html = format!("<html><body style=\"margin:0;background:#fff\">{body}</body></html>");
    let buffer = render(&mut open(App::new((), &html)));
    for &(column, row, chrome) in samples {
        let actual = pixel(&buffer, column, row);
        let off = (0..3)
            .map(|channel| actual[channel].abs_diff(chrome[channel]))
            .max()
            .unwrap_or(0);
        assert!(
            off <= 3,
            "pixel ({column}, {row}) is {actual:?}, Chrome draws {chrome:?}"
        );
    }
}

#[test]
fn a_tile_covering_the_box_clamps_its_edges_like_chrome() {
    let body = format!(
        "<div style=\"width:40px;height:20px;border:1px solid #000;background:url({RED_BLUE}) 0 0/100% 100%\"></div>"
    );
    check(
        &body,
        &[
            (1, 5, [204, 0, 0]),
            (2, 5, [204, 0, 0]),
            (39, 5, [0, 102, 204]),
            (40, 5, [0, 102, 204]),
        ],
    );
}

#[test]
fn unrepeated_images_snap_to_whole_pixels_like_chrome() {
    let body = format!(
        "<div style=\"width:40px;height:20px;background:url({GREEN}) no-repeat 0 1.5px/8px 7.5px\"></div>"
    );
    check(
        &body,
        &[
            (1, 1, [255, 255, 255]),
            (1, 2, [0, 153, 51]),
            (7, 9, [0, 153, 51]),
            (1, 10, [255, 255, 255]),
            (8, 5, [255, 255, 255]),
        ],
    );
}
