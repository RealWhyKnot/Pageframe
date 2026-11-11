use crate::node::{Node, NodeData};
use color::{AlphaColor, Srgb};
use keyboard_types::Modifiers;
use std::borrow::Cow;
use style::color::AbsoluteColor;

#[cfg(target_os = "macos")]
pub(crate) const ACTION_MOD: Modifiers = Modifiers::META;
#[cfg(not(target_os = "macos"))]
pub(crate) const ACTION_MOD: Modifiers = Modifiers::CONTROL;

pub type Color = AlphaColor<Srgb>;

/// Decode raw font bytes, decompressing WOFF/WOFF2 if the `woff` feature is enabled.
/// Returns the original slice unchanged for TTF/OTF input, and also on decompression
/// failure. With the `woff` feature disabled, all input passes through unchanged.
pub fn decode_font_bytes(bytes: &[u8]) -> Cow<'_, [u8]> {
    if bytes.len() < 4 {
        return Cow::Borrowed(bytes);
    }
    match &bytes[0..4] {
        #[cfg(feature = "woff")]
        b"wOFF" => wuff::decompress_woff1(bytes)
            .map(Cow::Owned)
            .unwrap_or_else(|_| {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to decompress woff1 font");
                Cow::Borrowed(bytes)
            }),
        #[cfg(feature = "woff")]
        b"wOF2" => wuff::decompress_woff2(bytes)
            .map(Cow::Owned)
            .unwrap_or_else(|_| {
                #[cfg(feature = "tracing")]
                tracing::warn!("Failed to decompress woff2 font");
                Cow::Borrowed(bytes)
            }),
        _ => Cow::Borrowed(bytes),
    }
}

#[cfg(feature = "svg")]
use std::sync::{Arc, LazyLock};
#[cfg(feature = "svg")]
use usvg::fontdb;
#[cfg(feature = "svg")]
pub(crate) static FONT_DB: LazyLock<Arc<fontdb::Database>> = LazyLock::new(|| {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    Arc::new(db)
});

/// Which kind of CSS image layer list (`background-image` or `mask-image`) to
/// flush from style to dedicated storage on the node.
#[derive(Clone, Copy, Debug)]
pub enum ImageLayerKind {
    Background,
    Mask,
}

