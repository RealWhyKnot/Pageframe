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
    </body></html>";

fn vertical_extents(scale: f32) -> Vec<[f64; 2]> {
    let mut harness = open(App::new((), SPANS));
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
    [
        "plain", "padded", "outer", "inner", "nest", "bold", "wrap", "sub", "sup",
    ]
    .map(|id| {
        let node = doc
            .query_selector(&format!("#{id}"))
            .expect("valid selector")
            .expect("the span exists");
        let rect = doc
            .get_client_bounding_rect(node)
            .expect("the span is laid out");
        [rect.y, rect.height]
    })
    .to_vec()
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
    ];
    assert_eq!(vertical_extents(2.0), chrome);
}
