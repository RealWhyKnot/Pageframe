use super::{App, BASE, LIME, open, scaled, support};

fn check_fragments(body: &str, scale: u16, rectangles: &[[usize; 4]]) {
    let html = format!(
        "<!doctype html><style>{BASE}span{{background:#0f0;color:transparent}}</style>{body}"
    );
    let buffer = scaled::render_at_scale(&mut open(App::new((), &html)), scale);
    let width =
        usize::try_from(support::WIDTH).expect("viewport width fits in usize") * usize::from(scale);
    for (index, &pixel) in buffer.as_chunks::<4>().0.iter().enumerate() {
        let (x, y) = (index % width, index / width);
        let green = rectangles.iter().any(|&[left, top, right, bottom]| {
            (left..right).contains(&x) && (top..bottom).contains(&y)
        });
        assert_eq!(pixel == LIME, green, "{scale}x at ({x}, {y})");
    }
}

#[test]
fn plaintext_span_keeps_its_primary_font_height_after_a_break() {
    let body = "<div style='unicode-bidi:plaintext;line-height:40px;width:30px'>\
                <span style='padding:0 5px;box-decoration-break:clone'>אא<br>אא</span></div>";
    check_fragments(body, 1, &[[10, 14, 30, 26], [10, 54, 30, 66]]);
    check_fragments(body, 2, &[[21, 28, 60, 51], [21, 108, 60, 131]]);
}

#[test]
fn wrapped_clone_keeps_its_primary_font_height_on_the_last_line() {
    let body = "<div style='line-height:20px;width:100px'>a \
                <span style='box-decoration-break:clone;padding:0 30px 0 4px'>\
                אאאא אאאא אאאא אאאא</span></div>";
    check_fragments(body, 1, &[[11, 4, 114, 16], [0, 24, 53, 36]]);
    check_fragments(body, 2, &[[22, 8, 228, 31], [0, 48, 107, 71]]);
}
