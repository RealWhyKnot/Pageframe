enum System {
    Numeric(&'static str),
    Alphabetic(&'static str),
    Additive {
        max: u32,
        symbols: &'static [(u32, &'static str)],
        uppercase: bool,
    },
    LeadingZero,
}

const ROMAN: &[(u32, &str)] = &[
    (1000, "m"),
    (900, "cm"),
    (500, "d"),
    (400, "cd"),
    (100, "c"),
    (90, "xc"),
    (50, "l"),
    (40, "xl"),
    (10, "x"),
    (9, "ix"),
    (5, "v"),
    (4, "iv"),
    (1, "i"),
];

const ARMENIAN: &[(u32, &str)] = &[
    (9000, "ք"),
    (8000, "փ"),
    (7000, "ւ"),
    (6000, "ց"),
    (5000, "ր"),
    (4000, "տ"),
    (3000, "վ"),
    (2000, "ս"),
    (1000, "ռ"),
    (900, "ջ"),
    (800, "պ"),
    (700, "չ"),
    (600, "ո"),
    (500, "շ"),
    (400, "ն"),
    (300, "յ"),
    (200, "մ"),
    (100, "ճ"),
    (90, "ղ"),
    (80, "ձ"),
    (70, "հ"),
    (60, "կ"),
    (50, "ծ"),
    (40, "խ"),
    (30, "լ"),
    (20, "ի"),
    (10, "ժ"),
    (9, "թ"),
    (8, "ը"),
    (7, "է"),
    (6, "զ"),
    (5, "ե"),
    (4, "դ"),
    (3, "գ"),
    (2, "բ"),
    (1, "ա"),
];

const GEORGIAN: &[(u32, &str)] = &[
    (10000, "ჵ"),
    (9000, "ჰ"),
    (8000, "ჯ"),
    (7000, "ჴ"),
    (6000, "ხ"),
    (5000, "ჭ"),
    (4000, "წ"),
    (3000, "ძ"),
    (2000, "ც"),
    (1000, "ჩ"),
    (900, "შ"),
    (800, "ყ"),
    (700, "ღ"),
    (600, "ქ"),
    (500, "ფ"),
    (400, "ჳ"),
    (300, "ტ"),
    (200, "ს"),
    (100, "რ"),
    (90, "ჟ"),
    (80, "პ"),
    (70, "ო"),
    (60, "ჲ"),
    (50, "ნ"),
    (40, "მ"),
    (30, "ლ"),
    (20, "კ"),
    (10, "ი"),
    (9, "თ"),
    (8, "ჱ"),
    (7, "ზ"),
    (6, "ვ"),
    (5, "ე"),
    (4, "დ"),
    (3, "გ"),
    (2, "ბ"),
    (1, "ა"),
];

const HEBREW: &[(u32, &str)] = &[
    (10000, "י׳"),
    (9000, "ט׳"),
    (8000, "ח׳"),
    (7000, "ז׳"),
    (6000, "ו׳"),
    (5000, "ה׳"),
    (4000, "ד׳"),
    (3000, "ג׳"),
    (2000, "ב׳"),
    (1000, "א׳"),
    (400, "ת"),
    (300, "ש"),
    (200, "ר"),
    (100, "ק"),
    (90, "צ"),
    (80, "פ"),
    (70, "ע"),
    (60, "ס"),
    (50, "נ"),
    (40, "מ"),
    (30, "ל"),
    (20, "כ"),
    (19, "יט"),
    (18, "יח"),
    (17, "יז"),
    (16, "טז"),
    (15, "טו"),
    (10, "י"),
    (9, "ט"),
    (8, "ח"),
    (7, "ז"),
    (6, "ו"),
    (5, "ה"),
    (4, "ד"),
    (3, "ג"),
    (2, "ב"),
    (1, "א"),
];

const LATIN: &str = "abcdefghijklmnopqrstuvwxyz";
const UPPER_LATIN: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const GREEK: &str = "αβγδεζηθικλμνξοπρστυφχψω";
const HIRAGANA: &str = "あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめもやゆよらりるれろわゐゑをん";
const HIRAGANA_IROHA: &str = "いろはにほへとちりぬるをわかよたれそつねならむうゐのおくやまけふこえてあさきゆめみしゑひもせす";
const KATAKANA: &str = "アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヰヱヲン";
const KATAKANA_IROHA: &str = "イロハニホヘトチリヌルヲワカヨタレソツネナラムウヰノオクヤマケフコエテアサキユメミシヱヒモセス";

fn system(name: &str) -> Option<System> {
    Some(match name {
        "decimal" => System::Numeric("0123456789"),
        "decimal-leading-zero" => System::LeadingZero,
        "arabic-indic" => System::Numeric("٠١٢٣٤٥٦٧٨٩"),
        "persian" => System::Numeric("۰۱۲۳۴۵۶۷۸۹"),
        "devanagari" => System::Numeric("०१२३४५६७८९"),
        "bengali" => System::Numeric("০১২৩৪৫৬৭৮৯"),
        "thai" => System::Numeric("๐๑๒๓๔๕๖๗๘๙"),
        "cjk-decimal" => System::Numeric("〇一二三四五六七八九"),
        "lower-alpha" | "lower-latin" => System::Alphabetic(LATIN),
        "upper-alpha" | "upper-latin" => System::Alphabetic(UPPER_LATIN),
        "lower-greek" => System::Alphabetic(GREEK),
        "hiragana" => System::Alphabetic(HIRAGANA),
        "hiragana-iroha" => System::Alphabetic(HIRAGANA_IROHA),
        "katakana" => System::Alphabetic(KATAKANA),
        "katakana-iroha" => System::Alphabetic(KATAKANA_IROHA),
        "lower-roman" => System::Additive {
            max: 3999,
            symbols: ROMAN,
            uppercase: false,
        },
        "upper-roman" => System::Additive {
            max: 3999,
            symbols: ROMAN,
            uppercase: true,
        },
        "armenian" | "upper-armenian" => System::Additive {
            max: 9999,
            symbols: ARMENIAN,
            uppercase: true,
        },
        "lower-armenian" => System::Additive {
            max: 9999,
            symbols: ARMENIAN,
            uppercase: false,
        },
        "georgian" => System::Additive {
            max: 19999,
            symbols: GEORGIAN,
            uppercase: false,
        },
        "hebrew" => System::Additive {
            max: 10999,
            symbols: HEBREW,
            uppercase: false,
        },
        _ => return None,
    })
}

