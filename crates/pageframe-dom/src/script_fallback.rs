use parley::fontique::{Collection, FallbackKey, GenericFamily, Language, Script};

const JAPANESE: &[&str] = &[
    "Noto Sans JP",
    "Noto Sans CJK JP",
    "Meiryo",
    "Yu Gothic",
    "MS PGothic",
    "Microsoft YaHei",
];

const SIMPLIFIED_HAN: &[&str] = &[
    "Noto Sans SC",
    "Noto Sans CJK SC",
    "Microsoft YaHei",
    "simsun",
];

const TRADITIONAL_HAN: &[&str] = &[
    "Noto Sans TC",
    "Noto Sans CJK TC",
    "Microsoft JhengHei",
    "pmingliu",
];

const KOREAN: &[&str] = &["Noto Sans KR", "Noto Sans CJK KR", "Malgun Gothic", "Gulim"];

const SCRIPT_FAMILIES: &[([u8; 4], Option<&str>, &[&str])] = &[
    (*b"Arab", None, &["Times New Roman"]),
    (*b"Arab", Some("fa"), &["Times New Roman"]),
    (*b"Arab", Some("ur"), &["Times New Roman"]),
    (*b"Hebr", None, &["Times New Roman"]),
    (*b"Thai", None, &["Tahoma"]),
    (*b"Hira", None, JAPANESE),
    (*b"Kana", None, JAPANESE),
    (*b"Hani", None, SIMPLIFIED_HAN),
    (*b"Hani", Some("ja"), JAPANESE),
    (*b"Hani", Some("ko"), KOREAN),
    (*b"Hani", Some("zh-TW"), TRADITIONAL_HAN),
    (*b"Hani", Some("zh-HK"), TRADITIONAL_HAN),
    (*b"Hani", Some("zh-MO"), TRADITIONAL_HAN),
    (*b"Hani", Some("zh-SG"), SIMPLIFIED_HAN),
];

