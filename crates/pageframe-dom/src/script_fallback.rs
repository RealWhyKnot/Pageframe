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

const ARABIC_GENERIC_FAMILIES: &[(GenericFamily, &str)] = &[
    (GenericFamily::Monospace, "Courier New"),
    (GenericFamily::SansSerif, "Segoe UI"),
];

pub(crate) fn browser_generic_family(
    generic: GenericFamily,
    locale: Option<&Language>,
) -> Option<&'static str> {
    if !cfg!(target_os = "windows") {
        return None;
    }
    let locale = locale?;
    let arabic = match locale.script() {
        Some(script) => script == "Arab",
        None => matches!(locale.language(), "ar" | "fa" | "ur"),
    };
    if !arabic {
        return None;
    }
    ARABIC_GENERIC_FAMILIES
        .iter()
        .find_map(|&(family, name)| (family == generic).then_some(name))
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
