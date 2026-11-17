mod support;

use pageframe::App;
use support::{open, pixel, render};

fn ink_span(buffer: &[u8], row: usize) -> Option<(usize, usize)> {
    let columns: Vec<usize> = (0..120)
        .filter(|&across| {
            (row * 20..(row + 1) * 20).any(|down| {
                let [red, green, blue, _] = pixel(buffer, across, down);
                red.max(green).max(blue) < 128
            })
        })
        .collect();
    Some((*columns.first()?, *columns.last()? + 1))
}

#[test]
fn inline_svg_text_uses_the_css_font_like_chrome() {
    let rows = [
        r#"<svg width="120" height="20"><text x="2" y="15" font-size="16">iiiiiii</text></svg>"#,
        r#"<svg width="120" height="20" font-family="Courier New"><text x="2" y="15" font-size="16">iiiiiii</text></svg>"#,
        r#"<svg width="120" height="20"><text x="2" y="15" font-size="16" font-family="monospace">iiiiiii</text></svg>"#,
        r#"<svg width="120" height="20" style="font-size:16px"><text x="2" y="15" style="font-weight:bold">iiiiiii</text></svg>"#,
    ];
    let html = format!(
        "<!doctype html><style>html,body{{margin:0;background:white;font:12px Arial}}svg{{display:block}}</style>{}",
        rows.concat()
    );
    let mut harness = open(App::new((), &html));
    let buffer = render(&mut harness);
    for (row, (chrome_left, chrome_right)) in
        [(3, 26), (4, 68), (3, 62), (3, 32)].into_iter().enumerate()
    {
        let (left, right) = ink_span(&buffer, row).expect("the text is drawn");
        assert!(
            left.abs_diff(chrome_left) <= 1 && right.abs_diff(chrome_right) <= 1,
            "{}: ink {left}..{right}, Chrome {chrome_left}..{chrome_right}",
            rows[row]
        );
    }
}
