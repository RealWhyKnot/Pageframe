mod support;

use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use support::{HEIGHT, WIDTH, open, pixel, render};

const OUTSIDE_GAP: usize = 7;
const LINE: usize = 20;
const EDGE: usize = 40;
const RTL_EDGE: usize = 80;

const STYLE: &str = "html,body{margin:0;background:white;font:20px/20px monospace;color:white}\
ul{margin:0;padding:0 0 0 40px;list-style:square}\
li::marker{color:rgb(255,0,0)}\
i{display:inline-block;width:10px;height:10px;background:rgb(0,0,255)}";

#[derive(Debug, Clone, Copy, PartialEq)]
struct Ink {
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
    solid: usize,
}

impl Ink {
    fn width(&self) -> usize {
        self.right - self.left
    }

    fn height(&self) -> usize {
        self.bottom - self.top
    }
}

fn page(body: &str) -> String {
    format!("<html><head><style>{STYLE}</style></head><body>{body}</body></html>")
}

fn frame(harness: &mut Harness<Page<()>>) -> Vec<u8> {
    harness.pump();
    render(harness)
}

fn draw(body: &str) -> Vec<u8> {
    let html = page(body);
    let mut harness = open(App::new((), &html));
    frame(&mut harness)
}

fn size() -> (usize, usize) {
    (
        usize::try_from(WIDTH).expect("test width fits in usize"),
        usize::try_from(HEIGHT).expect("test height fits in usize"),
    )
}

fn is_marker_ink(buffer: &[u8], x: usize, y: usize) -> bool {
    let [red, green, blue, _] = pixel(buffer, x, y);
    red == 255 && green == blue && green < 255
}

fn marker_ink(buffer: &[u8], row: usize) -> Option<Ink> {
    let (width, _) = size();
    let mut ink: Option<Ink> = None;
    for y in row * LINE..(row + 1) * LINE {
        for x in 0..width {
            if !is_marker_ink(buffer, x, y) {
                continue;
            }
            let solid = usize::from(pixel(buffer, x, y)[1] == 0);
            ink = Some(match ink {
                None => Ink {
                    left: x,
                    top: y,
                    right: x + 1,
                    bottom: y + 1,
                    solid,
                },
                Some(found) => Ink {
                    left: found.left.min(x),
                    top: found.top,
                    right: found.right.max(x + 1),
                    bottom: y + 1,
                    solid: found.solid + solid,
                },
            });
        }
    }
    ink
}

fn first_blue_column(buffer: &[u8], y: usize) -> Option<usize> {
    let (width, _) = size();
    (0..width).find(|x| {
        let [red, _, blue, _] = pixel(buffer, *x, y);
        blue == 255 && red == 0
    })
}

fn chrome_size(offset: usize) -> usize {
    offset.div_ceil(2)
}

#[test]
fn outside_square_sits_where_chrome_puts_it() {
    let buffer = draw("<ul><li>x</li></ul>");
    let ink = marker_ink(&buffer, 0).expect("the item has a marker");
    let offset = EDGE - OUTSIDE_GAP - ink.left;
    assert!(offset > 4, "marker offset {offset}");
    assert_eq!(ink.width(), chrome_size(offset));
    assert_eq!(ink.height(), ink.width());
    assert_eq!(ink.solid, ink.width() * ink.height());
}

#[test]
fn padding_and_border_do_not_move_the_marker() {
    let plain = draw("<ul><li>x</li></ul>");
    let padded = draw("<ul><li style='padding-left:15px;border-left:5px solid white'>x</li></ul>");
    assert_eq!(marker_ink(&padded, 0), marker_ink(&plain, 0));
}

#[test]
fn disc_and_circle_fill_the_square_box() {
    let buffer = draw(
        "<ul><li>x</li><li style='list-style-type:disc'>x</li>\
         <li style='list-style-type:circle'>x</li></ul>",
    );
    let square = marker_ink(&buffer, 0).expect("square marker");
    let disc = marker_ink(&buffer, 1).expect("disc marker");
    let circle = marker_ink(&buffer, 2).expect("circle marker");

    assert_eq!((disc.left, disc.right), (square.left, square.right));
    assert_eq!(disc.top, square.top + LINE);
    assert_eq!(disc.height(), square.height());
    assert!(disc.solid < square.solid, "disc corners are rounded");

    let centre_x = square.left + square.width() / 2;
    let centre_y = circle.top + circle.height() / 2;
    assert_eq!(pixel(&buffer, centre_x, centre_y - 2 * LINE)[1], 0);
    assert_eq!(pixel(&buffer, centre_x, centre_y - LINE)[1], 0);
    assert_eq!(pixel(&buffer, centre_x, centre_y)[1], 255);
    assert!(circle.left + 1 >= square.left && circle.right <= square.right + 1);
}