pub(crate) fn suffix(name: &str) -> &'static str {
    match name {
        "cjk-decimal" | "hiragana" | "hiragana-iroha" | "katakana" | "katakana-iroha" => "、",
        _ => ". ",
    }
}

pub(crate) fn text(name: &str, value: i32) -> String {
    let generated = match system(name) {
        Some(System::Numeric(digits)) => numeric(digits, value),
        Some(System::Alphabetic(symbols)) => alphabetic(symbols, value),
        Some(System::Additive {
            max,
            symbols,
            uppercase,
        }) => additive(symbols, max, value)
            .map(|text| if uppercase { text.to_uppercase() } else { text }),
        Some(System::LeadingZero) => Some(leading_zero(value)),
        None => None,
    };
    generated.unwrap_or_else(|| value.to_string())
}

fn symbol(symbols: &[char], index: u32) -> Option<char> {
    symbols.get(usize::try_from(index).ok()?).copied()
}

fn numeric(digits: &str, value: i32) -> Option<String> {
    let digits: Vec<char> = digits.chars().collect();
    let base = u32::try_from(digits.len()).ok()?;
    let mut rest = value.unsigned_abs();
    let mut reversed = Vec::new();
    loop {
        reversed.push(symbol(&digits, rest % base)?);
        rest /= base;
        if rest == 0 {
            break;
        }
    }
    if value < 0 {
        reversed.push('-');
    }
    Some(reversed.into_iter().rev().collect())
}

fn alphabetic(symbols: &str, value: i32) -> Option<String> {
    let symbols: Vec<char> = symbols.chars().collect();
    let base = u32::try_from(symbols.len()).ok()?;
    let mut rest = u32::try_from(value).ok().filter(|value| *value > 0)?;
    let mut reversed = Vec::new();
    while rest > 0 {
        rest -= 1;
        reversed.push(symbol(&symbols, rest % base)?);
        rest /= base;
    }
    Some(reversed.into_iter().rev().collect())
}

fn additive(symbols: &[(u32, &str)], max: u32, value: i32) -> Option<String> {
    let mut rest = u32::try_from(value)
        .ok()
        .filter(|value| (1..=max).contains(value))?;
    let mut text = String::new();
    for (weight, symbol) in symbols {
        while rest >= *weight {
            text.push_str(symbol);
            rest -= weight;
        }
    }
    Some(text)
}

fn leading_zero(value: i32) -> String {
    let sign = if value < 0 { "-" } else { "" };
    format!("{sign}{:02}", value.unsigned_abs())
}

#[cfg(test)]
mod tests {
    use super::{suffix, text};

    #[test]
    fn decimal_handles_zero_and_negatives() {
        assert_eq!(text("decimal", 0), "0");
        assert_eq!(text("decimal", -12), "-12");
        assert_eq!(text("decimal", 100), "100");
    }

    #[test]
    fn leading_zero_pads_single_digits() {
        assert_eq!(text("decimal-leading-zero", 1), "01");
        assert_eq!(text("decimal-leading-zero", -3), "-03");
        assert_eq!(text("decimal-leading-zero", 123), "123");
    }

    #[test]
    fn alphabetic_wraps_after_the_last_symbol() {
        assert_eq!(text("lower-alpha", 1), "a");
        assert_eq!(text("lower-alpha", 26), "z");
        assert_eq!(text("lower-alpha", 27), "aa");
        assert_eq!(text("upper-latin", 28), "AB");
        assert_eq!(text("lower-greek", 2), "β");
        assert_eq!(text("lower-alpha", 0), "0");
    }

    #[test]
    fn roman_falls_back_to_decimal_out_of_range() {
        assert_eq!(text("lower-roman", 4), "iv");
        assert_eq!(text("upper-roman", 1994), "MCMXCIV");
        assert_eq!(text("upper-roman", 3999), "MMMCMXCIX");
        assert_eq!(text("upper-roman", 4000), "4000");
        assert_eq!(text("lower-roman", 0), "0");
    }

    #[test]
    fn additive_scripts_use_their_tables() {
        assert_eq!(text("armenian", 1), "Ա");
        assert_eq!(text("lower-armenian", 12), "ժբ");
        assert_eq!(text("georgian", 1), "ა");
        assert_eq!(text("hebrew", 1), "א");
        assert_eq!(text("hebrew", 15), "טו");
        assert_eq!(text("hebrew", 16), "טז");
    }

    #[test]
    fn numeric_scripts_use_their_digits() {
        assert_eq!(text("cjk-decimal", 10), "一〇");
        assert_eq!(text("arabic-indic", 25), "٢٥");
    }

    #[test]
    fn unknown_styles_use_decimal() {
        assert_eq!(text("no-such-style", 7), "7");
    }

    #[test]
    fn ideographic_styles_use_an_ideographic_comma() {
        assert_eq!(suffix("cjk-decimal"), "、");
        assert_eq!(suffix("decimal"), ". ");
    }
}
