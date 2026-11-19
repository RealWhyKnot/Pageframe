use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use pageframe_traits::shell::Viewport;

const ROWS: &str = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}\
div{height:24px}</style>\
<div style=\"padding:5.5px 0 0 10.5px\"><span id=s>cdef</span></div>\
<div style=\"padding:5.6px 0 0 10.6px\"><span id=t>cdef</span>gh</div>\
<div style=\"padding:5.5px 0 0 10.5px\"><span id=p style=\"padding:0 2.3px\">cdef</span></div>\
<div style=\"padding:5.3px 0 0 10.3px\"><span id=n style=\"margin:0 3.3px\">cdef</span>gh</div>\
<div style=\"padding:5.5px 0 0 10.5px\"><span id=u style=\"position:relative\">cdef</span></div>";

const TURNED_ROWS: &str = "<!doctype html><style>html,body{margin:0;background:#fff;\
font:14px Arial}div{height:24px;width:120px}</style>\
<div style=\"padding:5.3px 0 0 10.3px\"><span id=v style=\"margin:4.3px 0\">cdef</span>gh</div>\
<div style=\"padding:5.5px 0 0 10.5px;direction:rtl\"><span id=w style=\"padding:0 2.3px\">cdef\
</span>gh</div><div style=\"padding:10.5px 0 0 5.5px;writing-mode:vertical-lr;height:70px;\
width:auto\"><span id=k style=\"padding:2.3px 0\">cdef</span>gh</div><div style=\"padding:10.3px \
0 0 5.3px;writing-mode:vertical-rl;height:70px;width:auto\"><span id=e>cdef</span>gh</div>";

struct Edges {
    id: &'static str,
    left: i16,
    right: i16,
    top: i16,
    bottom: i16,
    middle: (i16, i16),
}

const fn edges(id: &'static str, sides: [i16; 4], middle: (i16, i16)) -> Edges {
    Edges {
        id,
        left: sides[0],
        right: sides[1],
        top: sides[2],
        bottom: sides[3],
        middle,
    }
}

const AT_1X: [Edges; 5] = [
    edges("s", [609, 2367, 289, 1375], (1519, 864)),
    edges("t", [615, 2304, 2183, 3269], (1525, 2758)),
    edges("p", [641, 2659, 4097, 5157], (1666, 4646)),
    edges("n", [833, 2559, 5953, 7032], (1717, 6521)),
    edges("u", [641, 2367, 7809, 8895], (1519, 8409)),
];

const AT_2X: [Edges; 5] = [
    edges("s", [641, 2367, 321, 1343], (1519, 848)),
    edges("t", [647, 2336, 2215, 3237], (1525, 2742)),
    edges("p", [641, 2655, 4103, 5119], (1666, 4630)),
    edges("n", [833, 2559, 5985, 7007], (1717, 6505)),
    edges("u", [641, 2367, 7873, 8895], (1519, 8393)),
];

const TURNED_AT_1X: [Edges; 4] = [
    edges("v", [596, 2304, 276, 1343], (1506, 851)),
    edges("w", [5313, 7296, 2177, 3250], (6361, 2739)),
    edges("k", [321, 1375, 4372, 6336], (864, 5429)),
    edges("e", [276, 1362, 9511, 11200], (851, 10421)),
];

const TURNED_AT_2X: [Edges; 4] = [
    edges("v", [641, 2336, 321, 1343], (1506, 835)),
    edges("w", [5345, 7328, 2209, 3231], (6361, 2723)),
    edges("k", [353, 1375, 4417, 6400], (880, 5429)),
    edges("e", [340, 1362, 9543, 11232], (867, 10421)),
];

fn open(page: &str, scale: f32) -> Harness<Page<()>> {
    let page = App::new((), page)
        .page(160, 200)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    {
        let mut doc = harness.base_mut();
        let color_scheme = doc.get_viewport().color_scheme;
        let size = |css: u16| {
            num_traits::ToPrimitive::to_u32(&(f32::from(css) * scale))
                .expect("test viewports fit in u32")
        };
        doc.set_viewport(Viewport::new(size(160), size(200), scale, color_scheme));
    }
    harness.pump();
    harness
}

fn hit_id(harness: &Harness<Page<()>>, x: f32, y: f32) -> Option<String> {
    let doc = harness.base();
    let hit = doc.hit(x, y)?;
    let mut node = doc.get_node(hit.node_id)?;
    loop {
        if let Some(id) = node.element_data().and_then(|element| element.id.as_ref()) {
            return Some(id.to_string());
        }
        node = doc.get_node(node.parent?)?;
    }
}

fn units(value: i16) -> f32 {
    f32::from(value) / 64.0
}

fn check(page: &str, scale: f32, rows: &[Edges]) {
    let harness = open(page, scale);
    for row in rows {
        let (x, y) = (units(row.middle.0), units(row.middle.1));
        let points = [
            ("left", units(row.left - 1), y, false),
            ("left", units(row.left), y, true),
            ("right", units(row.right), y, true),
            ("right", units(row.right + 1), y, false),
            ("top", x, units(row.top - 1), false),
            ("top", x, units(row.top), true),
            ("bottom", x, units(row.bottom), true),
            ("bottom", x, units(row.bottom + 1), false),
        ];
        for (side, px, py, inside) in points {
            let hit = hit_id(&harness, px, py);
            assert_eq!(
                hit.as_deref() == Some(row.id),
                inside,
                "{} {side} at ({px}, {py}) {scale}x hit {hit:?}",
                row.id
            );
        }
    }
}

#[test]
fn text_and_spans_hit_at_their_pixel_snapped_edges_like_chrome() {
    check(ROWS, 1.0, &AT_1X);
}

#[test]
fn text_and_spans_hit_at_their_pixel_snapped_edges_at_2x_like_chrome() {
    check(ROWS, 2.0, &AT_2X);
}

#[test]
fn rtl_and_vertical_text_hit_at_pixel_snapped_edges_like_chrome() {
    check(TURNED_ROWS, 1.0, &TURNED_AT_1X);
    check(TURNED_ROWS, 2.0, &TURNED_AT_2X);
}
