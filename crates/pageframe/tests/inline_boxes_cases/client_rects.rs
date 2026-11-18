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
