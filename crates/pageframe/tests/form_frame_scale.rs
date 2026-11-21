use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use pageframe::App;
use pageframe_paint::paint_scene;
use pageframe_test_harness::Harness;

const WIDTH: u32 = 480;
const HEIGHT: u32 = 360;

const PAGE: &str = r#"<!doctype html><style>body{margin:0;background:#fff}div{position:absolute;transform-origin:0 0}</style>
<div style="left:20px;top:110px;transform:scale(3)"><input value="Abc" style="width:60px"></div>
<div style="left:300px;top:240px;transform:scale(3,2)"><input value="Abc" style="width:40px"></div>"#;

const UNIFORM_CORNER: [&str; 10] = [
    "............",
    "............",
    ".......#####",
    ".....#######",
    "....########",
    "...####.....",
    "...###......",
    "..###.......",
    "..###.......",
    "..###.......",
];

const STRETCHED_CORNER: [&str; 8] = [
    "............",
    "............",
    "......######",
    "....########",
    "...####.....",
    "..####......",
    "..###.......",
    "..###.......",
];

fn render(html: &str) -> Vec<u8> {
    let page = App::new((), html)
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

fn mask(pixels: &[u8], x0: usize, y0: usize, width: usize, height: usize) -> Vec<String> {
    (y0..y0 + height)
        .map(|y| {
            (x0..x0 + width)
                .map(|x| {
                    let at = (y * usize::try_from(WIDTH).expect("small width") + x) * 4;
                    let sum: u32 = pixels[at..at + 3].iter().map(|&v| u32::from(v)).sum();
                    if sum < 3 * 155 { '#' } else { '.' }
                })
                .collect()
        })
        .collect()
}

#[test]
fn text_field_frame_corners_scale_with_the_transform_like_chrome() {
    let pixels = render(PAGE);
    assert_eq!(mask(&pixels, 18, 108, 12, 10), UNIFORM_CORNER);
    assert_eq!(mask(&pixels, 298, 238, 12, 8), STRETCHED_CORNER);
}

#[test]
fn text_field_frame_under_a_huge_scale_paints_without_walking_every_row() {
    let pixels = render(
        r#"<!doctype html><style>body{margin:0;background:#fff}div{position:absolute;left:0;top:0;transform-origin:0 0;transform:translate(-3000000px,-50000px) scale(100000)}</style><div><input value="Abc" style="width:60px"></div>"#,
    );
    assert_eq!(mask(&pixels, 200, 150, 4, 2), ["####", "####"]);
}
