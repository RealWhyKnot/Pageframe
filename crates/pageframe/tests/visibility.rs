mod support;

use pageframe::App;
use support::{HEIGHT, WIDTH, open, pixel, render};

const NESTED: &str = "<!doctype html><style>html,body{margin:0}p{margin:0;font:16px/20px sans-serif}</style>\
<div style=\"visibility:hidden;background:#f00;border:4px solid #f00;padding:5px;width:50px\">\
<div style=\"visibility:visible;background:#00f;height:20px\"></div></div>\
<p><span style=\"visibility:hidden;background:#f00;color:#f00\">hid</span></p>\
<p style=\"visibility:hidden;background:#f00;color:#f00\">ab \
<span style=\"visibility:visible;background:#0f0;color:#000\">cd</span></p>";

const FADED: &str = "<!doctype html><style>html,body{margin:0}</style>\
<div style=\"visibility:hidden;opacity:0.5;background:#000;padding:5px\">\
<div style=\"visibility:visible;background:#00f;height:20px\"></div></div>";

fn on_white(buffer: &[u8], column: usize, row: usize) -> [u8; 3] {
    let [red, green, blue, alpha] = pixel(buffer, column, row);
    let cover = 255 - alpha;
    [
        red.saturating_add(cover),
        green.saturating_add(cover),
        blue.saturating_add(cover),
    ]
}

fn is_red([red, green, blue]: [u8; 3]) -> bool {
    red > 200 && green < 80 && blue < 80
}

fn is_green([red, green, blue]: [u8; 3]) -> bool {
    green > 200 && red < 80 && blue < 80
}

fn is_blue([red, green, blue]: [u8; 3]) -> bool {
    blue > 200 && red < 80 && green < 80
}

fn count(buffer: &[u8], rows: std::ops::Range<usize>, test: impl Fn([u8; 3]) -> bool) -> usize {
    let width = usize::try_from(WIDTH).expect("the width fits");
    rows.flat_map(|row| (0..width).map(move |column| (column, row)))
        .filter(|&(column, row)| test(on_white(buffer, column, row)))
        .count()
}

fn columns(buffer: &[u8], test: impl Fn([u8; 3]) -> bool) -> Vec<usize> {
    let width = usize::try_from(WIDTH).expect("the width fits");
    let height = usize::try_from(HEIGHT).expect("the height fits");
    (0..width)
        .filter(|&column| (0..height).any(|row| test(on_white(buffer, column, row))))
        .collect()
}

#[test]
fn visible_children_of_hidden_boxes_paint_like_chrome() {
    let buffer = render(&mut open(App::new((), NESTED)));
    assert_eq!(on_white(&buffer, 30, 19), [0, 0, 255]);
    assert_eq!(on_white(&buffer, 2, 2), [255, 255, 255]);
    assert_eq!(on_white(&buffer, 6, 6), [255, 255, 255]);
    let height = usize::try_from(HEIGHT).expect("the height fits");
    let red = count(&buffer, 0..height, is_red);
    assert_eq!(red, 0, "hidden boxes and text leave no red behind");
    let green = count(&buffer, 58..78, is_green);
    assert!(
        green > 100,
        "the visible span paints its background, got {green}"
    );
}

#[test]
fn hidden_boxes_still_fade_visible_children_like_chrome() {
    let buffer = render(&mut open(App::new((), FADED)));
    let [red, green, blue] = on_white(&buffer, 30, 15);
    assert!(
        (126..=129).contains(&red) && (126..=129).contains(&green) && blue == 255,
        "the child is half transparent blue, got {:?}",
        [red, green, blue]
    );
    assert_eq!(on_white(&buffer, 2, 2), [255, 255, 255]);
}

const PARTS: &str = "<!doctype html><style>html,body{margin:0}td{padding:0;width:30px;height:14px}</style>\
<table style=\"visibility:hidden;border-collapse:collapse\"><tr style=\"background:#f00\">\
<td style=\"visibility:visible;border:3px solid #00f\"></td><td style=\"border:3px solid #00f\"></td></tr></table>\
<ul style=\"margin:0;padding-left:30px\"><li style=\"visibility:hidden;color:#00f\">\
<span style=\"visibility:visible;color:#0f0\">x</span></li></ul>";

fn underlined(word_style: &str) -> String {
    format!(
        "<!doctype html><style>html,body{{margin:0}}</style>\
<p style=\"margin:0;font:16px/20px sans-serif\">\
<span style=\"text-decoration:underline;text-decoration-color:#f00\">a \
<span style=\"{word_style}\">mmmm</span> b</span></p>"
    )
}

#[test]
fn hidden_tables_and_list_items_keep_their_parts_hidden_like_chrome() {
    let buffer = render(&mut open(App::new((), PARTS)));
    let height = usize::try_from(HEIGHT).expect("the height fits");
    let blue = count(&buffer, 0..height, is_blue);
    assert_eq!(blue, 0, "no collapsed borders and no marker");
    assert_eq!(on_white(&buffer, 15, 10), [255, 0, 0]);
    assert_eq!(on_white(&buffer, 50, 10), [255, 255, 255]);
}

#[test]
fn underlines_skip_hidden_text_like_chrome() {
    let shown = render(&mut open(App::new((), &underlined("color:#00f"))));
    let word = columns(&shown, is_blue);
    let (Some(&word_left), Some(&word_right)) = (word.first(), word.last()) else {
        panic!("the visible word paints blue ink");
    };
    let hidden = render(&mut open(App::new((), &underlined("visibility:hidden"))));
    let line = columns(&hidden, is_red);
    let (gap_left, gap_right) = line
        .windows(2)
        .map(|pair| (pair[0], pair[1]))
        .max_by_key(|(left, right)| right - left)
        .expect("the underline spans at least two columns");
    assert!(
        gap_left < word_left && gap_right > word_right,
        "the gap {gap_left}..{gap_right} covers the hidden word {word_left}..{word_right}"
    );
    assert!(
        gap_right - gap_left < word_right - word_left + 12,
        "the gap {gap_left}..{gap_right} stays near the hidden word {word_left}..{word_right}"
    );
}
