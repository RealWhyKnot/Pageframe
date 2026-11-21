use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use pageframe::App;
use pageframe_paint::paint_scene;
use pageframe_test_harness::Harness;
use std::fmt::Write as _;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 400;
const ROW: usize = 195;

const SIDEWAYS_1X: [[usize; 4]; 24] = [
    [9, 164, 14, 169],
    [62, 27, 67, 32],
    [115, 148, 120, 153],
    [168, 43, 173, 48],
    [221, 164, 226, 169],
    [274, 27, 279, 32],
    [327, 148, 332, 153],
    [380, 43, 385, 48],
    [433, 164, 438, 169],
    [486, 27, 491, 32],
    [539, 148, 544, 153],
    [592, 43, 597, 48],
    [15, 364, 24, 373],
    [68, 213, 77, 222],
    [121, 339, 130, 348],
    [174, 238, 183, 247],
    [227, 364, 236, 373],
    [280, 213, 289, 222],
    [333, 339, 342, 348],
    [386, 238, 395, 247],
    [439, 364, 448, 373],
    [492, 213, 501, 222],
    [545, 339, 554, 348],
    [598, 238, 607, 247],
];

const SIDEWAYS_2X: [[usize; 4]; 24] = [
    [19, 324, 29, 334],
    [125, 62, 135, 72],
    [231, 298, 241, 308],
    [337, 88, 347, 98],
    [443, 324, 453, 334],
    [549, 62, 559, 72],
    [655, 298, 665, 308],
    [761, 88, 771, 98],
    [867, 324, 877, 334],
    [973, 62, 983, 72],
    [1079, 298, 1089, 308],
    [1185, 88, 1195, 98],
    [32, 723, 50, 741],
    [138, 435, 156, 453],
    [244, 680, 262, 698],
    [350, 478, 368, 496],
    [456, 723, 474, 741],
    [562, 435, 580, 453],
    [668, 680, 686, 698],
    [774, 478, 792, 496],
    [880, 723, 898, 741],
    [986, 435, 1004, 453],
    [1092, 680, 1110, 698],
    [1198, 478, 1216, 496],
];

const CIRCLES_1X: [[usize; 4]; 10] = [
    [26, 12, 31, 17],
    [128, 29, 133, 34],
    [278, 29, 283, 34],
    [369, 164, 374, 169],
    [518, 29, 523, 34],
    [17, 213, 26, 222],
    [134, 215, 143, 224],
    [268, 215, 277, 224],
    [375, 364, 384, 373],
    [508, 215, 517, 224],
];

const CIRCLES_2X: [[usize; 4]; 10] = [
    [58, 25, 68, 35],
    [256, 64, 266, 74],
    [555, 64, 565, 74],
    [739, 324, 749, 334],
    [1035, 64, 1045, 74],
    [41, 428, 59, 446],
    [267, 437, 285, 455],
    [534, 437, 552, 455],
    [752, 723, 770, 741],
    [1014, 437, 1032, 455],
];

fn render(html: &str, scale: u32) -> Vec<u8> {
    let page = App::new((), html)
        .page(WIDTH, HEIGHT)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let (width, height) = (WIDTH * scale, HEIGHT * scale);
    {
        let mut doc = harness.base_mut();
        let color_scheme = doc.get_viewport().color_scheme;
        doc.set_viewport(pageframe_traits::shell::Viewport::new(
            width,
            height,
            f32::from(u16::try_from(scale).expect("small scale")),
            color_scheme,
        ));
    }
    harness.pump();
    let mut doc = harness.base_mut();
    render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| paint_scene(scene, &mut doc, f64::from(scale), width, height, 0, 0),
        width,
        height,
    )
}