impl ImageLayerKind {
    pub fn image_type(self, idx: usize) -> ImageType {
        match self {
            Self::Background => ImageType::Background(idx),
            Self::Mask => ImageType::Mask(idx),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ImageType {
    Image,
    Background(usize),
    Mask(usize),
}

/// A point
#[derive(Clone, Debug, Copy, Eq, PartialEq)]
pub struct Point<T> {
    /// The x coordinate
    pub x: T,
    /// The y coordinate
    pub y: T,
}

impl Point<f64> {
    pub const ZERO: Self = Point { x: 0.0, y: 0.0 };
}

// Debug print an RcDom
pub fn walk_tree(indent: usize, node: &Node) {
    // Skip all-whitespace text nodes entirely
    if let NodeData::Text(data) = &node.data {
        if data.content.chars().all(|c| c.is_ascii_whitespace()) {
            return;
        }
    }

    print!("{}", " ".repeat(indent));
    let id = node.id;
    match &node.data {
        NodeData::Document(_) => println!("#Document {id}"),

        NodeData::Text(data) => {
            if data.content.chars().all(|c| c.is_ascii_whitespace()) {
                println!("{id} #text: <whitespace>");
            } else {
                let content = data.content.trim();
                if content.len() > 10 {
                    println!(
                        "#text {id}: {}...",
                        content
                            .split_at(content.char_indices().take(10).last().unwrap().0)
                            .0
                            .escape_default()
                    )
                } else {
                    println!("#text {id}: {}", data.content.trim().escape_default())
                }
            }
        }

        NodeData::Comment { .. } => println!("<!-- COMMENT {id} -->"),

        NodeData::AnonymousBlock(_) => println!("{id} AnonymousBlock"),

        NodeData::Element(data) => {
            print!("<{} {id}", data.name.local);
            for attr in data.attrs.iter() {
                print!(" {}=\"{}\"", attr.name.local, attr.value);
            }
            if !node.children.is_empty() {
                println!(">");
            } else {
                println!("/>");
            }
        } // NodeData::Doctype {
          //     ref name,
          //     ref public_id,
          //     ref system_id,
          // } => println!("<!DOCTYPE {} \"{}\" \"{}\">", name, public_id, system_id),
          // NodeData::ProcessingInstruction { .. } => unreachable!(),
    }

    if !node.children.is_empty() {
        for child_id in node.children.iter() {
            walk_tree(indent + 2, node.with(*child_id));
        }

        if let NodeData::Element(data) = &node.data {
            println!("{}</{}>", " ".repeat(indent), data.name.local);
        }
    }
}

/// Parse an SVG image.
#[cfg(feature = "svg")]
pub(crate) fn parse_svg_image(source: &[u8]) -> Result<crate::node::SvgImageData, usvg::Error> {
    let options = usvg::Options {
        fontdb: Arc::clone(&*FONT_DB),
        ..Default::default()
    };
    crate::node::SvgImageData::from_data(source, &options)
}

pub trait ToColorColor {
    /// Converts a color into the `AlphaColor<Srgb>` type from the `color` crate
    fn as_color_color(&self) -> Color;
}
impl ToColorColor for AbsoluteColor {
    fn as_color_color(&self) -> Color {
        Color::new(
            *self
                .to_color_space(style::color::ColorSpace::Srgb)
                .raw_components(),
        )
    }
}

#[cfg(all(test, feature = "svg"))]
mod svg_tests {
    use super::parse_svg_image;

    #[test]
    fn svg_without_xmlns_parses() {
        let src = br#"<svg viewBox="0 0 24 24"><path d="M0 0h24v24H0z"/></svg>"#;
        let svg = parse_svg_image(src).unwrap();
        assert!(!svg.tree.root().children().is_empty());
    }

    #[test]
    fn missing_height_is_computed_from_width_and_viewbox_ratio() {
        let src = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="200"><rect width="100%" height="100%" fill="green"/></svg>"#;
        let svg = parse_svg_image(src).unwrap();
        assert_eq!(svg.intrinsic_width(), Some(200.0));
        assert_eq!(svg.intrinsic_height(), None);
        assert_eq!(svg.viewbox_aspect_ratio(), Some(1.0));
        assert_eq!(svg.tree.size().width(), 200.0);
        assert_eq!(svg.intrinsic_size(), (200.0, 200.0));
    }

    #[test]
    fn viewbox_only_has_no_intrinsic_dimensions() {
        let src = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 485 58"></svg>"#;
        let svg = parse_svg_image(src).unwrap();
        assert_eq!(svg.intrinsic_width(), None);
        assert_eq!(svg.intrinsic_height(), None);
        // The aspect ratio is still available from the viewBox.
        assert!((svg.aspect_ratio() - (485.0 / 58.0)).abs() < 1e-3);
    }

    #[test]
    fn absolute_dimensions_are_intrinsic() {
        let src = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="16" viewBox="0 0 48 32"></svg>"#;
        let svg = parse_svg_image(src).unwrap();
        assert_eq!(svg.intrinsic_width(), Some(24.0));
        assert_eq!(svg.intrinsic_height(), Some(16.0));
    }

    #[test]
    fn percentage_dimensions_are_not_intrinsic() {
        let src = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="50%" viewBox="0 0 200 100"></svg>"#;
        let svg = parse_svg_image(src).unwrap();
        assert_eq!(svg.intrinsic_width(), None);
        assert_eq!(svg.intrinsic_height(), None);
    }

    #[test]
    fn unit_lengths_are_intrinsic() {
        let src = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24px" height="1.5em" viewBox="0 0 48 32"></svg>"#;
        let svg = parse_svg_image(src).unwrap();
        assert!(svg.intrinsic_width().is_some());
        assert!(svg.intrinsic_height().is_some());
    }

    #[test]
    fn non_numeric_dimensions_are_not_intrinsic() {
        let src = br#"<svg xmlns="http://www.w3.org/2000/svg" width="auto" height="foo" viewBox="0 0 200 100"></svg>"#;
        let svg = parse_svg_image(src).unwrap();
        assert_eq!(svg.intrinsic_width(), None);
        assert_eq!(svg.intrinsic_height(), None);
    }
}

/// Creates an markup5ever::QualName.
/// Given a local name and an optional namespace
#[macro_export]
macro_rules! qual_name {
    ($local:tt $(, $ns:ident)?) => {
        $crate::QualName {
            prefix: None,
            ns: $crate::ns!($($ns)?),
            local: $crate::local_name!($local),
        }
    };
}

/// The number of lines `line-clamp` keeps visible in an inline layout, or `None` when the
/// property is unset or the content already fits within the limit.
pub fn clamped_line_count(
    style: &style::properties::ComputedValues,
    layout: &parley::Layout<crate::node::TextBrush>,
) -> Option<usize> {
    let max_lines = max_clamped_lines(style)?;
    (line_count(layout) > max_lines).then_some(max_lines)
}

pub(crate) fn max_clamped_lines(style: &style::properties::ComputedValues) -> Option<usize> {
    let max_lines = style.get_box().line_clamp.max_lines.lines_value()?;
    Some(usize::try_from(max_lines.0).ok()?.max(1))
}

pub(crate) fn line_count(layout: &parley::Layout<crate::node::TextBrush>) -> usize {
    match layout.get(layout.len().wrapping_sub(1)) {
        Some(last) if last.text_range().is_empty() && last.items().next().is_none() => {
            layout.len() - 1
        }
        _ => layout.len(),
    }
}

pub fn inherited_clamp_line(node: &crate::Node) -> Option<usize> {
    let budget = node
        .element_data()?
        .inline_layout_data
        .as_ref()?
        .line_budget?;
    let mut ancestor = node.parent;
    while let Some(ancestor_id) = ancestor {
        let ancestor_node = node.tree().get(ancestor_id)?;
        if ancestor_id == budget.container {
            let style = ancestor_node.primary_styles()?;
            return max_clamped_lines(&style).and(budget.lines.checked_sub(1));
        }
        ancestor = ancestor_node.parent;
    }
    None
}

pub const INLINE_EDGE_START: &str = "\u{2063}";
pub const INLINE_EDGE_END: &str = "\u{2060}";
pub const INLINE_MARGIN_START: &str = "\u{2064}";
pub const INLINE_MARGIN_END: &str = "\u{feff}";

pub fn is_inline_spacer(ch: char) -> bool {
    [
        INLINE_EDGE_START,
        INLINE_EDGE_END,
        INLINE_MARGIN_START,
        INLINE_MARGIN_END,
    ]
    .iter()
    .any(|spacer| spacer.contains(ch))
}

pub fn inline_relative_offset(style: &style::properties::ComputedValues) -> (f32, f32) {
    use style::values::generics::position::GenericInset;
    use stylo_taffy::CopyStyleGetters as _;
    if style.get_box().position != style::computed_values::position::T::Relative {
        return (0.0, 0.0);
    }
    let zero = style::values::computed::CSSPixelLength::new(0.0);
    let position = style.get_position();
    let resolve = |inset: &style::values::computed::Inset| match inset {
        GenericInset::LengthPercentage(value) => Some(value.resolve(zero).px()),
        _ => None,
    };
    let (left, right) = (resolve(&position.left), resolve(&position.right));
    let x = if style.clone_direction() == style::computed_values::direction::T::Rtl {
        right.map(|value| -value).or(left)
    } else {
        left.or(right.map(|value| -value))
    };
    let y = resolve(&position.top).or(resolve(&position.bottom).map(|value| -value));
    (x.unwrap_or(0.0), y.unwrap_or(0.0))
}

pub fn inline_box_is_decorated(style: &style::properties::ComputedValues) -> bool {
    let border = style.get_border();
    let padding = style.get_padding();
    let has_border = [
        (border.border_top_style, &border.border_top_width),
        (border.border_right_style, &border.border_right_width),
        (border.border_bottom_style, &border.border_bottom_width),
        (border.border_left_style, &border.border_left_width),
    ]
    .iter()
    .any(|(line, width)| !line.none_or_hidden() && width.0.to_f32_px() > 0.0);
    let has_padding = [
        &padding.padding_top,
        &padding.padding_right,
        &padding.padding_bottom,
        &padding.padding_left,
    ]
    .iter()
    .any(|value| !style::Zero::is_zero(&value.0));
    let has_radius = [
        &border.border_top_left_radius,
        &border.border_top_right_radius,
        &border.border_bottom_right_radius,
        &border.border_bottom_left_radius,
    ]
    .iter()
    .any(|radius| {
        !style::Zero::is_zero(&radius.0.width.0) && !style::Zero::is_zero(&radius.0.height.0)
    });
    let has_image = style
        .get_background()
        .background_image
        .0
        .iter()
        .any(|image| !matches!(image, style::values::generics::image::GenericImage::None));
    let outline = style.get_outline();
    let has_outline = match outline.outline_style {
        style::values::computed::OutlineStyle::Auto => false,
        style::values::computed::OutlineStyle::BorderStyle(line) => {
            !line.none_or_hidden() && outline.outline_width.0.to_f32_px() > 0.0
        }
    };
    let has_shadow = !style.get_effects().box_shadow.0.is_empty();
    has_border || has_padding || has_radius || has_image || has_outline || has_shadow
}

pub fn clamp_hides_lines(style: &style::properties::ComputedValues) -> bool {
    let style_box = style.get_box();
    !style_box.line_clamp.webkit_legacy
        || style_box.overflow_y != style::values::computed::Overflow::Visible
}
