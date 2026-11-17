mod support;

use pageframe::App;
use support::{open, pixel, render};

fn render_rows(rows: &[&str]) -> Vec<u8> {
    let html = format!(
        "<!doctype html><style>html,body{{margin:0;background:white;font:12px Arial}}svg{{display:block}}</style>{}",
        rows.concat()
    );
    let mut harness = open(App::new((), &html));
    render(&mut harness)
}

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

fn ink_centre(buffer: &[u8], down: usize) -> f32 {
    let (weighted, total) = (0..30).fold((0.0, 0.0), |(weighted, total), across| {
        let [red, _, _, _] = pixel(buffer, across, down);
        let ink = f32::from(255 - red);
        let column = f32::from(u16::try_from(across).expect("test columns fit in u16"));
        (weighted + ink * column, total + ink)
    });
    weighted / total
}

fn assert_ink_spans(rows: &[&str], buffer: &[u8], chrome: &[(usize, usize)]) {
    for (row, (chrome_left, chrome_right)) in chrome.iter().copied().enumerate() {
        let (left, right) = ink_span(buffer, row).expect("the text is drawn");
        assert!(
            left.abs_diff(chrome_left) <= 1 && right.abs_diff(chrome_right) <= 1,
            "{}: ink {left}..{right}, Chrome {chrome_left}..{chrome_right}",
            rows[row]
        );
    }
}

#[test]
fn inline_svg_text_uses_the_css_font_like_chrome() {
    let rows = [
        r#"<svg width="120" height="20"><text x="2" y="15" font-size="16">iiiiiii</text></svg>"#,
        r#"<svg width="120" height="20" font-family="Courier New"><text x="2" y="15" font-size="16">iiiiiii</text></svg>"#,
        r#"<svg width="120" height="20"><text x="2" y="15" font-size="16" font-family="monospace">iiiiiii</text></svg>"#,
        r#"<svg width="120" height="20" style="font-size:16px"><text x="2" y="15" style="font-weight:bold">iiiiiii</text></svg>"#,
    ];
    let buffer = render_rows(&rows);
    assert_ink_spans(&rows, &buffer, &[(3, 26), (4, 68), (3, 62), (3, 32)]);
}

#[test]
fn oblique_odd_weights_and_system_ui_match_chrome() {
    let rows = [
        r#"<svg width="120" height="20"><text x="2" y="15" style="font-style:oblique 10deg;font-size:16px">lllllll</text></svg>"#,
        r#"<svg width="120" height="20"><text x="2" y="15" font-size="16">lllllll</text></svg>"#,
        r#"<svg width="120" height="20"><text x="2" y="15" style="font-weight:550;font-size:16px">iiiiiii</text></svg>"#,
        r#"<svg width="120" height="20"><text x="2" y="15" style="font-family:system-ui;font-size:16px">mmmmm</text></svg>"#,
    ];
    let buffer = render_rows(&rows);
    let oblique_slant = ink_centre(&buffer, 3) - ink_centre(&buffer, 14);
    let upright_slant = ink_centre(&buffer, 23) - ink_centre(&buffer, 34);
    assert!(
        oblique_slant > 1.5,
        "oblique slant {oblique_slant}, Chrome 2.2"
    );
    assert!(upright_slant.abs() < 0.5, "upright slant {upright_slant}");
    assert_ink_spans(&rows[2..], &buffer[120 * 40 * 4..], &[(3, 32), (3, 70)]);
}
