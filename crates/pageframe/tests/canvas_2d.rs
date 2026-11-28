use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use pageframe_dom::{Document as _, DocumentConfig};
use pageframe_paint::paint_scene;
use pageframe_script::ScriptDocument;
use pageframe_traits::shell::{ColorScheme, Viewport};

const WIDTH: u32 = 200;
const HEIGHT: u32 = 100;
const ROW_PIXELS: usize = 200;

fn opaque_pixels(width: u32, height: u32) -> Vec<u8> {
    (0..width * height)
        .flat_map(|index| {
            let [first, second, third, fourth] = index.wrapping_mul(0x9e37_79b9).to_le_bytes();
            [first, second ^ fourth, third, 255]
        })
        .collect()
}

#[test]
fn opaque_scaled_draw_matches_general_sampling() {
    use pageframe_dom::canvas::{Affine, Canvas2D, ImageSource, Rect};

    let image = ImageSource {
        width: 64,
        height: 48,
        premultiplied: &opaque_pixels(64, 48),
    };
    let full = Rect::new(0.0, 0.0, 64.0, 48.0);
    let crop = Rect::new(2.5, 3.25, 60.625, 45.5);
    let outside = Rect::new(-4.0, -2.0, 58.0, 42.0);
    for (src, dst, transform) in [
        (full, Rect::new(-1.0, 2.0, 31.0, 26.0), Affine::IDENTITY),
        (crop, Rect::new(2.0, -4.0, 43.0, 22.0), Affine::IDENTITY),
        (outside, Rect::new(0.0, 0.0, 31.0, 22.0), Affine::IDENTITY),
        (
            crop,
            Rect::new(-2.0, 3.0, 25.0, 18.0),
            Affine::new([-1.0, 0.0, 0.0, 2.0, 31.0, 0.0]),
        ),
        (full, Rect::new(6.25, 8.5, 25.75, 22.25), Affine::IDENTITY),
    ] {
        for smooth in [false, true] {
            let mut general = Canvas2D::new(40, 30);
            general.state_mut().image_smoothing = smooth;
            general.set_transform(transform);
            let mut fast = Canvas2D::new(40, 30);
            fast.state_mut().image_smoothing = smooth;
            fast.set_transform(transform);
            if smooth {
                assert!(general.set_composite("copy"));
                assert!(fast.set_composite("copy"));
            }
            general.draw_image(&image, src, dst);
            fast.draw_opaque_image(&image, src, dst);
            for (index, (&actual, &expected)) in fast
                .premultiplied_pixels()
                .iter()
                .zip(general.premultiplied_pixels())
                .enumerate()
            {
                let tolerance = u8::from(smooth && index % 4 != 3);
                assert!(
                    actual.abs_diff(expected) <= tolerance,
                    "channel {index}, {src:?} to {dst:?}, smoothing {smooth}: {actual} versus {expected}",
                );
            }
        }
    }
}

#[test]
#[ignore = "times 1080p and 1440p opaque canvas draws"]
fn opaque_video_canvas_copy_benchmark() {
    use std::io::Write;
    use std::time::Instant;

    use pageframe_dom::canvas::{Canvas2D, ImageSource, Rect};

    for (width, height) in [(1920, 1080), (2560, 1440)] {
        let pixels = opaque_pixels(width, height);
        let image = ImageSource {
            width,
            height,
            premultiplied: &pixels,
        };
        let full = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
        for divisor in [1, 2] {
            let (out_width, out_height) = (width / divisor, height / divisor);
            let dst = Rect::new(0.0, 0.0, f64::from(out_width), f64::from(out_height));
            let mut before = Canvas2D::new(out_width, out_height);
            let mut after = Canvas2D::new(out_width, out_height);
            let mut before_times = [std::time::Duration::ZERO; 7];
            let mut after_times = before_times;
            after.draw_opaque_image(&image, full, dst);
            for (before_time, after_time) in before_times.iter_mut().zip(&mut after_times) {
                let start = Instant::now();
                assert!(
                    std::hint::black_box(&pixels)
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .all(|pixel| pixel[3] == 255)
                );
                if divisor == 1 {
                    before.draw_opaque_image(&image, full, dst);
                } else {
                    before.draw_image(&image, full, dst);
                }
                *before_time = start.elapsed();

                let start = Instant::now();
                after.draw_opaque_image(&image, full, dst);
                *after_time = start.elapsed();
            }
            assert!(
                before
                    .premultiplied_pixels()
                    .iter()
                    .zip(after.premultiplied_pixels())
                    .all(|(&a, &b)| a.abs_diff(b) <= u8::from(divisor == 2))
            );
            before_times.sort_unstable();
            after_times.sort_unstable();
            writeln!(
                std::io::stdout().lock(),
                "{width}x{height} to {out_width}x{out_height}, median of 7: before {:.3} ms, after {:.3} ms",
                before_times[3].as_secs_f64() * 1000.0,
                after_times[3].as_secs_f64() * 1000.0,
            )
            .unwrap();
        }
    }
}

