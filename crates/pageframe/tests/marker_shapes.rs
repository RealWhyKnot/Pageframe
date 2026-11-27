use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use pageframe::App;
use pageframe_dom::{DocumentConfig, FontContext};
use pageframe_html::HtmlDocument;
use pageframe_paint::paint_scene;
use pageframe_test_harness::Harness;
use pageframe_traits::shell::{ColorScheme, Viewport};
use parley::fontique::{Collection, CollectionOptions};
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

#[test]
fn vertical_disclosure_markers_match_chrome_direction_and_size() {
    let page = r#"<!doctype html><style>
body { margin:0; background:white; font:40px/40px Arial }
details { position:absolute; top:20px; width:80px; height:90px; color:transparent }
summary::marker { color:black }
</style>
<details style="left:20px;writing-mode:vertical-rl"><summary>X</summary></details>
<details style="left:120px;writing-mode:vertical-lr"><summary>X</summary></details>
<details open style="left:220px;writing-mode:vertical-rl"><summary>X</summary></details>
<details open style="left:320px;writing-mode:vertical-lr"><summary>X</summary></details>"#;
    let buffer = render(page, 1);
    let width = usize::try_from(WIDTH).expect("width fits");
    let dark = |x: usize, y: usize| buffer[(y * width + x) * 4] < 128;
    let chrome_sizes = [(26, 22), (26, 22), (22, 25), (22, 26)];
    for (column, (chrome_width, chrome_height)) in chrome_sizes.into_iter().enumerate() {
        let cell_left = column * 100;
        let [left, top, right, bottom] =
            ink(&buffer, 1, [cell_left, 0, cell_left + 100, 120]).expect("marker has ink");
        assert!(
            (right - left).abs_diff(chrome_width) <= 2,
            "width in column {column}"
        );
        assert!(
            (bottom - top).abs_diff(chrome_height) <= 2,
            "height in column {column}"
        );
        let (base, tip) = match column {
            0 | 1 => (
                (left..right).filter(|&x| dark(x, top + 2)).count(),
                (left..right).filter(|&x| dark(x, bottom - 1)).count(),
            ),
            2 => (
                (top..bottom).filter(|&y| dark(right - 3, y)).count(),
                (top..bottom).filter(|&y| dark(left, y)).count(),
            ),
            _ => (
                (top..bottom).filter(|&y| dark(left + 2, y)).count(),
                (top..bottom).filter(|&y| dark(right - 1, y)).count(),
            ),
        };
        assert!(base > 2 * tip, "base {base}, tip {tip} in column {column}");
    }
}

#[test]
fn disclosure_markers_match_chrome_when_rotated_or_outside() {
    let page = r#"<!doctype html><style>
body { margin:0; background:white; font:40px/40px Arial }
details { position:absolute; top:20px; width:80px; height:90px; color:transparent }
summary::marker { color:black }
</style>
<details style="left:120px;transform-origin:0 0;transform:rotate(90deg)"><summary>X</summary></details>
<details style="left:240px"><summary style="list-style-position:outside">X</summary></details>
<details open style="left:340px"><summary style="list-style-position:outside">X</summary></details>"#;
    let buffer = render(page, 1);
    for (left, expected) in [
        (50, [86, 20, 112, 42]),
        (150, [206, 28, 228, 53]),
        (250, [306, 29, 331, 52]),
    ] {
        let actual = ink(&buffer, 1, [left, 0, left + 100, 120]).expect("marker has ink");
        assert!(
            actual
                .into_iter()
                .zip(expected)
                .all(|(actual, expected)| actual.abs_diff(expected) <= 2),
            "marker in column starting at {left}: {actual:?}, Chrome: {expected:?}",
        );
    }
}