#[test]
fn empty_item_keeps_a_line_for_its_marker() {
    let buffer = draw("<ul><li>x</li><li></li><li>x</li></ul>");
    let first = marker_ink(&buffer, 0).expect("first marker");
    for row in 1..3 {
        let ink = marker_ink(&buffer, row).expect("marker on its own line");
        assert_eq!(ink.top, first.top + row * LINE, "row {row}");
        assert_eq!(
            (ink.left, ink.right),
            (first.left, first.right),
            "row {row}"
        );
    }
}

#[test]
fn rtl_marker_mirrors_to_the_right_edge() {
    let ltr = draw("<ul><li>x</li></ul>");
    let rtl = draw("<ul dir='rtl' style='padding:0 40px 0 0'><li>x</li></ul>");
    let left_ink = marker_ink(&ltr, 0).expect("ltr marker");
    let right_ink = marker_ink(&rtl, 0).expect("rtl marker");
    assert_eq!(right_ink.left - RTL_EDGE, EDGE - left_ink.right);
    assert_eq!(right_ink.width(), left_ink.width());
    assert_eq!(right_ink.top, left_ink.top);
}

#[test]
fn inside_marker_starts_at_the_content_edge() {
    let outside = draw("<ul><li>x</li></ul>");
    let inside = draw("<ul style='list-style-position:inside'><li><i></i></li></ul>");
    let reference = marker_ink(&outside, 0).expect("outside marker");
    let ink = marker_ink(&inside, 0).expect("inside marker");
    assert_eq!(ink.left, EDGE);
    assert_eq!(ink.width(), reference.width());
    assert_eq!(ink.top, reference.top);
    assert_eq!(
        first_blue_column(&inside, ink.top + 2),
        Some(EDGE + ink.width() + LINE)
    );
}

#[test]
fn marker_pseudo_content_replaces_or_removes_the_marker() {
    let removed = draw("<ul><li class='n'>x</li></ul><style>.n::marker{content:none}</style>");
    assert_eq!(marker_ink(&removed, 0), None);

    let text = draw("<ul><li class='t'>x</li></ul><style>.t::marker{content:'\\2588'}</style>");
    let ink = marker_ink(&text, 0).expect("string marker");
    assert_eq!(ink.right, EDGE);
    assert!(
        ink.height() > ink.width(),
        "a full block glyph, not the square"
    );
}

#[test]
fn marker_follows_the_first_line_of_a_block_child() {
    let plain = draw("<ul><li>x</li></ul>");
    let nested = draw("<ul><li><p style='margin:20px 0 0'>x</p></li></ul>");
    let reference = marker_ink(&plain, 0).expect("plain marker");
    let ink = marker_ink(&nested, 1).expect("marker beside the paragraph");
    assert_eq!(marker_ink(&nested, 0), None);
    assert_eq!(ink.top, reference.top + LINE);
    assert_eq!((ink.left, ink.right), (reference.left, reference.right));
}

#[test]
fn min_height_is_not_stretched_by_the_marker_line() {
    let buffer = draw("<ul><li style='min-height:40px'></li><li>x</li></ul>");
    assert!(marker_ink(&buffer, 2).is_some());
    assert_eq!(marker_ink(&buffer, 3), None);
}

#[test]
fn start_reversed_and_value_number_the_items() {
    let numbered = |list: &str| {
        draw(&format!(
            "<style>ol{{margin:0;padding:0 0 0 60px;color:black}}</style>{list}"
        ))
    };
    let plain = numbered("<ol><li>x</li><li>x</li></ol>");
    let started = numbered("<ol start='5'><li>x</li><li>x</li></ol>");
    let valued = numbered("<ol><li value='5'>x</li><li>x</li></ol>");
    let reversed = numbered("<ol reversed><li>x</li><li>x</li></ol>");
    let counted_down = numbered("<ol><li value='2'>x</li><li value='1'>x</li></ol>");
    assert!(started != plain, "start changes the numbers");
    assert!(started == valued, "start and value agree");
    assert!(reversed != plain, "reversed changes the numbers");
    assert!(reversed == counted_down, "reversed counts down to one");
}

