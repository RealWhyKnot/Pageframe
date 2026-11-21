mod support;

use pageframe::App;
use support::{open, pixel, render};

const CHECKER: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAHElEQVR4nGP4f4LhPzJmYEDDdFCALoBFA60VAAAT2nihz+AndwAAAABJRU5ErkJggg==";

#[test]
fn upscaled_backgrounds_blend_like_chrome() {
    let tile = format!("background:url({CHECKER}) 0 0/20px");
    let html = format!(
        "<html><body style=\"margin:0;background:#fff\"><div style=\"width:40px;height:20px;{tile}\"></div><div style=\"width:40px;height:40px;margin-top:10px;{tile}\"></div></body></html>"
    );
    let buffer = render(&mut open(App::new((), &html)));
    let chrome: [(usize, usize, [u8; 3]); 10] = [
        (8, 5, [255, 200, 0]),
        (9, 5, [178, 140, 0]),
        (10, 5, [76, 60, 0]),
        (19, 5, [77, 60, 0]),
        (8, 35, [255, 200, 0]),
        (9, 35, [191, 150, 0]),
        (10, 35, [79, 62, 0]),
        (5, 38, [255, 200, 0]),
        (5, 39, [191, 150, 0]),
        (5, 40, [79, 62, 0]),
    ];
    for (column, row, expected) in chrome {
        let actual = pixel(&buffer, column, row);
        let off = (0..3)
            .map(|channel| actual[channel].abs_diff(expected[channel]))
            .max()
            .unwrap_or(0);
        assert!(
            off <= 1,
            "pixel ({column}, {row}) is {actual:?}, Chrome draws {expected:?}"
        );
    }
}
