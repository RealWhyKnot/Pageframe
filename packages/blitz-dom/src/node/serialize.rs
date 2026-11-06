//! Serialization of nodes to HTML, and of inline `<svg>` subtrees to SVG source for usvg.

use html_escape::{encode_quoted_attribute_to_string, encode_text_to_string};
use markup5ever::{LocalName, QualName, local_name, ns};
use style_traits::values::ToCss;
use stylo_taffy::CopyStyleGetters;

use super::{Node, NodeData};

#[derive(Clone, Copy)]
enum OutputStyle {
    Normal,
    Pretty,
}

#[derive(Clone, Copy)]
struct SerializeOpts {
    style: OutputStyle,
    /// Replace `currentColor` in attribute values with the element's computed `color`
    resolve_current_color: bool,
    /// Declare the `xlink` prefix on the root element so that the output is well-formed XML
    declare_xlink_ns: bool,
}

impl SerializeOpts {
    const fn html(style: OutputStyle) -> Self {
        Self {
            style,
            resolve_current_color: false,
            declare_xlink_ns: false,
        }
    }
}

const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

/// Writes an attribute's serialized name, per the HTML fragment serialization algorithm
fn write_attr_name(name: &QualName, writer: &mut String) {
    match name.ns {
        ns!() | ns!(html) => {}
        ns!(xml) => writer.push_str("xml:"),
        ns!(xmlns) if name.local.as_str() == "xmlns" => {}
        ns!(xmlns) => writer.push_str("xmlns:"),
        ns!(xlink) => writer.push_str("xlink:"),
        _ => {
            if let Some(prefix) = &name.prefix {
                writer.push_str(prefix);
                writer.push(':');
            }
        }
    }
    writer.push_str(&name.local);
}

fn is_xlink_ns_decl(name: &QualName) -> bool {
    (name.ns == ns!(xmlns) && name.local.as_str() == "xlink")
        || name.local.as_str() == "xmlns:xlink"
}

impl Node {
    /// Renders the HTML of this node and all its children as a `String` without extra whitespace.
    ///
    /// Example output:
    ///
    /// ```text
    /// <html><head /><body><main id="main"><div class="arbitrary-class" /></main></body></html>
    /// ```
    pub fn outer_html(&self) -> String {
        let mut output = String::new();
        self.write_outer_html(&mut output);
        output
    }

    /// Renders the HTML of this node and all its children as a `String` with whitespace for human
    /// readability.
    ///
    /// Example output:
    ///
    /// ```text
    /// <html>
    ///   <head />
    ///   <body>
    ///     <main id="main">
    ///       <div class="arbitrary-class" />
    ///     </main>
    ///   </body>
    /// </html>
    /// ```
    pub fn outer_html_pretty(&self) -> String {
        let mut output = String::new();
        self.write_outer_html_pretty(&mut output);
        output
    }

    pub fn write_outer_html(&self, writer: &mut String) {
        self.write_outer_html_in_style(writer, SerializeOpts::html(OutputStyle::Normal), 0);
    }

    pub fn write_outer_html_pretty(&self, writer: &mut String) {
        self.write_outer_html_in_style(writer, SerializeOpts::html(OutputStyle::Pretty), 0);
    }

    /// Serializes this `<svg>` subtree as a standalone SVG document for usvg, with
    /// `currentColor` resolved against each element's computed `color`.
    #[cfg(feature = "svg")]
    pub(crate) fn svg_source(&self) -> String {
        let mut output = String::new();
        let opts = SerializeOpts {
            style: OutputStyle::Normal,
            resolve_current_color: true,
            declare_xlink_ns: true,
        };
        self.write_outer_html_in_style(&mut output, opts, 0);
        output
    }

