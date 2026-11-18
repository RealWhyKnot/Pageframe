#![cfg(target_os = "windows")]

use pageframe::App;
use pageframe_test_harness::Harness;

fn heights(texts: &[&str]) -> Vec<f64> {
    heights_in(&vec![""; texts.len()], texts)
}

fn heights_in(langs: &[&str], texts: &[&str]) -> Vec<f64> {
    assert_eq!(langs.len(), texts.len());
    let rows: String = texts
        .iter()
        .zip(langs)
        .enumerate()
        .map(|(index, (text, lang))| format!("<div id=t{index} lang=\"{lang}\">{text}</div>"))
        .collect();
    let html = format!(
        "<!doctype html><style>html,body{{margin:0;font:16px Arial}}\
         div{{border:1px solid #999;margin:3px;width:120px}}</style>{rows}"
    );
    let page = App::new((), &html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let doc = harness.base();
    (0..texts.len())
        .map(|index| {
            let node_id = doc
                .query_selector_all(&format!("#t{index}"))
                .expect("row selector is valid")[0];
            doc.get_client_bounding_rect(node_id)
                .expect("row has a rectangle")
                .height
        })
        .collect()
}

#[test]
fn japanese_text_falls_back_to_the_fonts_chrome_uses() {
    let texts = [
        "日本語、「かな」。",
        "日本語日本語日本語",
        "日本語日本語かなかなかな",
        "日本語日本語日かな」。",
    ];
    assert_eq!(heights(&texts), [54.0, 44.0, 54.0, 49.0]);
}

#[test]
fn han_falls_back_by_the_page_language_like_chrome() {
    let langs = [
        "ja", "ko", "zh-TW", "zh-HK", "zh-MO", "zh-SG", "zh-CN", "zh-Hant", "en",
    ];
    let texts = ["漢字漢字"; 9];
    assert_eq!(
        heights_in(&langs, &texts),
        [28.0, 23.0, 23.0, 23.0, 23.0, 23.0, 23.0, 23.0, 23.0]
    );
}
