#![cfg(target_os = "windows")]

use std::fmt::Write as _;

use pageframe::{App, Page};
use pageframe_dom::BaseDocument;
use pageframe_test_harness::Harness;
use skrifa::string::StringId;
use skrifa::{FontRef, MetadataProvider};

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
    glyph_font_and_advance(&harness.base(), "#sample").1
}

fn glyph_font_and_advance(doc: &BaseDocument, selector: &str) -> (String, f32) {
    let sample = doc
        .query_selector(selector)
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
    let run = layout
        .get(0)
        .expect("sample has a line")
        .runs()
        .next()
        .expect("sample has a glyph run");
    let data = run.font();
    let font = FontRef::from_index(data.data.as_ref(), data.index).expect("valid font data");
    let name = font
        .localized_strings(StringId::POSTSCRIPT_NAME)
        .english_or_first()
        .expect("font has a PostScript name")
        .to_string();
    (name, run.advance())
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

type GlyphSample = (&'static str, f32);
type FamilySample = [GlyphSample; 3];

fn samples(face: &'static str, advances: [f32; 3]) -> FamilySample {
    advances.map(|advance| (face, advance))
}

fn check_locale_families(lang: &str, native: char, expected: [FamilySample; 4]) {
    let families = ["", "monospace", "serif", "sans-serif"];
    let glyphs = ['a', native, ' '];
    let lang_attr = format!("lang='{lang}'");
    for (placement, root_attr, elem_attr) in [
        ("html", lang_attr.as_str(), ""),
        ("div", "", lang_attr.as_str()),
    ] {
        let mut html = format!(
            "<!doctype html><html {root_attr}><style>html,body{{margin:0;font-size:10px}}\
             div{{white-space:pre}}</style>"
        );
        for (family_idx, family) in families.into_iter().enumerate() {
            let style = if family.is_empty() {
                String::new()
            } else {
                format!("style='font-family:{family}'")
            };
            for (glyph_idx, glyph) in glyphs.into_iter().enumerate() {
                write!(
                    html,
                    "<div id=f{family_idx}g{glyph_idx} {elem_attr} {style}>{glyph}</div>"
                )
                .expect("write to String");
            }
        }
        let harness = open(&html);
        let doc = harness.base();
        let measured: [[_; 3]; 4] = std::array::from_fn(|family_idx| {
            std::array::from_fn(|glyph_idx| {
                glyph_font_and_advance(&doc, &format!("#f{family_idx}g{glyph_idx}"))
            })
        });
        drop(doc);

        for (family_idx, family) in families.into_iter().enumerate() {
            for (glyph_idx, glyph) in glyphs.into_iter().enumerate() {
                let (font, advance) = &measured[family_idx][glyph_idx];
                assert_eq!(
                    (font.as_str(), *advance),
                    expected[family_idx][glyph_idx],
                    "lang={lang} on {placement}, {family:?}, {glyph:?}"
                );
            }
        }
    }
}

#[test]
fn locale_fonts_match_chrome_for_japanese() {
    let yu_gothic = samples("YuGothic-Regular", [5.593_75, 10.0, 2.859_375]);
    let ms_gothic = samples("MS-Gothic", [5.0, 10.0, 5.0]);
    check_locale_families("ja", 'あ', [yu_gothic, ms_gothic, yu_gothic, yu_gothic]);
}

#[test]
fn locale_fonts_match_chrome_for_korean() {
    let malgun = samples("MalgunGothic", [5.203_125, 10.0, 3.515_625]);
    check_locale_families("ko", '한', [malgun; 4]);
}

#[test]
fn locale_fonts_match_chrome_for_simplified_chinese() {
    let yahei = samples("MicrosoftYaHei", [5.531_25, 10.0, 2.968_75]);
    let simsun = samples("SimSun", [5.0, 10.0, 5.0]);
    let nsimsun = samples("NSimSun", [5.0, 10.0, 5.0]);
    check_locale_families("zh-CN", '汉', [yahei, nsimsun, simsun, yahei]);
}

#[test]
fn locale_fonts_match_chrome_for_traditional_chinese() {
    let jhenghei = samples("MicrosoftJhengHeiRegular", [5.484_375, 10.0, 2.5]);
    for lang in ["zh-TW", "zh-HK"] {
        check_locale_families(lang, '漢', [jhenghei; 4]);
    }
}

#[test]
fn locale_fonts_match_chrome_for_thai() {
    let thai = ("Tahoma", 5.953_125);
    let times = [
        ("TimesNewRomanPSMT", 4.453_125),
        thai,
        ("TimesNewRomanPSMT", 2.5),
    ];
    let consolas = [("Consolas", 5.5), thai, ("Consolas", 5.5)];
    let arial = [("ArialMT", 5.562_5), thai, ("ArialMT", 2.781_25)];
    check_locale_families("th", 'ก', [times, consolas, times, arial]);
}

#[test]
fn locale_fonts_match_chrome_for_devanagari() {
    let nirmala = samples("NirmalaUI", [5.093_75, 8.640_625, 2.75]);
    let consolas = [("Consolas", 5.5), nirmala[1], ("Consolas", 5.5)];
    check_locale_families("hi", 'क', [nirmala, consolas, nirmala, nirmala]);
}

#[test]
fn locale_fonts_match_chrome_for_cyrillic() {
    let times = samples("TimesNewRomanPSMT", [4.453_125, 6.671_875, 2.5]);
    let courier = samples("CourierNewPSMT", [6.015_625; 3]);
    let arial = samples("ArialMT", [5.562_5, 7.234_375, 2.781_25]);
    check_locale_families("ru", 'Я', [times, courier, times, arial]);
}

#[test]
fn locale_fonts_match_chrome_for_greek() {
    let times = samples("TimesNewRomanPSMT", [4.453_125, 7.437_5, 2.5]);
    let courier = samples("CourierNewPSMT", [6.015_625; 3]);
    let arial = samples("ArialMT", [5.562_5, 7.484_375, 2.781_25]);
    check_locale_families("el", 'Ω', [times, courier, times, arial]);
}

fn check_locale_font_samples(cases: &[(&str, GlyphSample, GlyphSample)]) {
    let mut html = "<!doctype html><style>body{font-size:10px}</style>".to_owned();
    for (index, (lang, _, _)) in cases.iter().enumerate() {
        write!(
            html,
            "<div id=d{index} lang='{lang}'>a</div>\
             <div id=m{index} lang='{lang}' style='font-family:monospace'>a</div>"
        )
        .expect("write to String");
    }

    let harness = open(&html);
    let doc = harness.base();
    let measured: Vec<_> = (0..cases.len())
        .map(|index| {
            [
                glyph_font_and_advance(&doc, &format!("#d{index}")),
                glyph_font_and_advance(&doc, &format!("#m{index}")),
            ]
        })
        .collect();
    drop(doc);

    for (&(lang, default, monospace), samples) in cases.iter().zip(measured) {
        for ((face, advance), (family, expected)) in samples
            .into_iter()
            .zip([("default", default), ("monospace", monospace)])
        {
            assert_eq!((face.as_str(), advance), expected, "lang={lang}, {family}");
        }
    }
}

#[test]
fn cyrillic_locale_defaults_match_chrome() {
    let times = ("TimesNewRomanPSMT", 4.453_125);
    let courier = ("CourierNewPSMT", 6.015_625);
    let consolas = ("Consolas", 5.5);
    check_locale_font_samples(&[
        ("uk", times, courier),
        ("bg", times, courier),
        ("sr", times, courier),
        ("sr-Latn", times, consolas),
        ("mn", times, courier),
        ("mn-Mong", times, consolas),
        ("sr-Zzzz-RS", times, courier),
    ]);
}

#[test]
fn devanagari_locale_defaults_match_chrome() {
    let nirmala = ("NirmalaUI", 5.093_75);
    let consolas = ("Consolas", 5.5);
    check_locale_font_samples(&[
        ("mr", nirmala, consolas),
        ("ne", nirmala, consolas),
        ("sa", nirmala, consolas),
    ]);
}

#[test]
fn arabic_locale_defaults_match_chrome() {
    let times = ("TimesNewRomanPSMT", 4.453_125);
    let courier = ("CourierNewPSMT", 6.015_625);
    let consolas = ("Consolas", 5.5);
    check_locale_font_samples(&[
        ("ps", times, courier),
        ("sd", times, courier),
        ("ug", times, courier),
        ("ks", times, courier),
        ("ar-Aran", times, courier),
        ("und-Aran", times, consolas),
    ]);
}

#[test]
fn chinese_locale_defaults_follow_chromes_subtag_order() {
    let yahei = ("MicrosoftYaHei", 5.531_25);
    let jhenghei = ("MicrosoftJhengHeiRegular", 5.484_375);
    let nsimsun = ("NSimSun", 5.0);
    let times = ("TimesNewRomanPSMT", 4.453_125);
    let consolas = ("Consolas", 5.5);
    check_locale_font_samples(&[
        ("zh-SG", yahei, nsimsun),
        ("zh-MO", jhenghei, jhenghei),
        ("zh-Hans-MO", yahei, nsimsun),
        ("zh-Hant-SG", jhenghei, jhenghei),
        ("zh-Latn-MO", times, consolas),
        ("zh-Bopo-MO", times, consolas),
        ("zh-Zzzz-MO", yahei, nsimsun),
        ("ZH_mO", jhenghei, jhenghei),
        ("zh-yue", jhenghei, jhenghei),
        ("zh-yue-Hans", yahei, nsimsun),
        ("zh-nan-TW", jhenghei, jhenghei),
        ("yue-SG", jhenghei, jhenghei),
        ("cmn-TW", yahei, nsimsun),
    ]);
}