    fn write_outer_html_in_style(&self, writer: &mut String, opts: SerializeOpts, nesting: usize) {
        const INDENT: &str = "  ";
        let style = opts.style;
        let has_children = !self.children.is_empty();
        let current_color = opts
            .resolve_current_color
            .then(|| self.primary_styles())
            .flatten()
            .map(|style| style.clone_color())
            .map(|color| color.to_css_string());

        match &self.data {
            NodeData::Document(_) => {}
            NodeData::Comment { .. } => {}
            NodeData::AnonymousBlock(_) => {}
            // NodeData::Doctype { name, .. } => write!(s, "DOCTYPE {name}"),
            NodeData::Text(data) => {
                if matches!(style, OutputStyle::Pretty) {
                    for _ in 0..nesting {
                        writer.push_str(INDENT);
                    }
                }
                let in_raw_text_element = self
                    .parent
                    .and_then(|id| self.tree()[id].data.downcast_element())
                    .is_some_and(|el| {
                        // Documents are parsed with scripting disabled, so `<noscript>` is not raw text
                        el.name.ns == ns!(html)
                            && matches!(
                                el.name.local.as_ref(),
                                "style"
                                    | "script"
                                    | "xmp"
                                    | "iframe"
                                    | "noembed"
                                    | "noframes"
                                    | "plaintext"
                            )
                    });
                if in_raw_text_element {
                    writer.push_str(data.content.as_str());
                } else {
                    encode_text_to_string(data.content.as_str(), writer);
                }
                if matches!(style, OutputStyle::Pretty) {
                    writer.push('\n');
                }
            }
            NodeData::Element(data) => {
                if matches!(style, OutputStyle::Pretty) {
                    for _ in 0..nesting {
                        writer.push_str(INDENT);
                    }
                }
                writer.push('<');
                writer.push_str(&data.name.local);

                if opts.declare_xlink_ns
                    && nesting == 0
                    && !data.attrs().iter().any(|attr| is_xlink_ns_decl(&attr.name))
                {
                    writer.push_str(" xmlns:xlink=\"");
                    writer.push_str(XLINK_NS);
                    writer.push('"');
                }

                let svg_paint = (opts.resolve_current_color && data.name.ns == ns!(svg))
                    .then(|| self.primary_styles())
                    .flatten()
                    .map(|style| {
                        let parent_style = self
                            .parent
                            .map(|parent_id| &self.tree()[parent_id])
                            .filter(|parent| {
                                parent
                                    .element_data()
                                    .is_some_and(|parent| parent.name.ns == ns!(svg))
                            })
                            .and_then(|parent| parent.primary_styles());
                        let style_attr = data.attr(local_name!("style")).unwrap_or_default();
                        svg_paint_css(
                            &style,
                            parent_style.as_ref().map(|style| &***style),
                            |name| {
                                style_attr.contains(name)
                                    || data.attr(LocalName::from(name)).is_some()
                            },
                        )
                    });

                for attr in data.attrs() {
                    if svg_paint.is_some() && attr.name.ns == ns!() && &*attr.name.local == "style"
                    {
                        continue;
                    }
                    writer.push(' ');
                    write_attr_name(&attr.name, writer);
                    writer.push_str("=\"");
                    #[allow(clippy::unnecessary_unwrap)] // Convert to if-let chain once stabilised
                    if current_color.is_some() && attr.value.contains("currentColor") {
                        let value = attr
                            .value
                            .replace("currentColor", current_color.as_ref().unwrap());
                        encode_quoted_attribute_to_string(&value, writer);
                    } else {
                        encode_quoted_attribute_to_string(&attr.value, writer);
                    }
                    writer.push('"');
                }
                if let Some(paint) = &svg_paint {
                    let mut style = data
                        .attrs()
                        .iter()
                        .find(|attr| attr.name.ns == ns!() && &*attr.name.local == "style")
                        .map(|attr| format!("{};", attr.value))
                        .unwrap_or_default();
                    style.push_str(paint);
                    writer.push_str(" style=\"");
                    encode_quoted_attribute_to_string(&style, writer);
                    writer.push('"');
                }
                if !has_children {
                    writer.push_str(" /");
                }
                writer.push('>');
                if matches!(style, OutputStyle::Pretty) {
                    writer.push('\n');
                }

                if has_children {
                    for &child_id in &self.children {
                        self.tree()[child_id].write_outer_html_in_style(writer, opts, nesting + 1);
                    }

                    if matches!(style, OutputStyle::Pretty) {
                        for _ in 0..nesting {
                            writer.push_str(INDENT);
                        }
                    }
                    writer.push_str("</");
                    writer.push_str(&data.name.local);
                    writer.push('>');
                    if matches!(style, OutputStyle::Pretty) {
                        writer.push('\n');
                    }
                }
            }
        }
    }
}

