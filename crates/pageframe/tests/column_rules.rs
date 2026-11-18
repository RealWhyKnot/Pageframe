use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use pageframe::{App, Page};
use pageframe_paint::paint_scene;
use pageframe_test_harness::Harness;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 400;

fn draw(body: &str) -> Vec<u8> {
    let html = format!(
        "<html><head><style>html,body{{margin:0;background:white}}</style></head>\
         <body>{body}</body></html>"
    );
    let page = App::new((), &html)
        .page(WIDTH, HEIGHT)
        .expect("test pages have valid selectors");
    let mut harness: Harness<Page<()>> = Harness::wrap(page);
    harness.pump();
    let mut doc = harness.base_mut();
    render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| paint_scene(scene, &mut doc, 1.0, WIDTH, HEIGHT, 0, 0),
        WIDTH,
        HEIGHT,
    )
}

fn pixel(buffer: &[u8], x: usize, y: usize) -> [u8; 4] {
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    let start = (y * width + x) * 4;
    let mut rgba = [0; 4];
    rgba.copy_from_slice(&buffer[start..start + 4]);
    rgba
}

#[test]
fn column_rules_span_the_content_box_like_chrome() {
    let blue = [0, 0, 255, 255];
    let blocks = "<div style=\"height:60px\"></div><div style=\"height:60px\"></div>\
                  <div style=\"height:40px\"></div>";
    let style = "<style>.m{columns:2;column-gap:20px;column-rule:4px solid #00f;width:120px;\
                 margin:4px;float:left}.m div{background:#ccc}\
                 .v{width:200px;height:120px}.v div{height:auto}</style>";
    let horizontal = format!(
        "{style}<div class=\"m\" style=\"height:200px\">{blocks}</div>\
         <div class=\"m\" style=\"min-height:150px\">{blocks}</div>"
    );
    let vertical = "<div class=\"m v\" style=\"writing-mode:vertical-rl\"><div style=\"width:60px\">\
                    </div><div style=\"width:60px\"></div><div style=\"width:40px\"></div></div>\
                    <div class=\"m v\" style=\"writing-mode:vertical-lr\"><div style=\"width:60px\">\
                    </div><div style=\"width:60px\"></div><div style=\"width:40px\"></div></div>";
    let cases = [
        (
            horizontal,
            [(0, 128, [62, 4, 65, 203]), (128, 256, [190, 4, 193, 153])],
        ),
        (
            format!("{style}{vertical}"),
            [(0, 208, [4, 62, 203, 65]), (208, 416, [212, 62, 411, 65])],
        ),
    ];
    for (body, rules) in cases {
        let buffer = draw(&body);
        for (left, right, chrome) in rules {
            let mut found: Option<[usize; 4]> = None;
            for y in 0..usize::try_from(HEIGHT).expect("test height fits in usize") {
                for x in left..right {
                    if pixel(&buffer, x, y) == blue {
                        found = Some(found.map_or([x, y, x, y], |[x0, y0, x1, _]| {
                            [x0.min(x), y0, x1.max(x), y]
                        }));
                    }
                }
            }
            assert_eq!(found, Some(chrome), "{left}..{right}");
        }
    }
}
