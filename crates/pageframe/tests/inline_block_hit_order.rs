use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use pageframe_traits::shell::Viewport;

const PAGE: &str = "<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}\
div{height:30px}b{display:inline-block;width:20.3px;height:8px;background:#0f0}</style>\
<div id=d4>ab<b id=b4 style=\"height:auto\">xy</b><span id=t4>cd</span></div>\
<div id=d5>ab<img id=i5 style=\"width:20.3px;height:10px;background:#00f\"><span id=t5>cd</span>\
</div><div id=d6>ab<span id=g6 style=\"background:#ff0\">c</span><b id=b6></b><span id=t6>de\
</span></div><div id=d7>ab<b id=b7></b><span id=p7 style=\"position:relative\">de</span></div>\
<div id=d8>ab<span id=g8 style=\"background:#ff0\">c<b id=b8></b>d</span>e</div>\
<div id=d9>ab<b id=b9 style=\"position:relative\"></b><span id=t9>de</span></div>";

struct Row {
    id: &'static str,
    y: f32,
    changes: &'static [(i16, &'static str)],
}

const AT_1X: [Row; 6] = [
    Row {
        id: "d4",
        y: 8.0,
        changes: &[(934, "b4"), (2233, "t4")],
    },
    Row {
        id: "d5",
        y: 38.0,
        changes: &[(934, "i5"), (2233, "t5")],
    },
    Row {
        id: "d6",
        y: 69.0,
        changes: &[(961, "g6"), (1382, "b6"), (2681, "t6")],
    },
    Row {
        id: "d7",
        y: 99.0,
        changes: &[(934, "b7"), (2241, "p7"), (3264, "d7")],
    },
    Row {
        id: "d8",
        y: 129.0,
        changes: &[(961, "g8"), (1382, "b8"), (2689, "g8"), (3201, "d8")],
    },
    Row {
        id: "d9",
        y: 159.0,
        changes: &[(934, "b9"), (2296, "t9"), (3293, "d9")],
    },
];

const AT_2X: [Row; 6] = [
    Row {
        id: "d4",
        y: 8.0,
        changes: &[(966, "b4"), (2265, "t4"), (3243, "d4")],
    },
    Row {
        id: "d5",
        y: 37.5,
        changes: &[(966, "i5"), (2265, "t5"), (3243, "d5")],
    },
    Row {
        id: "d6",
        y: 68.5,
        changes: &[(961, "g6"), (1414, "b6"), (2713, "t6")],
    },
    Row {
        id: "d7",
        y: 98.5,
        changes: &[(966, "b7"), (2273, "p7"), (3296, "d7")],
    },
    Row {
        id: "d8",
        y: 128.5,
        changes: &[(961, "g8"), (1414, "b8"), (2721, "g8"), (3201, "d8")],
    },
    Row {
        id: "d9",
        y: 158.5,
        changes: &[(966, "b9"), (2296, "t9")],
    },
];

fn open(scale: f32) -> Harness<Page<()>> {
    let page = App::new((), PAGE)
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

fn check(scale: f32, rows: &[Row]) {
    let harness = open(scale);
    for row in rows {
        let mut before = row.id;
        for &(at, id) in row.changes {
            for (x, want) in [(at - 1, before), (at, id)] {
                let px = f32::from(x) / 64.0;
                let hit = hit_id(&harness, px, row.y);
                assert_eq!(
                    hit.as_deref(),
                    Some(want),
                    "{} at ({px}, {}) {scale}x",
                    row.id,
                    row.y
                );
            }
            before = id;
        }
    }
}

#[test]
fn later_text_hits_above_an_earlier_inline_block_like_chrome() {
    check(1.0, &AT_1X);
}

#[test]
fn later_text_hits_above_an_earlier_inline_block_at_2x_like_chrome() {
    check(2.0, &AT_2X);
}
