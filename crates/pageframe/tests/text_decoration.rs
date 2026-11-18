mod scaled;
mod support;

use pageframe::App;
use support::{open, pixel, render};

const BASE: &str =
    "html,body{margin:0;background:white;font:10px/20px monospace;color:transparent}";
const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn draw(body: &str) -> Vec<u8> {
    let html = format!("<html><head><style>{BASE}</style></head><body>{body}</body></html>");
    let mut harness = open(App::new((), &html));
    harness.pump();
    render(&mut harness)
}

fn column(buffer: &[u8], across: usize) -> Vec<(usize, [u8; 4])> {
    (0..60)
        .map(|down| (down, pixel(buffer, across, down)))
        .filter(|(_, colour)| *colour != WHITE)
        .collect()
}

fn red_columns(buffer: &[u8]) -> Vec<usize> {
    let row = (0..60).find(|down| (0..80).any(|across| pixel(buffer, across, *down) == RED));
    row.map(|down| {
        (0..80)
            .filter(|across| pixel(buffer, *across, down) == RED)
            .collect()
    })
    .unwrap_or_default()
}

#[test]
fn decoration_lines_fill_whole_pixel_rows() {
    for (size, rows) in [(10, 1), (16, 1), (20, 2), (32, 3)] {
        for line in ["underline", "line-through", "overline"] {
            let buffer = draw(&format!(
                "<div style=\"margin-top:0.4px;font-size:{size}px;line-height:50px;\
                 text-decoration:{line} red\">aaa</div>"
            ));
            let colours: Vec<[u8; 4]> = column(&buffer, 4)
                .iter()
                .map(|(_, colour)| *colour)
                .collect();
            assert_eq!(colours, vec![RED; rows], "{line} at {size}px");
        }
    }
}

#[test]
fn an_underline_starts_half_its_thickness_below_the_baseline() {
    for (size, gap) in [(10, 1), (20, 1), (32, 2)] {
        let buffer = draw(&format!(
            "<div style=\"font-size:{size}px;line-height:50px;text-decoration:underline red\">aaa             <span style=\"display:inline-block;width:6px;height:5px;background:blue\"></span></div>"
        ));
        let block: Vec<usize> = (0..120)
            .filter(|across| {
                column(&buffer, *across)
                    .iter()
                    .any(|(_, colour)| *colour == BLUE)
            })
            .collect();
        let across = *block.get(2).expect("the inline-block is painted");
        let last_blue = column(&buffer, across)
            .iter()
            .filter(|(_, colour)| *colour == BLUE)
            .map(|(down, _)| *down)
            .max();
        let first_red = column(&buffer, 4)
            .iter()
            .find(|(_, colour)| *colour == RED)
            .map(|(down, _)| *down);
        assert_eq!(first_red, last_blue.map(|down| down + 1 + gap), "{size}px");
    }
}

fn line_gaps(style: &str) -> usize {
    let columns = red_columns(&draw(&format!(
        "<div style=\"font-size:32px;line-height:50px;{style}\">aga</div>"
    )));
    assert!(columns.len() > 20, "{style} paints a line");
    columns
        .windows(2)
        .filter(|pair| pair[1] != pair[0] + 1)
        .count()
}

#[test]
fn an_underline_breaks_around_a_descender() {
    assert_eq!(line_gaps("text-decoration:underline red"), 1);
}

#[test]
fn an_underline_with_skip_ink_none_is_unbroken() {
    assert_eq!(
        line_gaps("text-decoration:underline red;text-decoration-skip-ink:none"),
        0
    );
}

#[test]
fn a_line_through_is_never_broken() {
    assert_eq!(line_gaps("text-decoration:line-through red"), 0);
}

#[test]
fn an_underline_on_a_padded_span_covers_its_text_only() {
    let plain = red_columns(&draw(
        "<div><span style=\"text-decoration:underline red\">aa</span></div>",
    ));
    let padded = red_columns(&draw(
        "<div><span style=\"padding:0 10px;text-decoration:underline red\">aa</span></div>",
    ));
    assert_eq!(plain.first(), Some(&0));
    assert_eq!(padded.first(), Some(&10));
    assert_eq!(padded.len(), plain.len());
}

#[test]
fn a_parent_underline_skips_a_child_span_padding() {
    let plain = red_columns(&draw(
        "<div><u style=\"text-decoration-color:red\"><span>aa</span></u></div>",
    ));
    let padded = red_columns(&draw(
        "<div><u style=\"text-decoration-color:red\">\
         <span style=\"padding:0 10px\">aa</span></u></div>",
    ));
    assert_eq!(padded.first(), Some(&10));
    assert_eq!(padded.len(), plain.len());
}

