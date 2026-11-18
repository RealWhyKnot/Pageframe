use super::*;

const SPANS: &str = "<html><head><style>html,body{margin:0;background:#fff;font:14px/30px Arial}\
    div{margin-bottom:4px}</style></head><body>\
    <div><span id=plain>plain</span> x</div>\
    <div><span id=padded style='padding:3px 0 5px;border-top:2px solid #000;\
    border-bottom:1px solid #000'>padded</span> x</div>\
    <div><span id=outer style='font-size:20px'>a<span id=inner style='font-size:10px'>b</span>\
    </span></div>\
    <div><span id=nest><b id=bold style='font-size:18px'>big</b></span> x</div>\
    <div><span id=wrap>a <span style='display:inline-block;width:20px;height:50px'></span> b\
    </span></div>\
    <div style='line-height:normal'>e <sub id=sub>sub</sub><sup id=sup>sup</sup></div>\
    <div><span id=lone><span style='display:inline-block;width:20px;height:40px'></span>\
    </span></div>\
    </body></html>";

const FONTS: &str = "<html><head><style>html,body{margin:0;background:#fff;font:14px/30px Arial}\
    div{margin-bottom:4px}</style></head><body>\
    <div><span id=fallback>漢字 text</span> x</div>\
    <div><span id=borrow><code>mono</code></span> x</div>\
    <div><span id=larger><span style='font:20px Georgia'>big</span></span> x</div>\
    <div style='writing-mode:vertical-lr;height:80px'>ab <span id=lr \
    style='padding:2px 4px 6px 8px;border-left:3px solid #000'>vert</span></div>\
    <div style='writing-mode:vertical-rl;height:80px'>ab <span id=rl \
    style='padding:2px 4px 6px 8px;border-left:3px solid #000'>vert</span></div>\
    </body></html>";

const SIDES: &str = "<html><head><style>html,body{margin:0;background:#fff;font:14px/24px Arial}\
    section{position:absolute}span{background:#0f0}</style></head><body>\
    <section style='left:320px;top:4px;writing-mode:vertical-rl;width:300px;height:100px'>ab \
    <span id=vblock style='padding-left:10%'>cd</span></section>\
    <section style='left:4px;top:120px;width:300px;height:100px'>ab \
    <span id=hblock style='padding-top:10%'>cd</span></section></body></html>";

const SIDEWAYS: &str = "<html><head><style>html,body{margin:0;background:#fff;font:14px/24px Arial}\
    section{position:absolute;top:4px;height:100px;width:60px}span{background:#0f0}</style>\
    </head><body><section style='left:4px;writing-mode:sideways-lr'>ab <span id=lr \
    style='padding:2px 4px 6px 8px;border-bottom:3px solid #000'>cd</span> <i id=ib \
    style='display:inline-block;width:10px;height:6px;margin:0 2px 0 5px'></i></section>\
    <section style='left:100px;writing-mode:sideways-lr;direction:rtl'>ab <span id=rtl \
    style='padding:2px 4px 6px 8px'>cd</span></section></body></html>";

const SIDE_MARGINS: &str = "<html><head><style>html,body{margin:0;background:#fff;\
    font:14px/24px Arial}section{position:absolute;top:4px;height:100px;width:60px;\
    writing-mode:sideways-lr}i{display:inline-block;width:10px;height:6px}</style></head><body>\
    <section style='left:4px'>ab<i id=bottom style='margin:0 0 10px 0'></i>cd</section>\
    <section style='left:70px'>ab<i id=top style='margin:10px 0 0 0'></i>cd</section>\
    <section style='left:140px;padding:5px 3px 9px 7px;border-bottom:4px solid #000'>ab\
    <i id=padded></i>cd</section>\
    <section style='left:240px;direction:rtl'>ab<i id=rtl style='margin:0 0 10px 0'></i>cd\
    </section></body></html>";

const TRAILING: &str = "<html><head><style>html,body{margin:0;background:#fff;\
    font:14px/24px Arial}div{width:90px;margin-bottom:6px}span{background:#0f0}</style></head>\
    <body><div><span id=wrap>quick brown fox jumps over</span></div>\
    <div>ab <span id=tail>cd </span>efghijklmn</div>\
    <div style='white-space:pre-wrap'><span id=prew>quick brown   fox jumps</span></div>\
    <div><span id=nbsp>quick brown&nbsp; fox</span></div>\
    <div dir=rtl><span id=rtl>quick brown fox jumps</span></div>\
    <div><span id=clone style='box-decoration-break:clone;padding:0 4px 0 8px'>\
    ab cd ef gh ij</span></div></body></html>";