fn ink(buffer: &[u8], scale: usize, cell: [usize; 4]) -> Option<[usize; 4]> {
    let width = usize::try_from(WIDTH).expect("width fits") * scale;
    let [x0, y0, x1, y1] = cell.map(|edge| edge * scale);
    let mut found: Option<[usize; 4]> = None;
    for y in y0..y1 {
        for x in x0..x1 {
            let start = (y * width + x) * 4;
            let sum: usize = buffer[start..start + 3]
                .iter()
                .map(|&c| usize::from(c))
                .sum();
            if sum < 384 {
                found = Some(
                    found.map_or([x, y, x + 1, y + 1], |[left, top, right, bottom]| {
                        [left.min(x), top.min(y), right.max(x + 1), bottom.max(y + 1)]
                    }),
                );
            }
        }
    }
    found
}

fn sideways_page() -> String {
    let mut body = String::new();
    for (row, font_size) in [16, 30].into_iter().enumerate() {
        let mut col = 0;
        for shape in ["disc", "circle", "square"] {
            for class in ["", "in"] {
                for dir in ["ltr", "rtl"] {
                    write!(
                        body,
                        "<ul class=\"{class}\" dir=\"{dir}\" style=\"left:{}px;top:{}px;\
                         font-size:{font_size}px;list-style-type:{shape}\"><li>ab</li></ul>",
                        col * 53 + 2,
                        row * ROW + 5
                    )
                    .expect("a String accepts writes");
                    col += 1;
                }
            }
        }
    }
    format!(
        "<!doctype html><html><head><style>body{{margin:0;background:#fff;font-family:Arial}}\
         ul{{position:absolute;margin:0;padding:0;padding-inline-start:40px;\
         writing-mode:sideways-lr;width:48px;height:150px;list-style-position:outside}}\
         li{{color:transparent}}li::marker{{color:#000}}.in{{list-style-position:inside}}\
         </style></head><body>{body}</body></html>"
    )
}

fn circles_page() -> String {
    let mut body = String::new();
    for (row, font_size) in [16, 30].into_iter().enumerate() {
        for (col, (mode, class)) in [
            ("horizontal-tb", ""),
            ("vertical-lr", "v"),
            ("sideways-rl", "v"),
            ("sideways-lr", "v"),
            ("vertical-rl", "v"),
        ]
        .into_iter()
        .enumerate()
        {
            write!(
                body,
                "<ul class=\"{class}\" style=\"left:{}px;top:{}px;font-size:{font_size}px;\
                 writing-mode:{mode}\"><li>ab</li></ul>",
                col * 120 + 2,
                row * ROW + 5
            )
            .expect("a String accepts writes");
        }
    }
    format!(
        "<!doctype html><html><head><style>body{{margin:0;background:#fff;font-family:Arial}}\
         ul{{position:absolute;margin:0;padding:0;padding-inline-start:40px;width:150px;\
         height:48px;list-style-type:circle}}li{{color:transparent}}li::marker{{color:#000}}\
         .v{{width:48px;height:150px}}</style></head><body>{body}</body></html>"
    )
}

fn assert_cells(html: &str, column: usize, columns: usize, chrome: [&[[usize; 4]]; 2]) {
    for (scale, expected) in [1, 2].into_iter().zip(chrome) {
        let buffer = render(html, u32::try_from(scale).expect("small scale"));
        for (index, want) in expected.iter().enumerate() {
            let (row, col) = (index / columns, index % columns);
            let cell = [
                col * column,
                row * ROW,
                col * column + column,
                row * ROW + ROW,
            ];
            assert_eq!(
                ink(&buffer, scale, cell),
                Some(*want),
                "marker {index} at {scale}x"
            );
        }
    }
}

#[test]
fn sideways_lr_markers_match_chrome_for_every_shape() {
    assert_cells(&sideways_page(), 53, 12, [&SIDEWAYS_1X, &SIDEWAYS_2X]);
}

#[test]
fn circle_markers_close_like_chrome_in_every_writing_mode() {
    assert_cells(&circles_page(), 120, 5, [&CIRCLES_1X, &CIRCLES_2X]);
}