const LOCALE_SCRIPTS: &[(&str, &str)] = &[
    ("ab", "Cyrl"),
    ("ady", "Cyrl"),
    ("aeb", "Arab"),
    ("ar", "Arab"),
    ("arq", "Arab"),
    ("ary", "Arab"),
    ("arz", "Arab"),
    ("av", "Cyrl"),
    ("azb", "Arab"),
    ("ba", "Cyrl"),
    ("bal", "Arab"),
    ("be", "Cyrl"),
    ("bej", "Arab"),
    ("bg", "Cyrl"),
    ("bqi", "Arab"),
    ("brh", "Arab"),
    ("cdo", "Hans"),
    ("ce", "Cyrl"),
    ("cja", "Arab"),
    ("cjm", "Arab"),
    ("cjy", "Hans"),
    ("ckb", "Arab"),
    ("cmn", "Hans"),
    ("cpx", "Hans"),
    ("czh", "Hans"),
    ("czo", "Hans"),
    ("dcc", "Arab"),
    ("doi", "Arab"),
    ("dyo", "Arab"),
    ("el", "Grek"),
    ("fa", "Arab"),
    ("gan", "Hans"),
    ("gba", "Arab"),
    ("gbz", "Arab"),
    ("gjk", "Arab"),
    ("gju", "Arab"),
    ("glk", "Arab"),
    ("hak", "Hant"),
    ("haz", "Arab"),
    ("hi", "Deva"),
    ("hnd", "Arab"),
    ("hno", "Arab"),
    ("hsn", "Hans"),
    ("inh", "Cyrl"),
    ("ja", "Jpan"),
    ("kbd", "Cyrl"),
    ("khw", "Arab"),
    ("kk", "Cyrl"),
    ("ko", "Hang"),
    ("kok", "Deva"),
    ("krc", "Cyrl"),
    ("ks", "Arab"),
    ("ku", "Arab"),
    ("kum", "Cyrl"),
    ("kvx", "Arab"),
    ("kxp", "Arab"),
    ("ky", "Cyrl"),
    ("lah", "Arab"),
    ("lez", "Cyrl"),
    ("lki", "Arab"),
    ("lrc", "Arab"),
    ("luz", "Arab"),
    ("lzh", "Hant"),
    ("mai", "Deva"),
    ("mdf", "Cyrl"),
    ("mfa", "Arab"),
    ("mk", "Cyrl"),
    ("mn", "Cyrl"),
    ("mnp", "Hans"),
    ("mr", "Deva"),
    ("mvy", "Arab"),
    ("myv", "Cyrl"),
    ("mzn", "Arab"),
    ("nan", "Hant"),
    ("ne", "Deva"),
    ("os", "Cyrl"),
    ("prd", "Arab"),
    ("prs", "Arab"),
    ("ps", "Arab"),
    ("rmt", "Arab"),
    ("ru", "Cyrl"),
    ("sa", "Deva"),
    ("sah", "Cyrl"),
    ("sd", "Arab"),
    ("sdh", "Arab"),
    ("shi", "Arab"),
    ("skr", "Arab"),
    ("sr", "Cyrl"),
    ("sus", "Arab"),
    ("swb", "Arab"),
    ("syr", "Arab"),
    ("tg", "Cyrl"),
    ("tt", "Cyrl"),
    ("ttt", "Arab"),
    ("tyv", "Cyrl"),
    ("udm", "Cyrl"),
    ("ug", "Arab"),
    ("uk", "Cyrl"),
    ("ur", "Arab"),
    ("uz", "Cyrl"),
    ("wuu", "Hans"),
    ("yue", "Hant"),
    ("zdj", "Arab"),
    ("zh", "Hans"),
    ("zh-cdo", "Hans"),
    ("zh-cjy", "Hans"),
    ("zh-cmn", "Hans"),
    ("zh-cpx", "Hans"),
    ("zh-czh", "Hans"),
    ("zh-czo", "Hans"),
    ("zh-gan", "Hans"),
    ("zh-hak", "Hant"),
    ("zh-hk", "Hant"),
    ("zh-hsn", "Hans"),
    ("zh-lzh", "Hant"),
    ("zh-mnp", "Hans"),
    ("zh-mo", "Hant"),
    ("zh-nan", "Hant"),
    ("zh-tw", "Hant"),
    ("zh-wuu", "Hans"),
    ("zh-yue", "Hant"),
];

const OTHER_SCRIPT_SUBTAGS: &[&str] = &[
    "armn", "bali", "batk", "beng", "blis", "bopo", "brah", "brai", "bugi", "buhd", "cans", "cham",
    "cher", "cirt", "copt", "cprt", "cyrs", "dsrt", "egyd", "egyh", "egyp", "ethi", "geok", "geor",
    "glag", "goth", "gujr", "guru", "hani", "hano", "hebr", "hmng", "hung", "inds", "ital", "java",
    "kali", "khar", "khmr", "knda", "laoo", "latf", "latg", "latn", "lepc", "limb", "lina", "linb",
    "mand", "maya", "mero", "mlym", "mong", "mymr", "nkoo", "ogam", "orkh", "orya", "osma", "perm",
    "phag", "phnx", "plrd", "qaai", "roro", "runr", "sara", "shaw", "sinh", "sylo", "syrc", "syre",
    "syrj", "syrn", "tagb", "tale", "talu", "taml", "telu", "teng", "tfng", "tglg", "thaa", "thai",
    "tibt", "ugar", "vaii", "visp", "xpeo", "xsux", "yiii", "zxxx", "zyyy",
];

