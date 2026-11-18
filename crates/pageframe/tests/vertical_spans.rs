mod support;

use pageframe::App;
use support::{HEIGHT, open, pixel, render};

const SPANS: &str = "<html><head><style>html,body{margin:0;background:#fff;font:14px/24px Arial}\
    div{position:absolute;top:4px;height:70px}\
    span{padding:4px 0 6px;border-top:3px solid #f00;border-bottom:5px solid #00f;\
    background:#0f0}</style></head><body>\
    <div style='left:4px;writing-mode:vertical-rl'><span>ab</span></div>\
    <div style='left:44px;writing-mode:vertical-lr'><span>ab</span></div>\
    <div style='left:84px;writing-mode:vertical-rl;direction:rtl'><span>ab</span></div>\
    </body></html>";

const CLONED: &str = "<html><head><style>html,body{margin:0;background:#fff;font:14px/24px Arial}\
    span{background:#0f0}</style></head><body>\
    <section style='position:absolute;left:4px;top:4px;writing-mode:vertical-rl;width:100px;\
    height:60px'>ab <span style='box-decoration-break:clone;padding:2px 4px 6px 8px'>\
    cd ef gh ij</span></section></body></html>";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Paint {
    Red,
    Blue,
    Other,
}

fn rows_down(buffer: &[u8], column: usize) -> Vec<(u32, Paint)> {
    (0..HEIGHT)
        .filter_map(|row| {
            let index = usize::try_from(row).expect("rows fit in usize");
            let paint = match pixel(buffer, column, index) {
                [red, green, blue, _] if red > 200 && green < 60 && blue < 60 => Paint::Red,
                [red, green, blue, _] if blue > 200 && red < 60 && green < 60 => Paint::Blue,
                _ => Paint::Other,
            };
            (paint != Paint::Other).then_some((row, paint))
        })
        .collect()
}

fn border_rows(buffer: &[u8], column: usize, paint: Paint) -> Vec<u32> {
    rows_down(buffer, column)
        .into_iter()
        .filter(|&(_, found)| found == paint)
        .map(|(row, _)| row)
        .collect()
}

#[test]
fn vertical_span_borders_follow_the_physical_sides_like_chrome() {
    let mut harness = open(App::new((), SPANS));
    let buffer = render(&mut harness);
    for column in [9, 49] {
        assert_eq!(
            border_rows(&buffer, column, Paint::Red),
            [4, 5, 6],
            "column {column}"
        );
        assert_eq!(
            border_rows(&buffer, column, Paint::Blue),
            [33, 34, 35, 36, 37],
            "column {column}"
        );
    }
    let red = border_rows(&buffer, 89, Paint::Red);
    let blue = border_rows(&buffer, 89, Paint::Blue);
    assert_eq!((red.len(), blue.len()), (3, 5), "{red:?} {blue:?}");
    assert!(red[2] < blue[0], "{red:?} {blue:?}");
    assert_eq!(blue[4], 73, "{blue:?}");
}

fn green_rows(buffer: &[u8], column: usize) -> Vec<u32> {
    (0..HEIGHT)
        .filter(|&row| {
            let index = usize::try_from(row).expect("rows fit in usize");
            let [red, green, blue, _] = pixel(buffer, column, index);
            green > 200 && red < 80 && blue < 80
        })
        .collect()
}

#[test]
fn cloned_vertical_spans_repeat_the_block_side_padding_like_chrome() {
    let mut harness = open(App::new((), CLONED));
    let buffer = render(&mut harness);
    let first = green_rows(&buffer, 90);
    let second = green_rows(&buffer, 66);
    assert_eq!(
        [first.first(), first.last(), second.first(), second.last()],
        [Some(&23), Some(&61), Some(&4), Some(&37)],
        "{first:?} {second:?}"
    );
}
