// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;
use alloc::vec::Vec;
use icu_properties::CodePointMapData;
use icu_properties::props::{EastAsianWidth, GeneralCategory};
use skrifa::MetadataProvider;
use skrifa::raw::TableProvider;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CharType {
    Other,
    Open,
    Close,
    Middle,
    OpenNarrow,
    CloseNarrow,
    Dot,
    Colon,
    Semicolon,
    OpenQuote,
    CloseQuote,
}

fn base_char_type(ch: char) -> CharType {
    match ch {
        '\u{2018}' | '\u{201C}' => CharType::OpenQuote,
        '\u{2019}' | '\u{201D}' => CharType::CloseQuote,
        '\u{3000}' | '\u{00B7}' | '\u{2027}' | '\u{30FB}' => CharType::Middle,
        '\u{3001}' | '\u{3002}' | '\u{FF0C}' | '\u{FF0E}' => CharType::Dot,
        '\u{FF1A}' => CharType::Colon,
        '\u{FF1B}' => CharType::Semicolon,
        '\u{10000}'.. => CharType::Other,
        _ => {
            let wide = matches!(ch, '\u{3000}'..='\u{303F}')
                || const { CodePointMapData::<EastAsianWidth>::new() }.get(ch)
                    == EastAsianWidth::Fullwidth;
            match const { CodePointMapData::<GeneralCategory>::new() }.get(ch) {
                GeneralCategory::OpenPunctuation if wide => CharType::Open,
                GeneralCategory::ClosePunctuation if wide => CharType::Close,
                GeneralCategory::OpenPunctuation => CharType::OpenNarrow,
                GeneralCategory::ClosePunctuation => CharType::CloseNarrow,
                _ => CharType::Other,
            }
        }
    }
}

fn should_kern(char_type: CharType, last: CharType) -> bool {
    char_type == CharType::Open
        && matches!(
            last,
            CharType::Open | CharType::Middle | CharType::Close | CharType::OpenNarrow
        )
}