fn wavy_ink(thickness: &str) -> Vec<Vec<u8>> {
    let buffer = draw(&format!(
        "<div style=\"font:16px Arial;white-space:pre;text-decoration:underline wavy black \
         {thickness}px;text-decoration-skip-ink:none\">|                    |</div>"
    ));
    (0..support::WIDTH)
        .map(|across| {
            (0..40)
                .map(|down| 255 - pixel(&buffer, usize::try_from(across).expect("fits"), down)[0])
                .collect()
        })
        .collect()
}

#[test]
fn wavy_underlines_repeat_and_sit_where_chrome_draws_them() {
    for (thickness, period, rows) in [("1", 7, 15..=19), ("1.5", 11, 15..=21), ("3", 15, 16..=24)] {
        let ink = wavy_ink(thickness);
        for across in 20..40 {
            let drift = ink[across]
                .iter()
                .zip(&ink[across + period])
                .map(|(here, there)| here.abs_diff(*there))
                .max();
            assert!(
                drift <= Some(2),
                "{thickness}px wave doesn't repeat every {period}px at column {across}"
            );
        }
        let inked: Vec<usize> = (0..40)
            .filter(|&down| (20..60).any(|across| ink[across][down] > 12))
            .collect();
        assert_eq!(
            (inked.first(), inked.last()),
            (Some(rows.start()), Some(rows.end())),
            "{thickness}px wave rows"
        );
    }
}

#[test]
fn one_pixel_wavy_underlines_are_hairlines_like_chrome() {
    let ink = wavy_ink("1");
    let total: u32 = (20..62)
        .flat_map(|across| ink[across].iter().map(|value| u32::from(*value)))
        .sum();
    let per_column = f64::from(total) / 255.0 / 42.0;
    assert!(
        (0.85..1.15).contains(&per_column),
        "a 1px wave carries {per_column} px of ink per column"
    );
}

fn ink_rows(body: &str, columns: std::ops::Range<usize>) -> Vec<(usize, Vec<u8>)> {
    let buffer = draw(body);
    (0..60)
        .map(|down| {
            let ink: Vec<u8> = columns
                .clone()
                .map(|across| 255 - pixel(&buffer, across, down)[0])
                .collect();
            (down, ink)
        })
        .filter(|(_, ink)| ink.iter().any(|value| *value > 0))
        .collect()
}

fn assert_ink_near(
    found: &[(usize, Vec<u8>)],
    chrome: &[(usize, &[u8])],
    tolerance: u8,
    what: &str,
) {
    let found_rows: Vec<usize> = found.iter().map(|(down, _)| *down).collect();
    let chrome_rows: Vec<usize> = chrome.iter().map(|(down, _)| *down).collect();
    assert_eq!(found_rows, chrome_rows, "{what} rows");
    for ((down, ink), (_, expected)) in found.iter().zip(chrome) {
        for (across, (value, wanted)) in ink.iter().zip(expected.iter()).enumerate() {
            assert!(
                value.abs_diff(*wanted) <= tolerance,
                "{what} at column {across} row {down}: {value}, Chrome {wanted}"
            );
        }
    }
}

fn decorated(font: &str, decoration: &str, spaces: usize) -> String {
    let text = format!("|{}|", " ".repeat(spaces));
    format!(
        "<div style=\"font:{font} Arial;white-space:pre;text-decoration:{decoration};\
         text-decoration-skip-ink:none\">{text}</div>"
    )
}

#[test]
fn dashed_underlines_cover_quarter_pixel_rows_like_chrome() {
    let found = ink_rows(&decorated("16px", "underline dashed black", 20), 4..28);
    let chrome: &[u8] = &[
        192, 190, 0, 0, 0, 0, 171, 192, 192, 192, 192, 192, 21, 0, 0, 0, 150, 192, 192, 192, 192,
        192, 42, 0,
    ];
    assert_ink_near(&found, &[(14, chrome), (15, chrome)], 2, "1.6px dashes");
}

#[test]
fn dotted_underlines_are_square_dots_like_chrome() {
    let found = ink_rows(&decorated("16px", "underline dotted black", 20), 4..28);
    let chrome: &[u8] = &[
        205, 205, 0, 0, 205, 205, 0, 0, 205, 205, 0, 0, 205, 205, 0, 0, 205, 205, 0, 0, 205, 205,
        0, 0,
    ];
    assert_ink_near(&found, &[(14, chrome), (15, chrome)], 2, "1.6px dots");
}

#[test]
fn thick_dotted_underlines_are_round_dots_like_chrome() {
    let found = ink_rows(&decorated("40px", "underline dotted black", 8), 0..24);
    let edge: &[u8] = &[
        84, 216, 216, 84, 0, 0, 0, 0, 72, 208, 224, 95, 0, 0, 0, 0, 61, 203, 228, 106, 0, 0, 0, 0,
    ];
    let middle: &[u8] = &[
        213, 255, 255, 213, 0, 0, 0, 0, 196, 255, 255, 231, 1, 0, 0, 0, 179, 255, 255, 242, 6, 0,
        0, 0,
    ];
    assert_ink_near(
        &found,
        &[(38, edge), (39, middle), (40, middle), (41, edge)],
        20,
        "4px dots",
    );
}