fn client_rects<const COUNT: usize>(
    html: &str,
    scale: f32,
    ids: [&str; COUNT],
) -> Vec<pageframe_dom::BoundingRect> {
    let mut harness = open(App::new((), html));
    {
        let mut doc = harness.base_mut();
        let color_scheme = doc.get_viewport().color_scheme;
        doc.set_viewport(pageframe_traits::shell::Viewport::new(
            1600,
            1200,
            scale,
            color_scheme,
        ));
    }
    harness.pump();
    let doc = harness.base_mut();
    ids.map(|id| {
        let node = doc
            .query_selector(&format!("#{id}"))
            .expect("valid selector")
            .expect("the span exists");
        doc.get_client_bounding_rect(node)
            .expect("the span is laid out")
    })
    .to_vec()
}

fn vertical_extents(scale: f32) -> Vec<[f64; 2]> {
    client_rects(
        SPANS,
        scale,
        [
            "plain", "padded", "outer", "inner", "nest", "bold", "wrap", "sub", "sup", "lone",
        ],
    )
    .iter()
    .map(|rect| [rect.y, rect.height])
    .collect()
}

#[test]
fn inline_client_rects_use_the_primary_font_like_chrome() {
    let chrome = [
        (
            1.0,
            [[7.0, 16.0], [41.0, 16.0], [76.0, 16.0]],
            [-4.0, 31.0, 31.343_75],
        ),
        (
            2.0,
            [[7.0, 15.5], [41.0, 15.5], [75.0, 15.5]],
            [-3.5, 30.5, 31.343_75],
        ),
    ];
    for (scale, horizontal, vertical) in chrome {
        let rects = client_rects(FONTS, scale, ["fallback", "borrow", "larger", "lr", "rl"]);
        let found: Vec<[f64; 2]> = rects[..3]
            .iter()
            .map(|rect| [rect.y, rect.height])
            .collect();
        assert_eq!(found, horizontal, "scale {scale}");
        let found: Vec<[f64; 3]> = rects[3..]
            .iter()
            .map(|rect| [rect.x, rect.width, rect.height])
            .collect();
        assert_eq!(found, [vertical, vertical], "scale {scale}");
    }
}

#[test]
fn percent_block_side_padding_resolves_like_chrome() {
    let rects: Vec<[f64; 4]> = client_rects(SIDES, 1.0, ["vblock", "hblock"])
        .iter()
        .map(|rect| [rect.x, rect.y, rect.width, rect.height])
        .collect();
    let chrome = [
        [590.0, 23.468_75, 26.0, 14.796_875],
        [23.468_75, 94.0, 14.796_875, 46.0],
    ];
    assert_eq!(rects, chrome);
}

#[test]
fn inline_client_rects_cover_the_font_content_area_like_chrome() {
    let chrome = vec![
        [7.0, 16.0],
        [36.0, 27.0],
        [72.0, 22.0],
        [81.0, 11.0],
        [114.0, 16.0],
        [111.0, 20.0],
        [178.0, 16.0],
        [214.453_125, 13.0],
        [205.0, 13.0],
        [258.453_125, 16.0],
    ];
    assert_eq!(vertical_extents(1.0), chrome);
}

#[test]
fn inline_client_rects_round_the_content_area_in_device_pixels_at_2x() {
    let chrome = vec![
        [7.0, 15.5],
        [36.0, 26.5],
        [72.0, 22.0],
        [81.0, 11.0],
        [114.0, 15.5],
        [110.0, 20.5],
        [178.5, 15.5],
        [213.960_937_5, 13.0],
        [205.5, 13.0],
        [258.960_937_5, 15.5],
    ];
    assert_eq!(vertical_extents(2.0), chrome);
}

#[test]
fn sideways_lr_client_rects_run_bottom_to_top_like_chrome() {
    let rects: Vec<[f64; 4]> = client_rects(SIDEWAYS, 1.0, ["lr", "ib", "rtl"])
        .iter()
        .map(|rect| [rect.x, rect.y, rect.width, rect.height])
        .collect();
    assert_eq!(
        rects,
        [
            [0.0, 58.734_375, 28.0, 25.796_875],
            [9.0, 48.843_75, 10.0, 6.0],
            [96.0, 4.0, 28.0, 22.796_875],
        ]
    );
}

#[test]
fn sideways_lr_inline_blocks_take_the_bottom_margin_first_like_chrome() {
    let rects: Vec<[f64; 2]> = client_rects(SIDE_MARGINS, 1.0, ["bottom", "top", "padded", "rtl"])
        .iter()
        .map(|rect| [rect.x, rect.y])
        .collect();
    assert_eq!(
        rects,
        [
            [11.0, 72.421_875],
            [77.0, 82.421_875],
            [154.0, 87.421_875],
            [247.0, 18.796_875],
        ]
    );
}

