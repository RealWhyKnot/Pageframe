mod support;

use blitz_test_harness::Harness;
use pageframe::{App, Page};
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
