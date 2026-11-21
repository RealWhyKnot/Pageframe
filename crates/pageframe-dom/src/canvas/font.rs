#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FontSlant {
    #[default]
    Normal,
    Italic,
    Oblique,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FontSpec {
    pub slant: FontSlant,
    pub small_caps: bool,
    pub weight: f32,
    pub stretch: f32,
    pub size: f32,
    pub family: String,
}

impl Default for FontSpec {
    fn default() -> Self {
        Self {
            slant: FontSlant::Normal,
            small_caps: false,
            weight: 400.0,
            stretch: 100.0,
            size: 10.0,
            family: "sans-serif".to_string(),
        }
    }
}

const STRETCHES: [(&str, f32); 9] = [
    ("ultra-condensed", 50.0),
    ("extra-condensed", 62.5),
    ("condensed", 75.0),
    ("semi-condensed", 87.5),
    ("semi-expanded", 112.5),
    ("expanded", 125.0),
    ("extra-expanded", 150.0),
    ("ultra-expanded", 200.0),
    ("normal", 100.0),
];

const SIZE_KEYWORDS: [(&str, f32); 9] = [
    ("xx-small", 9.0),
    ("x-small", 10.0),
    ("small", 13.0),
    ("medium", 16.0),
    ("large", 18.0),
    ("x-large", 24.0),
    ("xx-large", 32.0),
    ("xxx-large", 48.0),
    ("smaller", 8.333_333),
];

impl FontSpec {
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let mut spec = Self::default();
        let mut rest = text;
        loop {
            let (word, tail) = split_word(rest);
            if word.is_empty() {
                return None;
            }
            let lower = word.to_ascii_lowercase();
            if let Some(size) = parse_size(&lower) {
                spec.size = size;
                rest = skip_line_height(tail)?;
                break;
            }
            match lower.as_str() {
                "normal" => {}
                "italic" => spec.slant = FontSlant::Italic,
                "oblique" => spec.slant = FontSlant::Oblique,
                "small-caps" => spec.small_caps = true,
                "bold" | "bolder" => spec.weight = 700.0,
                "lighter" => spec.weight = 100.0,
                _ => {
                    if let Some((_, stretch)) = STRETCHES.iter().find(|(name, _)| *name == lower) {
                        spec.stretch = *stretch;
                    } else if let Ok(weight) = lower.parse::<f32>() {
                        if !(1.0..=1000.0).contains(&weight) {
                            return None;
                        }
                        spec.weight = weight;
                    } else {
                        return None;
                    }
                }
            }
            rest = tail;
        }
        let family = rest.trim();
        if family.is_empty() || family.ends_with(',') {
            return None;
        }
        spec.family = normalize_family(family)?;
        Some(spec)
    }

    pub fn serialize(&self) -> String {
        let mut out = String::new();
        match self.slant {
            FontSlant::Normal => {}
            FontSlant::Italic => out.push_str("italic "),
            FontSlant::Oblique => out.push_str("oblique "),
        }
        if self.small_caps {
            out.push_str("small-caps ");
        }
        if self.weight != 400.0 {
            if self.weight == 700.0 {
                out.push_str("bold ");
            } else {
                out.push_str(&format!("{} ", self.weight));
            }
        }
        if let Some((name, _)) = STRETCHES
            .iter()
            .find(|(name, v)| *v == self.stretch && *name != "normal")
        {
            out.push_str(name);
            out.push(' ');
        }
        out.push_str(&format!("{}px {}", format_number(self.size), self.family));
        out
    }
}

fn format_number(value: f32) -> String {
    let rounded = (f64::from(value) * 10000.0).round() / 10000.0;
    let text = format!("{rounded:.4}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn split_word(text: &str) -> (&str, &str) {
    let text = text.trim_start();
    let end = text
        .find(|c: char| c.is_whitespace() || c == '/')
        .unwrap_or(text.len());
    (&text[..end], &text[end..])
}

fn skip_line_height(text: &str) -> Option<&str> {
    let trimmed = text.trim_start();
    let Some(after) = trimmed.strip_prefix('/') else {
        return Some(text);
    };
    let (word, tail) = split_word(after);
    if word.is_empty() {
        return None;
    }
    Some(tail)
}

fn parse_size(word: &str) -> Option<f32> {
    if let Some((_, size)) = SIZE_KEYWORDS.iter().find(|(name, _)| *name == word) {
        return Some(*size);
    }
    if word == "larger" {
        return Some(12.0);
    }
    let units: [(&str, f32); 8] = [
        ("px", 1.0),
        ("pt", 4.0 / 3.0),
        ("pc", 16.0),
        ("in", 96.0),
        ("cm", 96.0 / 2.54),
        ("mm", 96.0 / 25.4),
        ("rem", 16.0),
        ("em", 10.0),
    ];
    for (unit, scale) in units {
        if let Some(number) = word.strip_suffix(unit) {
            let value: f32 = number.parse().ok()?;
            let size = value * scale;
            return (value >= 0.0 && size.is_finite()).then_some(size);
        }
    }
    let number = word.strip_suffix('%')?;
    let value: f32 = number.parse().ok()?;
    (value >= 0.0 && value.is_finite()).then_some(value / 10.0)
}

fn normalize_family(family: &str) -> Option<String> {
    let mut names = Vec::new();
    for part in split_families(family)? {
        let part = part.trim();
        if part.is_empty() {
            return None;
        }
        let quoted = part.len() >= 2
            && ((part.starts_with('"') && part.ends_with('"'))
                || (part.starts_with('\'') && part.ends_with('\'')));
        if quoted {
            names.push(format!("\"{}\"", &part[1..part.len() - 1]));
        } else {
            let words: Vec<&str> = part.split_whitespace().collect();
            names.push(words.join(" "));
        }
    }
    Some(names.join(", "))
}

fn split_families(family: &str) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    let mut quote: Option<char> = None;
    let mut start = 0;
    for (i, c) in family.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == ',' => {
                parts.push(&family[start..i]);
                start = i + 1;
            }
            None => {}
        }
    }
    if quote.is_some() {
        return None;
    }
    parts.push(&family[start..]);
    Some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_backtrack_fonts() {
        let spec = FontSpec::parse("600 15px 'Segoe UI', system-ui, sans-serif").unwrap();
        assert_eq!(spec.weight, 600.0);
        assert_eq!(spec.size, 15.0);
        assert_eq!(spec.family, "\"Segoe UI\", system-ui, sans-serif");
        assert_eq!(
            spec.serialize(),
            "600 15px \"Segoe UI\", system-ui, sans-serif"
        );
        let spec = FontSpec::parse("21px \"Segoe UI\", sans-serif").unwrap();
        assert_eq!(spec.serialize(), "21px \"Segoe UI\", sans-serif");
        let spec = FontSpec::parse("italic bold 12px/30px Georgia, serif").unwrap();
        assert_eq!(spec.serialize(), "italic bold 12px Georgia, serif");
    }

    #[test]
    fn rejects_invalid_fonts() {
        assert!(FontSpec::parse("bold").is_none());
        assert!(FontSpec::parse("12px").is_none());
        assert!(FontSpec::parse("12px 'Unclosed").is_none());
        assert!(FontSpec::parse("huge 12px serif").is_none());
    }
}