fn fragment_rects(html: &str, id: &str) -> Vec<[f64; 2]> {
    let mut harness = open(App::new((), html));
    harness.pump();
    let doc = harness.base_mut();
    let node = doc
        .query_selector(&format!("#{id}"))
        .expect("valid selector")
        .expect("the span exists");
    doc.inline_fragment_rects(node)
        .expect("the span is laid out")
        .map(|rect| [rect.x, rect.width])
        .collect()
}

#[test]
fn line_end_spaces_leave_inline_fragments_like_chrome() {
    let expected: [(&str, &[[f64; 2]]); 5] = [
        (
            "wrap",
            &[[0.0, 74.703_125], [0.0, 59.921_875], [0.0, 27.234_375]],
        ),
        ("prew", &[[0.0, 86.375], [0.0, 59.921_875]]),
        ("nbsp", &[[0.0, 78.593_75], [0.0, 18.687_5]]),
        ("rtl", &[[15.296_875, 74.703_125], [30.078_125, 59.921_875]]),
        ("clone", &[[0.0, 81.281_25], [0.0, 18.234_375]]),
    ];
    for (id, rects) in expected {
        assert_eq!(fragment_rects(TRAILING, id), rects, "{id}");
    }
    let tail = fragment_rects(TRAILING, "tail");
    assert_eq!(tail.len(), 1, "{tail:?}");
    assert_eq!(tail[0][0], 19.468_75, "{tail:?}");
    assert!((tail[0][1] - 14.796_875).abs() <= 1.0 / 64.0, "{tail:?}");
}

const SPAN_ENDS: &str = "<html><head><style>html,body{margin:0;background:#fff;\
    font:14px/24px Arial}div{width:90px;margin-bottom:6px}span{background:#0f0}</style></head>\
    <body><div dir=rtl><span id=rend>quick brown </span>fox jumps</div>\
    <div dir=rtl>ab <span id=rtail>cd </span>efghijklmn</div>\
    <div><span id=outer><span style='box-decoration-break:clone;padding:0 4px 0 8px'>\
    ab cd ef gh ij</span></span></div>\
    <div><span id=outer2>xy <span style='box-decoration-break:clone;padding:0 4px 0 8px'>\
    ab cd ef gh</span> z</span></div>\
    <div style=text-align:right><span id=lright>quick brown </span>fox jumps</div>\
    <div style=text-align:center><span id=lcenter>quick brown </span>fox jumps</div>\
    <div><span id=deep><span><span style='box-decoration-break:clone;padding-left:8px'>\
    ab cd ef gh ij kl mn</span></span></span></div></body></html>";

#[test]
fn spaces_before_span_ends_hang_and_clone_insets_reach_outer_spans_like_chrome() {
    let expected: [(&str, &[[f64; 2]]); 6] = [
        ("rend", &[[15.296_875, 74.703_125]]),
        ("lright", &[[15.296_875, 74.703_125]]),
        ("lcenter", &[[7.640_625, 74.703_125]]),
        ("deep", &[[0.0, 87.390_625], [0.0, 41.453_125]]),
        ("outer", &[[0.0, 81.281_25], [0.0, 18.234_375]]),
        ("outer2", &[[0.0, 79.718_75], [0.0, 38.468_75]]),
    ];
    for (id, rects) in expected {
        assert_eq!(fragment_rects(SPAN_ENDS, id), rects, "{id}");
    }
    let tail = fragment_rects(SPAN_ENDS, "rtail");
    assert_eq!(tail.len(), 1, "{tail:?}");
    assert!((tail[0][0] - 75.203_125).abs() <= 1.0 / 64.0, "{tail:?}");
    assert!((tail[0][1] - 14.796_875).abs() <= 1.0 / 64.0, "{tail:?}");
}

const CENTERED: &str = "<html><head><style>html,body{margin:0;background:#fff;\
    font:14px/24px Arial}div{width:90px;text-align:center}</style></head>\
    <body><div><span id=ltr>fox jumps</span></div>\
    <div dir=rtl><span id=rtl>fox jumps</span></div></body></html>";

#[test]
fn centered_lines_floor_their_offset_to_a_layout_unit_like_chrome() {
    for id in ["ltr", "rtl"] {
        assert_eq!(
            fragment_rects(CENTERED, id),
            [[15.031_25, 59.921_875]],
            "{id}"
        );
    }
}