fn svg_paint_css(
    style: &style::properties::ComputedValues,
    parent_style: Option<&style::properties::ComputedValues>,
    is_set_on_element: impl Fn(&str) -> bool,
) -> String {
    let mut css = String::new();
    let parent_values = parent_style.map(svg_paint_values);
    for (index, (name, value)) in svg_paint_values(style).into_iter().enumerate() {
        let Some(value) = value else {
            continue;
        };
        let inherited = parent_values
            .as_ref()
            .is_some_and(|parent| parent[index].1.as_ref() == Some(&value));
        if inherited && !is_set_on_element(name) {
            continue;
        }
        css.push_str(name);
        css.push(':');
        css.push_str(&value);
        css.push(';');
    }
    css
}

fn svg_paint_values(
    style: &style::properties::ComputedValues,
) -> [(&'static str, Option<String>); 11] {
    use style::values::computed::url::ComputedUrl;
    use style::values::generics::svg::{SVGPaintFallback, SVGPaintKind};

    let current_color = style.clone_color();
    let color_css = |color: &style::values::computed::Color| {
        color
            .resolve_to_absolute(&current_color)
            .into_srgb_legacy()
            .to_css_string()
    };
    let paint = |paint: &style::values::computed::SVGPaint| match &paint.kind {
        SVGPaintKind::None => Some("none".to_string()),
        SVGPaintKind::Color(color) => Some(color_css(color)),
        SVGPaintKind::PaintServer(url) => {
            let fragment = match url {
                ComputedUrl::Valid(url) => url.fragment().map(|fragment| {
                    percent_encoding::percent_decode_str(fragment)
                        .decode_utf8_lossy()
                        .into_owned()
                }),
                ComputedUrl::Invalid(url) => url.strip_prefix('#').map(str::to_owned),
            }?;
            let mut css = String::from("url(");
            cssparser::serialize_string(&format!("#{fragment}"), &mut css).ok()?;
            css.push(')');
            match &paint.fallback {
                SVGPaintFallback::None => css.push_str(" none"),
                SVGPaintFallback::Color(color) => {
                    css.push(' ');
                    css.push_str(&color_css(color));
                }
                SVGPaintFallback::Unset => {}
            }
            Some(css)
        }
        SVGPaintKind::ContextFill | SVGPaintKind::ContextStroke => None,
    };

    let svg = style.get_inherited_svg();
    [
        ("fill", paint(&svg.fill)),
        ("stroke", paint(&svg.stroke)),
        ("fill-opacity", Some(svg.fill_opacity.to_css_string())),
        ("fill-rule", Some(svg.fill_rule.to_css_string())),
        ("stroke-width", Some(svg.stroke_width.to_css_string())),
        ("stroke-opacity", Some(svg.stroke_opacity.to_css_string())),
        ("stroke-linecap", Some(svg.stroke_linecap.to_css_string())),
        ("stroke-linejoin", Some(svg.stroke_linejoin.to_css_string())),
        (
            "stroke-miterlimit",
            Some(svg.stroke_miterlimit.to_css_string()),
        ),
        (
            "stroke-dasharray",
            Some(svg.stroke_dasharray.to_css_string()),
        ),
        (
            "stroke-dashoffset",
            Some(svg.stroke_dashoffset.to_css_string()),
        ),
    ]
}
