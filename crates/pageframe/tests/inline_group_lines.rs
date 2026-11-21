use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use pageframe::App;
use pageframe_paint::paint_scene;
use pageframe_test_harness::Harness;

const WIDTH: u32 = 240;
const HEIGHT: u32 = 160;

fn render(body: &str) -> Vec<u8> {
    let html = format!(
        "<html><body style=\"margin:0;background:white;font:20px/20px Arial\">{body}</body></html>"
    );
    let page = App::new((), &html)
        .page(WIDTH, HEIGHT)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let mut doc = harness.base_mut();
    render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| paint_scene(scene, &mut doc, 1.0, WIDTH, HEIGHT, 0, 0),
        WIDTH,
        HEIGHT,
    )
}

fn inked(buffer: &[u8]) -> usize {
    buffer
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[..3] != [255, 255, 255])
        .count()
}

fn line_ink(buffer: &[u8]) -> Vec<usize> {
    let width = usize::try_from(WIDTH).expect("small width");
    buffer.chunks(width * 4 * 20).map(inked).collect()
}

fn clamped(text: &str) -> String {
    format!(
        "<div style=\"width:100px;display:-webkit-box;-webkit-box-orient:vertical;overflow:hidden;-webkit-line-clamp:2\">{text}</div>"
    )
}

#[test]
fn spans_on_later_lines_paint_like_plain_text() {
    let plain = line_ink(&render(
        "<p style=\"width:120px;margin:0\">aa bb cc dd ee ff gg hh ii jj kk ll</p>",
    ));
    let grouped = line_ink(&render(
        "<p style=\"width:120px;margin:0\">aa bb <span style=\"position:relative\">cc dd ee</span> ff <span style=\"position:relative\">gg <span style=\"position:relative\">hh ii</span></span> jj kk <span style=\"position:relative\">ll</span></p>",
    ));
    assert!(plain.iter().filter(|&&ink| ink > 0).count() >= 3);
    for (line, (plain, grouped)) in plain.iter().zip(&grouped).enumerate() {
        assert!(
            plain.abs_diff(*grouped) * 20 <= *plain,
            "line {line}: {plain} vs {grouped}"
        );
    }
}

#[test]
fn line_clamp_ellipsis_lands_inside_a_span_that_starts_on_a_later_line() {
    let plain = render(&clamped("HH HH HH HH HH HH HH HH HH"));
    let grouped = render(&clamped(
        "HH HH HH <span style=\"position:relative\">HH HH HH HH HH HH</span>",
    ));
    assert!(inked(&plain) > 0);
    assert!(plain == grouped);
}
