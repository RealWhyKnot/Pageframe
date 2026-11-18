#![cfg(target_os = "windows")]

use pageframe::App;
use pageframe_test_harness::Harness;

#[test]
fn clicks_in_centred_text_find_the_line_under_them_like_chrome() {
    let html = "<html><head><style>html,body{margin:0;font:16px/20px Arial}\
                div{height:100px;width:200px;align-content:center}</style></head>\
                <body><div id=\"d\">aaa<br>bbb</div></body></html>";
    let page = App::new((), html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let doc = harness.base();
    let root = doc.query_selector_all("#d").expect("selector is valid")[0];
    for (x, y, offset) in [
        (10.0, 40.0, 1),
        (10.0, 55.0, 5),
        (10.0, 65.0, 5),
        (20.0, 55.0, 6),
    ] {
        assert_eq!(
            doc.find_text_position(x, y),
            Some((root, offset)),
            "{x},{y}"
        );
    }
}
