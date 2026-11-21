use pageframe::{App, Page};
use pageframe_test_harness::Harness;

const STYLE: &str = "html,body{margin:0;font:14px Arial}td,th{padding:3px}";

const BODY: &str = "<div style=\"margin:10px 0 0 7px\"><table id=t1 style=\"border:4px solid;\
border-spacing:5px 6px\"><caption>cap</caption><thead id=g1><tr id=r1><th>h</th><th>h2</th>\
</tr></thead><tbody id=g2><tr id=r2><td>a</td><td>b</td></tr><tr id=r3 style=\"height:40px\">\
<td>c</td><td>d</td></tr></tbody></table></div><div id=w2 style=\"position:relative;padding:5px\">\
<table style=\"border-collapse:collapse;border:6px solid\"><tr><td style=\"border:2px solid\">e</td>\
</tr><tr id=r5 style=\"position:relative;top:3px\"><td id=c5>f</td></tr></table></div>\
<table id=t3 style=\"position:relative;margin-left:20px\"><tbody id=g3 style=\"position:relative\">\
<tr id=r6><td id=c6>g</td></tr></tbody></table>";

fn page() -> Harness<Page<()>> {
    let html = format!(
        "<!doctype html><html><head><style>{STYLE}</style></head><body>{BODY}</body></html>"
    );
    let page = App::new((), &html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness
}

struct Geometry {
    client: [f64; 4],
    offset: [f64; 4],
    parent: Option<String>,
}

fn geometry(harness: &Harness<Page<()>>, id: &str) -> Geometry {
    let doc = harness.base();
    let node_id = doc
        .query_selector_all(&format!("#{id}"))
        .expect("selector is valid")[0];
    let client = doc
        .get_client_bounding_rect(node_id)
        .expect("part has a client rect");
    let offset = doc.offset_rect(node_id).expect("part has an offset rect");
    let parent = doc
        .get_node(node_id)
        .and_then(|node| node.offset_parent())
        .and_then(|parent| parent.element_data())
        .and_then(|element| element.id.as_ref())
        .map(ToString::to_string);
    drop(doc);
    Geometry {
        client: [client.x, client.y, client.width, client.height],
        offset: [offset.x, offset.y, offset.width, offset.height].map(f64::round),
        parent,
    }
}

#[test]
fn rows_and_groups_span_the_columns_of_their_tracks() {
    let harness = page();
    for (id, client, offset) in [
        ("g1", [16.0, 36.0, 41.90625, 22.0], [5.0, 22.0, 42.0, 22.0]),
        ("r1", [16.0, 36.0, 41.90625, 22.0], [5.0, 22.0, 42.0, 22.0]),
        ("g2", [16.0, 64.0, 41.90625, 68.0], [5.0, 50.0, 42.0, 68.0]),
        ("r2", [16.0, 64.0, 41.90625, 22.0], [5.0, 50.0, 42.0, 22.0]),
        ("r3", [16.0, 92.0, 41.90625, 40.0], [5.0, 78.0, 42.0, 40.0]),
    ] {
        let found = geometry(&harness, id);
        assert_eq!(found.client, client, "{id} client rect");
        assert_eq!(found.offset, offset, "{id} offset rect");
        assert_eq!(found.parent.as_deref(), Some("t1"), "{id} offsetParent");
    }
}

#[test]
fn positioned_rows_and_groups_become_offset_parents() {
    let harness = page();
    for (id, client, offset, parent) in [
        (
            "r5",
            [8.0, 179.0, 19.796875, 26.0],
            [8.0, 37.0, 20.0, 26.0],
            "w2",
        ),
        (
            "c5",
            [8.0, 179.0, 19.796875, 26.0],
            [0.0, 0.0, 20.0, 26.0],
            "r5",
        ),
        (
            "g3",
            [22.0, 212.0, 13.796875, 22.0],
            [2.0, 2.0, 14.0, 22.0],
            "t3",
        ),
        (
            "r6",
            [22.0, 212.0, 13.796875, 22.0],
            [0.0, 0.0, 14.0, 22.0],
            "g3",
        ),
        (
            "c6",
            [22.0, 212.0, 13.796875, 22.0],
            [0.0, 0.0, 14.0, 22.0],
            "g3",
        ),
    ] {
        let found = geometry(&harness, id);
        assert_eq!(found.client, client, "{id} client rect");
        assert_eq!(found.offset, offset, "{id} offset rect");
        assert_eq!(found.parent.as_deref(), Some(parent), "{id} offsetParent");
    }
}