fn should_kern_last(char_type: CharType, last: CharType) -> bool {
    last == CharType::Close
        && matches!(
            char_type,
            CharType::Close | CharType::Middle | CharType::CloseNarrow
        )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Flow {
    Horizontal,
    Vertical,
}

impl Flow {
    pub(crate) fn alternate_tag(self) -> harfrust::Tag {
        match self {
            Self::Horizontal => harfrust::Tag::new(b"halt"),
            Self::Vertical => harfrust::Tag::new(b"vhal"),
        }
    }

    fn contextual_tag(self) -> harfrust::Tag {
        match self {
            Self::Horizontal => harfrust::Tag::new(b"chws"),
            Self::Vertical => harfrust::Tag::new(b"vchw"),
        }
    }

    fn direction(self) -> harfrust::Direction {
        match self {
            Self::Horizontal => harfrust::Direction::LeftToRight,
            Self::Vertical => harfrust::Direction::TopToBottom,
        }
    }
}

const EXCLUSIVE_FEATURES: [&[u8; 4]; 9] = [
    b"halt", b"hwid", b"palt", b"pwid", b"qwid", b"twid", b"valt", b"vhal", b"vpal",
];

pub(crate) fn has_exclusive_feature(features: &[harfrust::Feature]) -> bool {
    features.iter().any(|feature| {
        feature.value != 0
            && EXCLUSIVE_FEATURES
                .iter()
                .any(|tag| feature.tag == harfrust::Tag::new(tag))
    })
}

pub(crate) fn may_apply(text: &str) -> bool {
    text.chars()
        .any(|ch| ch >= '\u{2018}' && base_char_type(ch) != CharType::Other)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spacing {
    Alternate,
    Contextual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FontSpacing {
    spacing: Spacing,
    dot: CharType,
    colon: CharType,
    semicolon: CharType,
    quotes_fullwidth: bool,
}

const PROBE: [char; 10] = [
    '\u{3001}', '\u{3002}', '\u{FF0C}', '\u{FF0E}', '\u{FF1A}', '\u{FF1B}', '\u{201C}',
    '\u{2018}', '\u{201D}', '\u{2019}',
];

#[derive(Clone, Copy, Debug)]
struct ProbeGlyph {
    advance: i32,
    near: f64,
    far: f64,
    extent: f64,
}

impl ProbeGlyph {
    fn char_type(&self, half: f64) -> CharType {
        if self.far <= half {
            return CharType::Close;
        }
        if self.near >= half {
            return CharType::Open;
        }
        if self.extent <= half && self.near >= half / 2. {
            return CharType::Middle;
        }
        CharType::Other
    }
}

fn group_type(glyphs: &[ProbeGlyph]) -> CharType {
    let Some(first) = glyphs.first() else {
        return CharType::Other;
    };
    let half = f64::from(first.advance) / 2.;
    let first_type = first.char_type(half);
    let uniform = glyphs
        .iter()
        .all(|glyph| glyph.advance == first.advance && glyph.char_type(half) == first_type);
    if uniform { first_type } else { CharType::Other }
}

fn has_feature(font: &skrifa::FontRef<'_>, tag: harfrust::Tag) -> bool {
    let has = |records: &[skrifa::raw::tables::layout::FeatureRecord]| {
        records.iter().any(|record| record.feature_tag() == tag)
    };
    let in_gsub = font
        .gsub()
        .ok()
        .and_then(|gsub| gsub.feature_list().ok())
        .is_some_and(|list| has(list.feature_records()));
    in_gsub
        || font
            .gpos()
            .ok()
            .and_then(|gpos| gpos.feature_list().ok())
            .is_some_and(|list| has(list.feature_records()))
}

fn is_excluded(font: &skrifa::FontRef<'_>) -> bool {
    font.localized_strings(skrifa::string::StringId::POSTSCRIPT_NAME)
        .english_or_first()
        .is_some_and(|name| name.chars().collect::<String>().starts_with("YuGothicUI"))
}

pub(crate) fn font_spacing(
    shaper: &harfrust::Shaper<'_>,
    font: &skrifa::FontRef<'_>,
    flow: Flow,
    language: Option<&harfrust::Language>,
) -> Option<FontSpacing> {
    if !has_feature(font, flow.alternate_tag()) || is_excluded(font) {
        return None;
    }
    let spacing = if has_feature(font, flow.contextual_tag()) {
        Spacing::Contextual
    } else {
        Spacing::Alternate
    };

    let mut buffer = harfrust::UnicodeBuffer::new();
    for (cluster, ch) in (0_u32..).zip(PROBE) {
        buffer.add(ch, cluster);
    }
    buffer.set_direction(flow.direction());
    buffer.set_script(harfrust::script::HAN);
    if let Some(language) = language {
        buffer.set_language(language.clone());
    }
    let shaped = shaper.shape(buffer, harfrust::ShapeOptions::new());
    let infos = shaped.glyph_infos();
    let positions = shaped.glyph_positions();
    if infos.len() != PROBE.len()
        || infos
            .iter()
            .zip(0_u32..)
            .any(|(info, cluster)| info.cluster != cluster || info.glyph_id == 0)
    {
        return None;
    }

    let metrics = font.glyph_metrics(
        skrifa::instance::Size::unscaled(),
        skrifa::instance::LocationRef::default(),
    );
    let glyphs = infos
        .iter()
        .zip(positions)
        .map(|(info, position)| {
            let bounds = metrics.bounds(skrifa::GlyphId::new(info.glyph_id))?;
            let x = f64::from(position.x_offset);
            let y = f64::from(position.y_offset);
            let (x_min, x_max) = (f64::from(bounds.x_min), f64::from(bounds.x_max));
            let (y_min, y_max) = (f64::from(bounds.y_min), f64::from(bounds.y_max));
            Some(match flow {
                Flow::Horizontal => ProbeGlyph {
                    advance: position.x_advance,
                    near: x + x_min,
                    far: x + x_max,
                    extent: x_max - x_min,
                },
                Flow::Vertical => ProbeGlyph {
                    advance: -position.y_advance,
                    near: -(y + y_max),
                    far: -(y + y_min),
                    extent: y_max - y_min,
                },
            })
        })
        .collect::<Option<Vec<_>>>()?;

    Some(FontSpacing {
        spacing,
        dot: group_type(&glyphs[..4]),
        colon: group_type(&glyphs[4..5]),
        semicolon: group_type(&glyphs[5..6]),
        quotes_fullwidth: group_type(&glyphs[6..8]) == CharType::Open
            && group_type(&glyphs[8..10]) == CharType::Close,
    })
}

impl FontSpacing {
    fn char_type(&self, ch: char) -> CharType {
        match base_char_type(ch) {
            CharType::Dot => self.dot,
            CharType::Colon => self.colon,
            CharType::Semicolon => self.semicolon,
            CharType::OpenQuote if self.quotes_fullwidth => CharType::Open,
            CharType::OpenQuote => CharType::OpenNarrow,
            CharType::CloseQuote if self.quotes_fullwidth => CharType::Close,
            CharType::CloseQuote => CharType::CloseNarrow,
            other => other,
        }
    }

    pub(crate) fn kerned_indices(
        &self,
        run: &str,
        before: Option<char>,
        after: Option<char>,
        indices: &mut Vec<usize>,
    ) {
        indices.clear();
        let mut types = run.chars().map(|ch| self.char_type(ch));
        let Some(first) = types.next() else {
            return;
        };
        if before.is_some_and(|ch| should_kern(first, self.char_type(ch))) {
            indices.push(0);
        }
        let mut last = first;
        let mut last_index = 0;
        for char_type in types {
            if self.spacing == Spacing::Alternate {
                if should_kern_last(char_type, last) {
                    indices.push(last_index);
                } else if should_kern(char_type, last) {
                    indices.push(last_index + 1);
                }
            }
            last = char_type;
            last_index += 1;
        }
        if after.is_some_and(|ch| should_kern_last(self.char_type(ch), last)) {
            indices.push(last_index);
        }
        indices.dedup();
    }
}