#[test]
fn outside_vertical_disclosure_markers_match_chrome_in_both_directions() {
    let page = r#"<!doctype html><style>
body { margin:0; background:white; font:40px/40px Arial }
details { position:absolute; top:40px; width:80px; height:90px; color:transparent }
summary { list-style-position:outside }
summary::marker { color:black }
</style>
<details open dir="ltr" style="left:20px;writing-mode:vertical-lr"><summary>X</summary></details>
<details open dir="rtl" style="left:170px;writing-mode:vertical-lr"><summary>X</summary></details>
<details open dir="ltr" style="left:320px;writing-mode:vertical-rl"><summary>X</summary></details>
<details open dir="rtl" style="left:470px;writing-mode:vertical-rl"><summary>X</summary></details>
<details dir="ltr" style="left:20px;top:220px;writing-mode:vertical-lr"><summary>X</summary></details>
<details dir="rtl" style="left:170px;top:220px;writing-mode:vertical-lr"><summary>X</summary></details>
<details dir="ltr" style="left:320px;top:220px;writing-mode:vertical-rl"><summary>X</summary></details>
<details dir="rtl" style="left:470px;top:220px;writing-mode:vertical-rl"><summary>X</summary></details>"#;
    let buffer = render(page, 1);
    let chrome = [
        [26, 6, 48, 32],
        [176, 138, 198, 164],
        [370, 7, 392, 32],
        [520, 139, 542, 164],
        [26, 188, 52, 210],
        [177, 320, 202, 343],
        [366, 188, 392, 210],
        [517, 320, 542, 343],
    ];
    let actual = std::array::from_fn::<_, 8, _>(|index| {
        let left = index % 4 * 150;
        let top = index / 4 * 180;
        ink(&buffer, 1, [left, top, left + 150, top + 180]).expect("marker has ink")
    });
    assert!(
        actual.into_iter().zip(chrome).all(|(actual, expected)| {
            actual
                .into_iter()
                .zip(expected)
                .all(|(actual, expected)| actual.abs_diff(expected) <= 2)
        }),
        "Pageframe: {actual:?}, Chrome: {chrome:?}",
    );
}

#[test]
fn inside_and_outside_disclosure_markers_paint_without_fonts() {
    let page = r#"<!doctype html><style>
body { margin:0; padding-top:20px; background:white; font:40px/40px Arial }
details { margin-left:40px; width:100px; color:transparent }
summary::marker { color:black }
.outside summary { list-style-position:outside }
.vertical { position:absolute; top:60px; margin:0; width:80px; height:100px }
.rl { left:80px; writing-mode:vertical-rl }
.lr { left:220px; writing-mode:vertical-lr }
</style>
<details><summary id="horizontal">X</summary></details>
<details open><summary>X</summary></details>
<details class="outside"><summary>X</summary></details>
<details open class="outside"><summary>X</summary></details>
<details class="vertical rl"><summary id="rl">X</summary></details>
<details class="vertical lr"><summary id="lr">X</summary></details>"#;
    let mut fonts = FontContext {
        collection: Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        }),
        ..Default::default()
    };
    assert_eq!(fonts.collection.family_names().next(), None);
    let mut doc = HtmlDocument::from_html(
        page,
        DocumentConfig {
            font_ctx: Some(fonts),
            viewport: Some(Viewport::new(WIDTH, HEIGHT, 1.0, ColorScheme::Light)),
            ..Default::default()
        },
    );
    doc.resolve(0.0);
    let buffer = render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| paint_scene(scene, &mut doc, 1.0, WIDTH, HEIGHT, 0, 0),
        WIDTH,
        HEIGHT,
    );
    for (label, top, left, right) in [
        ("inside open", 60, 40, 140),
        ("outside closed", 100, 0, 40),
        ("outside open", 140, 0, 40),
    ] {
        assert!(
            ink(&buffer, 1, [left, top, right, top + 40]).is_some(),
            "{label} marker has no ink"
        );
    }
    for (selector, cell, expected) in [
        ("#horizontal", [0, 0, 150, 60], [40, 28, 62, 54]),
        ("#rl", [80, 50, 190, 95], [126, 60, 152, 82]),
        ("#lr", [200, 50, 320, 95], [240, 60, 266, 82]),
    ] {
        let actual = ink(&buffer, 1, cell).expect("marker has ink");
        assert_eq!(actual, expected, "{selector} marker bounds");
        let id = doc.query_selector(selector).unwrap().unwrap();
        let summary = doc.get_client_bounding_rect(id).expect("summary has a box");
        let [left, top, right, bottom] = actual.map(|edge| f64::from(u32::try_from(edge).unwrap()));
        assert!(
            left >= summary.x
                && top >= summary.y
                && right <= summary.x + summary.width
                && bottom <= summary.y + summary.height,
            "{selector} marker {actual:?} exceeds summary {summary:?}"
        );
    }
}
