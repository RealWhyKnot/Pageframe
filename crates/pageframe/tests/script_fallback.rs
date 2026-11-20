#![cfg(target_os = "windows")]

use pageframe::{App, Page};
use pageframe_test_harness::Harness;

const BASE: &str = "html,body{margin:0;background:white;font:10px/10px monospace;color:black}";

fn open(html: &str) -> Harness<Page<()>> {
    let page = App::new((), html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness
}

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
    let harness = open(&html);
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
fn kana_falls_back_by_the_page_language_like_chrome() {
    let langs = ["ko", "zh-CN", "zh-TW", "zh-HK", "ja", "en", ""];
    let texts = ["かなカナ"; 7];
    assert_eq!(
        heights_in(&langs, &texts),
        [23.0, 23.0, 23.0, 23.0, 28.0, 28.0, 28.0]
    );
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

fn glyph_advance(root_attr: &str, attr: &str, family: &str, glyph: char) -> f32 {
    let html = format!(
        "<!doctype html><html {root_attr}><style>{BASE}</style>\
         <div {attr} id=sample style='font-family:{family};white-space:pre'>{glyph}</div>"
    );
    let harness = open(&html);
    let doc = harness.base();
    let sample = doc
        .query_selector("#sample")
        .expect("valid selector")
        .expect("sample exists");
    let layout = &doc
        .get_node(sample)
        .expect("sample node exists")
        .element_data()
        .expect("sample is an element")
        .inline_layout_data
        .as_ref()
        .expect("sample has an inline layout")
        .layout;
    let advance = layout
        .get(0)
        .expect("sample has a line")
        .runs()
        .next()
        .expect("sample has a glyph run")
        .advance();
    drop(doc);
    advance
}

#[test]
fn hebrew_monospace_uses_chromes_standard_font_fallback() {
    for (family, glyph, expected) in [
        ("monospace", 'א', 4.828_125),
        ("Consolas", 'א', 4.828_125),
        ("monospace", ' ', 5.5),
        ("monospace", 'a', 5.5),
        ("serif", 'א', 4.828_125),
        ("sans-serif", 'א', 5.640_625),
        ("monospace, Arial", 'א', 5.640_625),
    ] {
        assert_eq!(
            glyph_advance("", "", family, glyph),
            expected,
            "{family}, {glyph:?}"
        );
    }
}

#[test]
fn arabic_monospace_uses_chromes_standard_font_fallback() {
    assert_eq!(glyph_advance("", "", "monospace", 'ب'), 7.140_625);
}

#[test]
fn padded_hebrew_monospace_wraps_like_chrome() {
    let html = format!(
        "<!doctype html><style>{BASE}\
         div{{width:100px;line-height:20px;margin:0 0 10px 200px}}\
         span{{box-decoration-break:clone}}</style>\
         <div dir=ltr>a <span id=right style='padding:0 30px 0 4px'>אאאא אאאא אאאא אאאא</span></div>\
         <div dir=ltr>a <span id=left style='padding:0 4px 0 30px'>אאאא אאאא אאאא אאאא</span></div>"
    );
    let harness = open(&html);
    let doc = harness.base();
    let extents = ["#right", "#left"].map(|selector| {
        let span = doc
            .query_selector(selector)
            .expect("valid selector")
            .expect("span exists");
        doc.inline_fragment_rects(span)
            .expect("span has line fragments")
            .map(|rect| [rect.x, rect.x + rect.width])
            .collect::<Vec<_>>()
    });
    drop(doc);
    assert_eq!(
        extents,
        [
            [[211.0, 313.781_25], [200.0, 253.265_625]],
            [[211.0, 289.015_625], [200.0, 278.015_625]],
        ]
    );
}

#[test]
fn hebrew_lang_keeps_chromes_font_advances() {
    for (family, expected) in [
        ("monospace", 4.828_125),
        ("serif", 4.828_125),
        ("sans-serif", 5.640_625),
    ] {
        assert_eq!(
            glyph_advance("", "lang=he", family, 'א'),
            expected,
            "element lang=he, {family}"
        );
        assert_eq!(
            glyph_advance("lang=he", "", family, 'א'),
            expected,
            "root lang=he, {family}"
        );
    }
}

#[test]
fn arabic_script_locales_select_chromes_generic_families() {
    for lang in ["lang=ar", "lang=fa", "lang=ur"] {
        for (family, glyph, expected) in [
            ("monospace", 'ب', 6.015_625),
            ("serif", 'ب', 7.140_625),
            ("sans-serif", 'ب', 9.187_5),
            ("monospace", 'a', 6.015_625),
            ("sans-serif", 'a', 5.093_75),
            ("monospace, Arial", 'ب', 6.015_625),
            ("Arial, monospace", 'a', 5.562_5),
            ("Consolas", 'ب', 7.140_625),
        ] {
            assert_eq!(
                glyph_advance("", lang, family, glyph),
                expected,
                "{lang}, {family}, {glyph:?}"
            );
        }
    }
}

#[test]
fn arabic_script_tags_select_chromes_monospace_metrics() {
    let html = format!(
        "<!doctype html><style>{BASE} div{{width:1ch}}</style>\
         <div lang=ar></div><div lang=und-Arab></div>\
         <div lang=fa-Latn></div><div></div>"
    );
    let harness = open(&html);
    let doc = harness.base();
    let widths: Vec<_> = doc
        .query_selector_all("div")
        .expect("valid selector")
        .into_iter()
        .map(|node| {
            doc.get_client_bounding_rect(node)
                .expect("element has a rectangle")
                .width
        })
        .collect();
    drop(doc);
    assert_eq!(widths, [6.0, 6.0, 5.484_375, 5.484_375]);
}