fn locale_script(mut tag: &str) -> Option<&'static str> {
    loop {
        if let Ok(index) = LOCALE_SCRIPTS.binary_search_by(|&(key, _)| {
            let tag_bytes = tag.bytes().map(|byte| match byte {
                b'_' => b'-',
                _ => byte.to_ascii_lowercase(),
            });
            key.bytes().cmp(tag_bytes)
        }) {
            return Some(LOCALE_SCRIPTS[index].1);
        }

        let (parent, subtag) = tag.rsplit_once(['-', '_'])?;
        tag = parent;
        let Ok(script) = <[u8; 4]>::try_from(subtag.as_bytes()) else {
            continue;
        };
        let script = script.map(|byte| byte.to_ascii_lowercase());
        match &script {
            b"arab" => return Some("Arab"),
            b"cyrl" => return Some("Cyrl"),
            b"deva" => return Some("Deva"),
            b"grek" => return Some("Grek"),
            b"hang" | b"kore" => return Some("Hang"),
            b"hans" => return Some("Hans"),
            b"hant" => return Some("Hant"),
            b"hira" | b"hrkt" | b"jpan" | b"kana" => return Some("Jpan"),
            _ => {}
        }
        if OTHER_SCRIPT_SUBTAGS
            .binary_search_by(|key| key.as_bytes().cmp(&script))
            .is_ok()
        {
            return None;
        }
    }
}

pub(crate) fn browser_generic_family(
    generic: GenericFamily,
    locale: &str,
    initial: bool,
) -> Option<&'static str> {
    use GenericFamily::{Monospace, SansSerif, Serif};

    if !cfg!(target_os = "windows") {
        return None;
    }
    match (locale_script(locale)?, generic) {
        ("Arab" | "Cyrl" | "Grek", Monospace) => Some("Courier New"),
        ("Arab", SansSerif) => Some("Segoe UI"),
        ("Deva", Serif | SansSerif) => Some("Nirmala UI"),
        ("Jpan", Monospace) => Some("MS Gothic"),
        ("Jpan", Serif | SansSerif) => Some("Yu Gothic"),
        ("Hang", Monospace | Serif | SansSerif) => Some("Malgun Gothic"),
        ("Hans", Monospace) => Some("NSimSun"),
        ("Hans", Serif) if !initial => Some("SimSun"),
        ("Hans", Serif | SansSerif) => Some("Microsoft YaHei"),
        ("Hant", Monospace | Serif | SansSerif) => Some("Microsoft JhengHei"),
        _ => None,
    }
}

pub(crate) fn prefer_browser_script_fallbacks(collection: &mut Collection) {
    if !cfg!(target_os = "windows") {
        return;
    }
    for (script, locale, names) in SCRIPT_FAMILIES {
        let Some(preferred) = names.iter().find_map(|name| collection.family_id(name)) else {
            continue;
        };
        let language = match locale {
            Some(tag) => match Language::parse(tag) {
                Ok(language) => Some(language),
                Err(_) => continue,
            },
            None => None,
        };
        let key = FallbackKey::new(Script::from_bytes(*script), language.as_ref());
        let system: Vec<_> = collection
            .fallback_families(key)
            .filter(|family| *family != preferred)
            .collect();
        collection.set_fallbacks(key, std::iter::once(preferred).chain(system));
    }
}

#[cfg(test)]
mod tests {
    use super::{LOCALE_SCRIPTS, OTHER_SCRIPT_SUBTAGS, locale_script};

    #[test]
    fn locale_tables_are_strictly_sorted() {
        for pair in LOCALE_SCRIPTS.windows(2) {
            assert!(pair[0].0 < pair[1].0, "{pair:?}");
        }
        for pair in OTHER_SCRIPT_SUBTAGS.windows(2) {
            assert!(pair[0] < pair[1], "{pair:?}");
        }
    }

    #[test]
    fn locale_script_handles_case_and_subtag_order() {
        for (locale, expected) in [
            ("ja-JP", Some("Jpan")),
            ("zh-Hant-HK", Some("Hant")),
            ("zh_TW", Some("Hant")),
            ("sr-Latn-RS", None),
            ("en-US", None),
            ("x-klingon", None),
            ("", None),
            ("ja-", Some("Jpan")),
            ("zh-Zzzz-MO", Some("Hans")),
        ] {
            assert_eq!(locale_script(locale), expected, "{locale:?}");
        }
    }
}