const PAGE: &str = r##"<!doctype html><html><head><style>html,body{margin:0;background:#fff}canvas{display:block}</style></head><body><canvas id="c" width="200" height="100"></canvas><script>
const ctx = document.getElementById("c").getContext("2d");
ctx.fillStyle = "#101418";
ctx.fillRect(0, 0, 200, 100);
ctx.strokeStyle = "rgba(255, 255, 255, 0.25)";
ctx.lineWidth = 1;
ctx.beginPath();
for (let y = 20.5; y < 100; y += 20) {
    ctx.moveTo(0, y);
    ctx.lineTo(200, y);
}
ctx.stroke();
const fill = ctx.createLinearGradient(0, 30, 0, 90);
fill.addColorStop(0, "rgba(80, 160, 255, 0.8)");
fill.addColorStop(1, "rgba(80, 160, 255, 0)");
const points = [[10, 70], [40, 50], [70, 60], [100, 35], [130, 45], [160, 30], [190, 55]];
ctx.beginPath();
ctx.moveTo(10, 90);
for (const [x, y] of points) ctx.lineTo(x, y);
ctx.lineTo(190, 90);
ctx.closePath();
ctx.fillStyle = fill;
ctx.fill();
ctx.beginPath();
points.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)));
ctx.strokeStyle = "#50a0ff";
ctx.lineWidth = 2;
ctx.lineJoin = "round";
ctx.stroke();
ctx.save();
ctx.setLineDash([4, 3]);
ctx.strokeStyle = "#ffcc00";
ctx.lineWidth = 1;
ctx.beginPath();
ctx.moveTo(0, 25.5);
ctx.lineTo(200, 25.5);
ctx.stroke();
ctx.restore();
ctx.fillStyle = "#ff5050";
ctx.beginPath();
ctx.arc(170, 75, 10, 0, Math.PI * 2);
ctx.fill();
ctx.fillStyle = "#e0e0e0";
ctx.font = "11px 'Segoe UI', system-ui, sans-serif";
ctx.textBaseline = "top";
ctx.fillText("16.7 ms", 6, 4);
ctx.textAlign = "right";
ctx.fillText("60 fps", 194, 4);
const off = new OffscreenCanvas(30, 10);
const octx = off.getContext("2d");
octx.fillStyle = "#00ff80";
octx.fillRect(0, 0, 30, 10);
ctx.drawImage(off, 6, 84);
ctx.save();
ctx.beginPath();
ctx.rect(100, 80, 40, 15);
ctx.clip();
ctx.fillStyle = "#ffffff";
ctx.fillRect(90, 70, 80, 40);
ctx.restore();
</script></body></html>"##;

const CHROME_SAMPLES: [(usize, usize, [u8; 3], u8); 16] = [
    (100, 10, [16, 20, 24], 3),
    (5, 20, [76, 79, 82], 3),
    (1, 25, [255, 204, 0], 3),
    (5, 25, [16, 20, 24], 3),
    (100, 70, [32, 56, 84], 3),
    (55, 55, [80, 160, 255], 24),
    (170, 75, [255, 80, 80], 3),
    (182, 88, [17, 23, 28], 3),
    (20, 89, [0, 255, 128], 3),
    (40, 89, [16, 21, 26], 3),
    (120, 87, [255, 255, 255], 3),
    (95, 87, [18, 25, 32], 3),
    (120, 97, [16, 20, 24], 3),
    (10, 60, [76, 79, 82], 3),
    (197, 10, [16, 20, 24], 3),
    (160, 31, [75, 150, 240], 24),
];

fn render(html: &str) -> Vec<u8> {
    let config = DocumentConfig {
        viewport: Some(Viewport::new(WIDTH, HEIGHT, 1.0, ColorScheme::Light)),
        ..Default::default()
    };
    let mut doc = ScriptDocument::from_html(html, config).without_timer_thread();
    doc.execute_scripts();
    doc.poll(None);
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
    let mut inner = doc.inner_mut();
    inner.resolve(0.0);
    let buffer = render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| paint_scene(scene, &mut inner, 1.0, WIDTH, HEIGHT, 0, 0),
        WIDTH,
        HEIGHT,
    );
    drop(inner);
    buffer
}

fn pixel(buffer: &[u8], x: usize, y: usize) -> [u8; 3] {
    let start = (y * ROW_PIXELS + x) * 4;
    [buffer[start], buffer[start + 1], buffer[start + 2]]
}

fn ink(buffer: &[u8], xs: std::ops::Range<usize>, ys: std::ops::Range<usize>) -> usize {
    ys.flat_map(|y| xs.clone().map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(buffer, x, y)[0] > 96)
        .count()
}

#[test]
fn script_drawn_chart_matches_chrome_pixels() {
    let buffer = render(PAGE);
    let misses: Vec<String> = CHROME_SAMPLES
        .iter()
        .filter_map(|&(x, y, want, tolerance)| {
            let got = pixel(&buffer, x, y);
            let close = got
                .iter()
                .zip(want)
                .all(|(g, w)| g.abs_diff(w) <= tolerance);
            (!close).then(|| format!("({x},{y}) got {got:?} want {want:?}"))
        })
        .collect();
    assert!(misses.is_empty(), "{misses:#?}");
}

#[test]
fn canvas_text_inks_about_as_much_as_chrome() {
    let buffer = render(PAGE);
    let left = ink(&buffer, 0..60, 0..18);
    let right = ink(&buffer, 140..200, 0..18);
    assert!((89..=121).contains(&left), "left ink {left}, chrome 105");
    assert!(
        (101..=137).contains(&right),
        "right ink {right}, chrome 119"
    );
}

#[test]
fn resizing_the_canvas_clears_what_was_painted() {
    let html = PAGE.replace(
        "ctx.restore();\n</script>",
        "ctx.restore();\nctx.canvas.width = 200;\n</script>",
    );
    let buffer = render(&html);
    assert_eq!(pixel(&buffer, 100, 10), [255, 255, 255]);
    assert_eq!(pixel(&buffer, 170, 75), [255, 255, 255]);
}
