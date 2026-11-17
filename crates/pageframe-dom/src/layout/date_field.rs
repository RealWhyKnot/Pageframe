use markup5ever::local_name;
use style::properties::style_structs::Font as FontStyles;
use style::values::computed::font::LineHeight;
use style_dom::ElementState;
use stylo_taffy::CopyStyleGetters;
use taffy::{CoreStyle, LayoutInput, LayoutOutput, ResolveOrZero, Size, compute_leaf_layout};

use crate::layout::resolve_calc_value;
use crate::node::{DateField, DateIndicator, Node, SpecialElementData, TextLayout};
use crate::{BaseDocument, NodeId};

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const YEAR_MAX: &str = "275760";
const FIELD_PADDING: f32 = DateField::FIELD_PADDING;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Date,
    Time,
    DateTime,
    Month,
    Week,
}

impl Kind {
    fn from_type(input_type: &str) -> Option<Self> {
        match input_type {
            "date" => Some(Self::Date),
            "time" => Some(Self::Time),
            "datetime-local" => Some(Self::DateTime),
            "month" => Some(Self::Month),
            "week" => Some(Self::Week),
            _ => None,
        }
    }
}

enum Part {
    Field { shown: String, widest: Vec<String> },
    Text(&'static str),
}

fn field(value: Option<String>, placeholder: &str, widest: &[&str]) -> Part {
    let mut candidates: Vec<String> = widest.iter().map(|text| (*text).to_owned()).collect();
    candidates.push(placeholder.to_owned());
    let shown = value.unwrap_or_else(|| placeholder.to_owned());
    candidates.push(shown.clone());
    Part::Field {
        shown,
        widest: candidates,
    }
}

fn digits(text: &str, min: usize, max: usize) -> Option<u32> {
    let valid = (min..=max).contains(&text.len()) && text.bytes().all(|byte| byte.is_ascii_digit());
    valid.then(|| text.parse().ok()).flatten()
}

fn is_leap(year: u32) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn parse_year(text: &str) -> Option<u32> {
    digits(text, 4, 6).filter(|year| *year > 0)
}

fn parse_month(text: &str) -> Option<(u32, u32)> {
    let (year, month) = text.rsplit_once('-')?;
    let month = digits(month, 2, 2).filter(|month| (1..=12).contains(month))?;
    Some((parse_year(year)?, month))
}

fn parse_date(text: &str) -> Option<(u32, u32, u32)> {
    let (month_text, day) = text.rsplit_once('-')?;
    let (year, month) = parse_month(month_text)?;
    let day = digits(day, 2, 2).filter(|day| (1..=days_in_month(year, month)).contains(day))?;
    Some((year, month, day))
}

fn weekday_of_january_first(year: u32) -> u32 {
    let previous = year - 1;
    (1 + 5 * (previous % 4) + 4 * (previous % 100) + 6 * (previous % 400)) % 7
}

fn weeks_in_year(year: u32) -> u32 {
    let first = weekday_of_january_first(year);
    if first == 4 || (first == 3 && is_leap(year)) {
        53
    } else {
        52
    }
}

fn parse_week(text: &str) -> Option<(u32, u32)> {
    let (year, week) = text.split_once("-W")?;
    let year = parse_year(year)?;
    let week = digits(week, 2, 2).filter(|week| (1..=weeks_in_year(year)).contains(week))?;
    Some((year, week))
}

struct Time {
    hour: u32,
    minute: u32,
    second: u32,
    millisecond: Option<u32>,
}

fn parse_time(text: &str) -> Option<Time> {
    let mut pieces = text.splitn(3, ':');
    let hour = digits(pieces.next()?, 2, 2).filter(|hour| *hour < 24)?;
    let minute = digits(pieces.next()?, 2, 2).filter(|minute| *minute < 60)?;
    let (second, millisecond) = match pieces.next() {
        None => (0, None),
        Some(rest) => {
            let (second, fraction) = match rest.split_once('.') {
                Some((second, fraction)) => (second, Some(fraction)),
                None => (rest, None),
            };
            let second = digits(second, 2, 2).filter(|second| *second < 60)?;
            let millisecond = match fraction {
                Some(fraction) => {
                    let value = digits(fraction, 1, 3)?;
                    let padding = 3 - u32::try_from(fraction.len()).ok()?;
                    Some(value * 10_u32.pow(padding))
                }
                None => None,
            };
            (second, millisecond)
        }
    };
    Some(Time {
        hour,
        minute,
        second,
        millisecond,
    })
}

fn year_text(year: u32) -> String {
    format!("{year:04}")
}

fn date_parts(parts: &mut Vec<Part>, date: Option<(u32, u32, u32)>) {
    parts.push(field(
        date.map(|(_, month, _)| format!("{month:02}")),
        "mm",
        &["12"],
    ));
    parts.push(Part::Text("/"));
    parts.push(field(
        date.map(|(_, _, day)| format!("{day:02}")),
        "dd",
        &["31"],
    ));
    parts.push(Part::Text("/"));
    parts.push(field(
        date.map(|(year, _, _)| year_text(year)),
        "yyyy",
        &[YEAR_MAX],
    ));
}

fn time_parts(parts: &mut Vec<Part>, time: Option<&Time>, step: Option<f64>) {
    let step_seconds = step.unwrap_or(60.0);
    let fraction_step = step_seconds.fract() != 0.0;
    let has_millisecond =
        fraction_step || time.is_some_and(|time| time.millisecond.is_some_and(|ms| ms != 0));
    let has_second =
        has_millisecond || step_seconds % 60.0 != 0.0 || time.is_some_and(|time| time.second != 0);
    parts.push(field(
        time.map(|time| {
            let hour = match time.hour % 12 {
                0 => 12,
                hour => hour,
            };
            format!("{hour:02}")
        }),
        "--",
        &["12"],
    ));
    parts.push(Part::Text(":"));
    parts.push(field(
        time.map(|time| format!("{:02}", time.minute)),
        "--",
        &["59"],
    ));
    if has_second {
        parts.push(Part::Text(":"));
        parts.push(field(
            time.map(|time| format!("{:02}", time.second)),
            "--",
            &["59"],
        ));
    }
    if has_millisecond {
        parts.push(Part::Text("."));
        parts.push(field(
            time.map(|time| format!("{:03}", time.millisecond.unwrap_or(0))),
            "---",
            &["999"],
        ));
    }
    parts.push(Part::Text(" "));
    parts.push(field(
        time.map(|time| if time.hour < 12 { "AM" } else { "PM" }.to_owned()),
        "--",
        &["AM", "PM"],
    ));
}

fn parts_for(kind: Kind, value: &str, step: Option<f64>) -> Vec<Part> {
    let mut parts = Vec::new();
    match kind {
        Kind::Date => date_parts(&mut parts, parse_date(value)),
        Kind::Time => time_parts(&mut parts, parse_time(value).as_ref(), step),
        Kind::DateTime => {
            let parsed = value
                .split_once(['T', ' '])
                .and_then(|(date, time)| Some((parse_date(date)?, parse_time(time)?)));
            date_parts(&mut parts, parsed.as_ref().map(|(date, _)| *date));
            parts.push(Part::Text(" "));
            time_parts(&mut parts, parsed.as_ref().map(|(_, time)| time), step);
        }
        Kind::Month => {
            let month = parse_month(value);
            let longest = MONTHS.iter().map(|name| name.len()).max().unwrap_or(0);
            parts.push(field(
                month.and_then(|(_, month)| {
                    let index = usize::try_from(month).ok()?.checked_sub(1)?;
                    MONTHS.get(index).map(|name| (*name).to_owned())
                }),
                &"-".repeat(longest),
                &MONTHS,
            ));
            parts.push(Part::Text(" "));
            parts.push(field(
                month.map(|(year, _)| year_text(year)),
                "----",
                &[YEAR_MAX],
            ));
        }
        Kind::Week => {
            let week = parse_week(value);
            parts.push(Part::Text("Week "));
            parts.push(field(
                week.map(|(_, week)| format!("{week:02}")),
                "--",
                &["53"],
            ));
            parts.push(Part::Text(", "));
            parts.push(field(
                week.map(|(year, _)| year_text(year)),
                "----",
                &[YEAR_MAX],
            ));
        }
    }
    parts
}

fn edit_width(
    doc: &BaseDocument,
    font_styles: &FontStyles,
    font_size: f32,
    parts: &[Part],
) -> Option<f32> {
    let mut fonts = doc.font_ctx.lock().ok()?;
    let mut total = 0.0;
    for part in parts {
        total += match part {
            Part::Text(text) => {
                crate::font_metrics::text_advances(&mut fonts, font_styles, font_size, &[text])
                    .into_iter()
                    .sum()
            }
            Part::Field { widest, .. } => {
                let texts: Vec<&str> = widest.iter().map(String::as_str).collect();
                crate::font_metrics::text_advances(&mut fonts, font_styles, font_size, &texts)
                    .into_iter()
                    .fold(0.0, f32::max)
                    + FIELD_PADDING * 2.0
            }
        };
    }
    Some(total.ceil())
}

pub(crate) fn prepare(doc: &mut BaseDocument, node_id: NodeId) {
    let node = &doc.nodes[node_id];
    let Some(element) = node.element_data() else {
        return;
    };
    let Some(kind) = element.input_type().as_deref().and_then(Kind::from_type) else {
        return;
    };
    let Some(styles) = node.primary_styles() else {
        return;
    };
    let value = element.attr(local_name!("value")).unwrap_or("");
    let step = element
        .attr(local_name!("step"))
        .and_then(|step| step.trim().parse::<f64>().ok())
        .filter(|step| step.is_finite() && *step > 0.0);
    let parts = parts_for(kind, value, step);
    let hidden = element.element_state.contains(ElementState::DISABLED)
        || element.has_attr(local_name!("readonly"));
    let font_size = styles.slow_clone_font_size().used_size().px();
    let scale = doc.viewport.scale();
    let Some(edit_width) = edit_width(doc, styles.get_font(), font_size, &parts) else {
        return;
    };
    let Some((line_height, baseline)) = line_metrics(doc, styles.get_font(), font_size, &styles)
    else {
        return;
    };
    let indicator = match kind {
        Kind::Time => DateIndicator {
            clock: true,
            visible: !hidden,
            size: font_size * 1.05,
            padding: 3.0,
            margin: 8.0,
        },
        _ => DateIndicator {
            clock: false,
            visible: !hidden,
            size: font_size,
            padding: 2.0,
            margin: 0.0,
        },
    };

    let mut text_style = crate::stylo_to_parley::style(node_id, &styles);
    text_style.line_height = parley::LineHeight::Absolute(line_height);
    drop(styles);
    let mut layout = TextLayout::new();
    let Ok(mut fonts) = doc.font_ctx.lock() else {
        return;
    };
    let mut builder = doc
        .layout_ctx
        .tree_builder(&mut fonts, scale, true, &text_style);
    let padding_box = parley::InlineBox {
        id: node_id.as_u64(),
        kind: parley::InlineBoxKind::InFlow,
        index: 0,
        width: FIELD_PADDING * scale,
        height: 0.0,
        baseline: None,
        vertical_align: parley::VerticalAlign::BASELINE,
    };
    for part in &parts {
        match part {
            Part::Text(text) => builder.push_text(text),
            Part::Field { shown, .. } => {
                builder.push_inline_box(padding_box.clone());
                builder.push_text(shown);
                builder.push_inline_box(padding_box.clone());
            }
        }
    }
    layout.text = builder.build_into(&mut layout.layout);
    drop(fonts);
    layout.layout.break_all_lines(None);
    if let Some(element) = doc.nodes[node_id].element_data_mut() {
        element.special_data = SpecialElementData::DateField(Box::new(DateField {
            layout,
            edit_width,
            line_height,
            baseline,
            indicator,
        }));
    }
}

fn line_metrics(
    doc: &BaseDocument,
    font_styles: &FontStyles,
    font_size: f32,
    styles: &style::properties::ComputedValues,
) -> Option<(f32, f32)> {
    let scale = doc.viewport.scale();
    let metrics = crate::font_metrics::form_control_metrics(
        &mut *doc.font_ctx.lock().ok()?,
        font_styles,
        font_size,
        scale,
    )?;
    let ascent = metrics.ascent / scale;
    let descent = metrics.descent / scale;
    let line_height = match styles.clone_line_height() {
        LineHeight::Normal => metrics.normal_line_height / scale,
        LineHeight::Number(number) => font_size * number.0,
        LineHeight::Length(value) => value.0.px(),
    };
    let baseline = match styles.clone_line_height() {
        LineHeight::Normal => {
            (metrics.ascent.round()
                + ((metrics.normal_line_height - metrics.ascent.round() - metrics.descent.round())
                    / 2.0)
                    .floor())
                / scale
        }
        _ => (line_height - ascent - descent) / 2.0 + ascent,
    };
    Some((line_height, baseline))
}

pub(crate) fn layout(node: &Node, inputs: LayoutInput) -> LayoutOutput {
    let Some(SpecialElementData::DateField(field)) =
        node.element_data().map(|element| &element.special_data)
    else {
        return LayoutOutput::HIDDEN;
    };
    let width = field.edit_width + field.indicator_width();
    let height = field.content_height();
    let style = node.layout_style();
    let mut output = compute_leaf_layout(inputs, &style, resolve_calc_value, |_, _| Size {
        width,
        height,
    });
    let basis = inputs.parent_size.width;
    let border = style.border().resolve_or_zero(basis, resolve_calc_value);
    let padding = style.padding().resolve_or_zero(basis, resolve_calc_value);
    let content_top = border.top + padding.top;
    let content = output.size.height - content_top - border.bottom - padding.bottom;
    let baseline = content_top + field.baseline_in(content);
    output.baselines.first = Some(baseline);
    output.baselines.last = Some(baseline);
    output
}