#[test]
fn short_thick_dotted_underlines_draw_one_dot_two_dots_or_a_capsule_like_chrome() {
    let short = |spacing: i32| {
        format!(
            "<div style=\"font:40px/50px Arial;white-space:pre;letter-spacing:{spacing}px;\
             text-decoration:underline dotted black;text-decoration-skip-ink:none\">|</div>"
        )
    };
    let dot_edge: &[u8] = &[86, 216, 216, 86, 0, 0, 0, 0, 0];
    let dot_middle: &[u8] = &[216, 255, 255, 216, 0, 0, 0, 0, 0];
    assert_ink_near(
        &ink_rows(&short(-3), 0..9),
        &[
            (41, dot_edge),
            (42, dot_middle),
            (43, dot_middle),
            (44, dot_edge),
        ],
        20,
        "a line shorter than two dots",
    );
    let capsule_edge: &[u8] = &[86, 216, 255, 255, 255, 255, 216, 86, 0];
    let capsule_middle: &[u8] = &[216, 255, 255, 255, 255, 255, 255, 216, 0];
    assert_ink_near(
        &ink_rows(&short(-2), 0..9),
        &[
            (41, capsule_edge),
            (42, capsule_middle),
            (43, capsule_middle),
            (44, capsule_edge),
        ],
        20,
        "a line exactly two dots long",
    );
    let dots_edge: &[u8] = &[84, 216, 216, 84, 0, 86, 219, 213, 81];
    let dots_middle: &[u8] = &[212, 255, 255, 212, 0, 219, 255, 255, 210];
    assert_ink_near(
        &ink_rows(&short(-1), 0..9),
        &[
            (41, dots_edge),
            (42, dots_middle),
            (43, dots_middle),
            (44, dots_edge),
        ],
        20,
        "a line between two and three dots long",
    );
}

#[test]
fn fractional_decoration_thickness_rounds_to_whole_pixels_like_chrome() {
    let found = ink_rows(
        &(decorated("16px", "underline black 1.5px", 8)
            + &decorated("16px", "underline black 1.25px", 8)),
        4..20,
    );
    let full: &[u8] = &[255; 16];
    assert_ink_near(
        &found,
        &[(15, full), (16, full), (33, full)],
        0,
        "1.5px and 1.25px lines",
    );
}

fn inked_rows(body: &str) -> (Option<usize>, Option<usize>) {
    let rows: Vec<usize> = ink_rows(body, 20..60)
        .into_iter()
        .filter(|(_, ink)| ink.iter().any(|value| *value > 12))
        .map(|(down, _)| down)
        .collect();
    (rows.first().copied(), rows.last().copied())
}

#[test]
fn waves_sit_where_chrome_draws_them_for_every_line_kind() {
    let spacer = "<div style=\"height:10px\"></div>";
    for (body, rows, what) in [
        (
            decorated("13px", "underline wavy black", 20),
            (13, 17),
            "13px underline",
        ),
        (
            format!(
                "{spacer}{}",
                decorated("16px", "overline wavy black 2px", 20)
            ),
            (2, 8),
            "2px overline",
        ),
        (
            decorated("16px", "line-through wavy black 2px", 20),
            (5, 11),
            "2px line-through",
        ),
    ] {
        assert_eq!(inked_rows(&body), (Some(rows.0), Some(rows.1)), "{what}");
    }
}

#[test]
fn double_lines_leave_one_device_pixel_between_them_like_chrome() {
    let html = format!(
        "<html><head><style>{BASE}</style></head><body><div style=\"font-size:16px;\
         line-height:40px;text-decoration:underline double red 2px\">aaa</div></body></html>"
    );
    for scale in [1, 2] {
        let mut harness = open(App::new((), &html));
        harness.pump();
        let buffer = scaled::render_at_scale(&mut harness, scale);
        let width =
            usize::try_from(support::WIDTH).expect("test width fits in usize") * usize::from(scale);
        let inked: Vec<usize> = (0..80 * usize::from(scale))
            .filter(|down| {
                let start = (down * width + 4) * 4;
                buffer[start..start + 4] != WHITE
            })
            .collect();
        let thick = 2 * usize::from(scale);
        let first = inked[0];
        let expected: Vec<usize> = (first..first + thick)
            .chain(first + thick + 1..first + 2 * thick + 1)
            .collect();
        assert_eq!(inked, expected, "at {scale}x");
    }
}
