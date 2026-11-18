use pageframe::{App, Page};
use pageframe_test_harness::Harness;

const SPANS: &str = "<div lang=ja style=\"font-family:'Yu Gothic'\"><span id=j1>漢」。漢</span> \
     <span id=j2>漢（「漢</span> <span id=j3>」</span> <span id=j4>。</span> \
     <span id=j5>漢、」漢</span> <span id=j6>「漢</span></div>\
     <div lang=zh-CN style=\"font-family:'Microsoft YaHei'\"><span id=z1>汉”。汉</span> \
     <span id=z2>汉（“汉</span> <span id=z5>汉，”汉</span></div>\
     <div lang=ja style=\"font-family:'MS Gothic'\"><span id=m1>漢」。漢</span> \
     <span id=m2>漢（「漢</span></div>";

const CHROME: [(&str, f64); 11] = [
    ("j1", 56.0),
    ("j2", 56.0),
    ("j3", 16.0),
    ("j4", 16.0),
    ("j5", 56.0),
    ("j6", 32.0),
    ("z1", 64.0),
    ("z2", 64.0),
    ("z5", 64.0),
    ("m1", 64.0),
    ("m2", 64.0),
];

fn page(rules: &str) -> Harness<Page<()>> {
    let html = format!(
        "<!doctype html><html><head><meta charset=utf-8><style>html,body{{margin:0;\
         background:#fff;font-size:16px}}div{{margin:4px;white-space:nowrap}}{rules}\
         </style></head><body>{SPANS}</body></html>"
    );
    let page = App::new((), &html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness
}

fn rect(harness: &Harness<Page<()>>, id: &str) -> [f64; 4] {
    let doc = harness.base();
    let node_id = doc
        .query_selector_all(&format!("#{id}"))
        .expect("span selector is valid")[0];
    let rect = doc
        .get_client_bounding_rect(node_id)
        .expect("span has a rectangle");
    drop(doc);
    [rect.x, rect.y, rect.width, rect.height]
}

#[test]
fn adjacent_cjk_punctuation_is_trimmed_like_chrome() {
    let harness = page("");
    for (id, chrome) in CHROME {
        let [_, _, width, _] = rect(&harness, id);
        assert!(
            (width - chrome).abs() < 0.5,
            "{id}: {width}, Chrome {chrome}"
        );
    }
}

#[test]
fn vertical_cjk_punctuation_is_trimmed_like_chrome() {
    let harness =
        page("div{writing-mode:vertical-rl;float:left;height:380px}span{display:inline-block}");
    for (id, chrome) in CHROME {
        let [_, _, _, height] = rect(&harness, id);
        assert!(
            (height - chrome).abs() < 0.5,
            "{id}: {height}, Chrome {chrome}"
        );
    }
}
