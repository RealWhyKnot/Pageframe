mod scaled;
mod support;

use pageframe::App;
use scaled::render_at_scale;
use support::{WIDTH, open, pixel, render};

const PAGE: &str = r#"<html><head><style>
body{margin:0;background:white}
input,textarea{position:absolute;left:4px;width:100px;font:13px sans-serif;border:1px solid black}
.red::placeholder{color:rgb(200,0,0)}
</style></head><body>
<input id="plain" style="top:2px" placeholder="Hello">
<input id="red" class="red" style="top:28px" placeholder="Hello">
<input id="filled" class="red" style="top:54px" placeholder="Hello" value="Hey">
</body></html>"#;

const TEXTAREA: &str = r#"<html><body style="margin:0;background:white">
<textarea style="position:absolute;left:4px;top:2px;width:100px;height:60px;font:13px monospace;border:1px solid black;resize:none" placeholder="one&#10;two"></textarea>
</body></html>"#;

struct Ink {
    darkest: u8,
    inked_rows: Vec<usize>,
    red: usize,
}

fn ink(buffer: &[u8], scale: u16, top: usize, bottom: usize) -> Ink {
    let scale = usize::from(scale);
    let width = usize::try_from(WIDTH).expect("test width fits in usize") * scale;
    let sample = |column: usize, row: usize| {
        if scale == 1 {
            return pixel(buffer, column, row);
        }
        let start = (row * width + column) * 4;
        let mut rgba = [0; 4];
        rgba.copy_from_slice(&buffer[start..start + 4]);
        rgba
    };
    let mut darkest = u8::MAX;
    let mut inked_rows = Vec::new();
    let mut red = 0;
    for row in top * scale..bottom * scale {
        let mut inked = false;
        for column in 8 * scale..100 * scale {
            let [r, g, b, _] = sample(column, row);
            if r > 150 && g < 80 && b < 80 {
                red += 1;
            }
            let low = r.min(g).min(b);
            if low < 230 {
                inked = true;
                darkest = darkest.min(low);
            }
        }
        if inked {
            inked_rows.push(row / scale);
        }
    }
    Ink {
        darkest,
        inked_rows,
        red,
    }
}

fn draw(page: &str, scale: u16) -> Vec<u8> {
    let mut harness = open(App::new((), page));
    if scale == 1 {
        return render(&mut harness);
    }
    render_at_scale(&mut harness, scale)
}

#[test]
fn empty_inputs_show_their_placeholder_in_chrome_grey() {
    for scale in [1_u16, 2] {
        let buffer = draw(PAGE, scale);
        let plain = ink(&buffer, scale, 6, 18);
        assert_eq!(plain.darkest, 117, "placeholder ink at {scale}x");
        assert_eq!(plain.red, 0, "plain placeholder is grey at {scale}x");
    }
}

#[test]
fn placeholder_pseudo_element_styles_apply() {
    for scale in [1_u16, 2] {
        let buffer = draw(PAGE, scale);
        let red = ink(&buffer, scale, 32, 44);
        assert!(
            red.red > 20,
            "styled placeholder at {scale}x has {} red pixels",
            red.red
        );
    }
}

#[test]
fn a_value_hides_the_placeholder() {
    for scale in [1_u16, 2] {
        let buffer = draw(PAGE, scale);
        let filled = ink(&buffer, scale, 58, 70);
        assert_eq!(filled.red, 0, "placeholder drawn under a value at {scale}x");
        assert!(filled.darkest < 60, "value text missing at {scale}x");
    }
}

#[test]
fn typing_hides_the_placeholder() {
    let mut harness = open(App::new((), PAGE));
    harness.click("#red");
    harness.type_text("Hey");
    harness.pump();
    let buffer = render(&mut harness);
    let typed = ink(&buffer, 1, 32, 44);
    assert_eq!(typed.red, 0, "placeholder still drawn after typing");
    assert!(typed.darkest < 60, "typed text missing");
}

#[test]
fn textarea_placeholders_keep_their_line_breaks() {
    for (scale, chrome_rows) in [(1_u16, (11, 31)), (2, (10, 32))] {
        let buffer = draw(TEXTAREA, scale);
        let area = ink(&buffer, scale, 4, 60);
        let first = area.inked_rows.first().copied().unwrap_or_default();
        let last = area.inked_rows.last().copied().unwrap_or_default();
        assert_eq!(
            (first, last),
            chrome_rows,
            "textarea placeholder rows at {scale}x"
        );
        assert_eq!(area.darkest, 117, "textarea placeholder ink at {scale}x");
    }
}
