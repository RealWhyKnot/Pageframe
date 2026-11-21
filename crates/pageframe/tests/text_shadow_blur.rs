mod support;

use pageframe::App;
use support::{open, pixel, render};

#[test]
fn small_text_shadow_blurs_match_chrome() {
    let glyph = "position:absolute;top:0;font:40px/1 Arial";
    let html = format!(
        "<html><body style=\"margin:0;background:#fff\"><div style=\"{glyph};left:10px;text-shadow:0 40px 2px #000\">I</div><div style=\"{glyph};left:60px;text-shadow:0 40px 1px #000\">I</div></body></html>"
    );
    let buffer = render(&mut open(App::new((), &html)));
    let chrome: [(usize, [u8; 9]); 2] = [
        (11, [251, 224, 148, 49, 15, 40, 127, 215, 248]),
        (61, [255, 247, 162, 17, 0, 13, 127, 242, 255]),
    ];
    for (start, expected) in chrome {
        for (offset, expected) in expected.into_iter().enumerate() {
            let column = start + offset;
            let actual = pixel(&buffer, column, 60)[0];
            assert!(
                actual.abs_diff(expected) <= 8,
                "pixel ({column}, 60) is {actual}, Chrome draws {expected}"
            );
        }
    }
}
