mod support;

use pageframe::App;
use support::{open, pixel, render};

const TILE: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAYAAACp8Z5+AAAAGElEQVR42mM4w8Dw/wwDw38GKGBiIBkAAOPGA5lI6dvmAAAAAElFTkSuQmCC";

fn page(html: &str, body: &str, content: &str) -> String {
    format!("<html style=\"{html}\"><body style=\"{body}\">{content}</body></html>")
}

fn over_white(rgba: [u8; 4]) -> [u8; 3] {
    let cover = 255 - rgba[3];
    [
        rgba[0].saturating_add(cover),
        rgba[1].saturating_add(cover),
        rgba[2].saturating_add(cover),
    ]
}

fn check(html: &str, samples: &[(usize, usize, [u8; 3])]) {
    let buffer = render(&mut open(App::new((), html)));
    for &(column, row, chrome) in samples {
        let actual = over_white(pixel(&buffer, column, row));
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

const BLOCK: &str = "<div style=\"height:30px\"></div>";

#[test]
fn root_gradients_tile_across_the_canvas_like_chrome() {
    let html = page(
        "background:repeating-linear-gradient(rgb(204,0,0) 0 10px,rgb(0,102,204) 10px 20px)",
        "margin:0",
        BLOCK,
    );
    check(
        &html,
        &[
            (5, 5, [204, 0, 0]),
            (5, 15, [0, 102, 204]),
            (5, 45, [0, 102, 204]),
            (5, 55, [204, 0, 0]),
            (5, 65, [204, 0, 0]),
        ],
    );
}

#[test]
fn body_backgrounds_move_to_the_canvas_like_chrome() {
    let html = page(
        "",
        "margin:10px;background:linear-gradient(rgb(204,0,0),rgb(0,102,204))",
        BLOCK,
    );
    check(
        &html,
        &[
            (5, 5, [181, 11, 22]),
            (60, 25, [100, 52, 104]),
            (5, 60, [162, 22, 43]),
            (5, 75, [100, 52, 104]),
        ],
    );
}

#[test]
fn translucent_root_colours_paint_once_like_chrome() {
    let html = page("background:rgba(0,0,255,.5)", "margin:0", BLOCK);
    check(&html, &[(5, 5, [127, 127, 255]), (5, 60, [127, 127, 255])]);
}

#[test]
fn body_colours_stay_put_under_a_root_image_like_chrome() {
    let html = page(
        "background-image:linear-gradient(rgb(204,0,0),rgb(0,102,204));\
background-repeat:no-repeat;background-size:20px 10px",
        "margin:0;background:rgb(255,255,0)",
        BLOCK,
    );
    check(&html, &[(50, 5, [255, 255, 0]), (5, 60, [255, 255, 255])]);
}

#[test]
fn repeated_images_reach_under_the_far_borders_like_chrome() {
    let content = format!(
        "<div style=\"margin:10px;width:40px;height:30px;border:8px dashed rgba(0,0,0,.2);\
background:url({TILE}) 1px 1px\"></div>"
    );
    let html = page("background:white", "margin:0", &content);
    check(
        &html,
        &[
            (27, 51, [204, 0, 0]),
            (47, 52, [204, 0, 0]),
            (60, 27, [204, 0, 0]),
            (63, 31, [204, 0, 0]),
        ],
    );
}
