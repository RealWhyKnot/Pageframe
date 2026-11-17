mod support;

use pageframe::App;
use support::{WIDTH, open, pixel, render};

const MARK: &str = "<span style='background:red'>X</span>";

fn page(rows: &[(&str, String)], colour: &str) -> Vec<u8> {
    let rows: String = rows
        .iter()
        .map(|(style, text)| format!("<div style='{style}'>{text}</div>"))
        .collect();
    let html = format!(
        "<!doctype html><style>html,body{{margin:0;background:white;font:10px/16px Arial;color:{colour};white-space:pre}}</style>{rows}"
    );
    let mut harness = open(App::new((), &html));
    render(&mut harness)
}

fn red_start(buffer: &[u8], row: usize) -> Option<f32> {
    let down = row * 16 + 8;
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    (0..width).find_map(|across| {
        let [red, green, _, _] = pixel(buffer, across, down);
        let cover = 1.0 - f32::from(green) / 255.0;
        let column = f32::from(u16::try_from(across).expect("test columns fit in u16"));
        (red > 200 && cover > 0.05).then_some(column + 1.0 - cover)
    })
}

fn assert_markers_start_at(rows: &[(&str, String)], chrome: &[f32]) {
    let buffer = page(rows, "transparent");
    for (row, ((style, text), expected)) in rows.iter().zip(chrome).enumerate() {
        let start = red_start(&buffer, row).expect("the marker is drawn");
        assert_eq!(
            start.round(),
            *expected,
            "{text:?} with {style:?} starts at {start}"
        );
    }
}

#[test]
fn tabs_reach_the_same_stops_as_chrome() {
    let rows = [
        ("", format!("\t{MARK}")),
        ("", format!("aaaa\t\t{MARK}")),
        ("tab-size:3;letter-spacing:1px", format!("a\t{MARK}")),
        ("text-indent:5px", format!("\t{MARK}")),
        ("tab-size:15px", format!("ab\t{MARK}")),
    ];
    assert_markers_start_at(&rows, &[22.0, 67.0, 11.0, 22.0, 15.0]);
}

#[test]
fn contents_tab_size_and_hanging_tabs_match_chrome() {
    let rows = [
        (
            "",
            format!("<span style='display:contents;tab-size:2'>\t{MARK}</span>"),
        ),
        (
            "white-space:pre-wrap;width:50px",
            "aaaa <span style='background:red'>bbbb</span>\t".to_owned(),
        ),
    ];
    assert_markers_start_at(&rows, &[6.0, 25.0]);
}

#[test]
fn a_tab_draws_no_glyph() {
    let buffer = page(&[("", "\t\t".to_owned())], "black");
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    for down in 0..16 {
        for across in 0..width {
            assert_eq!(
                pixel(&buffer, across, down),
                [255, 255, 255, 255],
                "at {across},{down}"
            );
        }
    }
}
