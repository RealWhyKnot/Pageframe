use parley::fontique::{Collection, FallbackKey, Script};

const JAPANESE_KANA: &[&str] = &[
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

const SCRIPT_FAMILIES: &[([u8; 4], &[&str])] = &[
    (*b"Hira", JAPANESE_KANA),
    (*b"Kana", JAPANESE_KANA),
    (*b"Hani", SIMPLIFIED_HAN),
];

pub(crate) fn prefer_browser_script_fallbacks(collection: &mut Collection) {
    if !cfg!(target_os = "windows") {
        return;
    }
    for (script, names) in SCRIPT_FAMILIES {
        let Some(preferred) = names.iter().find_map(|name| collection.family_id(name)) else {
            continue;
        };
        let key = FallbackKey::new(Script::from_bytes(*script), None);
        let system: Vec<_> = collection
            .fallback_families(key)
            .filter(|family| *family != preferred)
            .collect();
        collection.set_fallbacks(key, std::iter::once(preferred).chain(system));
    }
}