#[test]
fn items_in_a_wrapper_count_with_their_list() {
    let numbered = |list: &str| {
        draw(&format!(
            "<style>ol{{margin:0;padding:0 0 0 60px;color:black}}</style>{list}"
        ))
    };
    let plain = numbered("<ol><li>x</li><li>x</li></ol>");
    let started = numbered("<ol start='5'><li>x</li><li>x</li></ol>");
    let reversed = numbered("<ol reversed><li>x</li><li>x</li></ol>");
    assert!(started != plain, "start changes the numbers");
    assert!(
        numbered("<ol><li>x</li><div><li>x</li></div></ol>") == plain,
        "a wrapped second item restarts the count"
    );
    assert!(
        numbered("<ol start='5'><div><div><li>x</li></div></div><li>x</li></ol>") == started,
        "a wrapped first item misses the start"
    );
    assert!(
        numbered("<ol reversed><div><li>x</li></div><li>x</li></ol>") == reversed,
        "a wrapped item is left out of the reversed count"
    );
    assert!(
        numbered("<ol start='5'><div><li>x</li></div><div><li>x</li></div></ol>") == started,
        "each wrapper restarts the count"
    );
    assert!(
        numbered("<ol><li value='5'>x</li><div><li>x</li></div></ol>") == started,
        "a wrapped item does not follow the value before it"
    );
    assert!(
        numbered("<ol><li>x</li><div><li>x<ol><li>y</li></ol></li></div></ol>")
            == numbered("<ol><li>x</li><li>x<ol><li>y</li></ol></li></ol>"),
        "a list nested in a wrapped item is numbered with the outer list"
    );
}

fn marker_box(buffer: &[u8], line_left: usize) -> Option<[usize; 4]> {
    let mut found: Option<[usize; 4]> = None;
    for y in 0..60 {
        for x in line_left..line_left + LINE {
            if is_marker_ink(buffer, x, y) {
                found = Some(
                    found.map_or([x, y, x + 1, y + 1], |[left, top, right, bottom]| {
                        [left.min(x), top.min(y), right.max(x + 1), bottom.max(y + 1)]
                    }),
                );
            }
        }
    }
    found
}

#[test]
fn vertical_list_markers_sit_where_chrome_puts_them() {
    let items = "<ul><li>alpha</li><li>beta</li></ul><ol><li>one</li><li>two</li></ol>";
    let rl_discs = [(68, [75, 22, 80, 27]), (48, [55, 22, 60, 27])];
    let rl_numbers = [(28, [32, 22, 46, 32]), (8, [12, 21, 26, 32])];
    let lr_discs = [(8, [15, 22, 20, 27]), (28, [35, 22, 40, 27])];
    let lr_numbers = [(48, [52, 22, 66, 32]), (68, [72, 21, 86, 32])];
    for (mode, discs, numbers) in [
        ("vertical-rl", rl_discs, rl_numbers),
        ("vertical-lr", lr_discs, lr_numbers),
    ] {
        let html = format!(
            "<html><head><style>html,body{{margin:0;background:#fff;font:16px/20px Arial;\
             color:#fff}}.v{{margin:8px;height:150px;width:max-content}}ul,ol{{margin:0;\
             padding:0;padding-inline-start:30px}}li::marker{{color:rgb(255,0,0)}}</style>\
             </head><body><div class=\"v\" style=\"writing-mode:{mode}\">{items}</div></body>\
             </html>"
        );
        let mut harness = open(App::new((), &html));
        let buffer = frame(&mut harness);
        for (line_left, chrome) in discs {
            let found = marker_box(&buffer, line_left);
            assert_eq!(found, Some(chrome), "{mode} disc at {line_left}");
        }
        for (line_left, chrome) in numbers {
            let found = marker_box(&buffer, line_left).expect("the item has a number");
            for (edge, expected) in found.into_iter().zip(chrome) {
                let message = format!("{mode} number at {line_left}: {found:?}");
                assert!(edge.abs_diff(expected) <= 1, "{message}");
            }
        }
    }
}

#[test]
fn vertical_discs_follow_chrome_inside_empty_and_off_the_pixel_grid() {
    for (box_style, list_style, items, chrome) in [
        (
            "",
            "list-style-position:inside",
            "<li>alpha</li><li>beta</li>",
            [(28, [35, 38, 40, 43]), (8, [15, 38, 20, 43])],
        ),
        (
            "",
            "",
            "<li></li><li>beta</li>",
            [(28, [35, 22, 40, 27]), (8, [15, 22, 20, 27])],
        ),
        (
            "margin-top:8.4px",
            "",
            "<li>alpha</li>",
            [(8, [15, 22, 20, 27]); 2],
        ),
        (
            "margin-top:8.6px",
            "",
            "<li>alpha</li>",
            [(8, [15, 23, 20, 28]); 2],
        ),
    ] {
        let html = format!(
            "<html><head><style>html,body{{margin:0;background:#fff;font:16px/20px Arial;\
             color:#fff}}.v{{margin:8px;height:150px;width:max-content;\
             writing-mode:vertical-rl}}ul{{margin:0;padding:0;padding-inline-start:30px}}\
             li::marker{{color:rgb(255,0,0)}}</style></head><body><div class=\"v\" \
             style=\"{box_style}\"><ul style=\"{list_style}\">{items}</ul></div></body></html>"
        );
        let mut harness = open(App::new((), &html));
        let buffer = frame(&mut harness);
        for (line_left, expected) in chrome {
            let found = marker_box(&buffer, line_left);
            assert_eq!(found, Some(expected), "{box_style} {list_style} {items}");
        }
    }
}
