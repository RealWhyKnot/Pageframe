mod support;

use pageframe::App;
use support::{HEIGHT, open, pixel, render};

fn letter_columns(size: u32, style: &str) -> Vec<Vec<u8>> {
    let html = format!(
        "<html><head><style>html,body{{margin:0;background:#fff}}\
         div{{position:absolute;left:4px;top:4px;font:{size}px/30px Arial;{style}}}</style></head>\
         <body><div>o</div></body></html>"
    );
    let mut harness = open(App::new((), &html));
    let buffer = render(&mut harness);
    (0..40)
        .map(|column| {
            (0..HEIGHT)
                .map(|row| ink(&buffer, column, row))
                .filter(|&value| value > 8)
                .collect()
        })
        .filter(|inked: &Vec<u8>| !inked.is_empty())
        .collect()
}

fn ink(buffer: &[u8], column: usize, row: u32) -> u8 {
    let row = usize::try_from(row).expect("rows fit in usize");
    let [red, _, _, _] = pixel(buffer, column, row);
    255 - red
}

#[test]
fn small_text_rows_are_point_sampled_like_chrome() {
    let columns = letter_columns(14, "");
    assert_eq!(columns[columns.len() / 2], [255, 255], "{columns:?}");
}

#[test]
fn hard_text_shadows_are_point_sampled_like_the_text() {
    let columns = letter_columns(14, "color:transparent;text-shadow:1px 0 #000");
    assert_eq!(columns[columns.len() / 2], [255, 255], "{columns:?}");
}

#[test]
fn smoothed_text_sizes_keep_soft_curves() {
    let columns = letter_columns(20, "");
    let middle = &columns[columns.len() / 2];
    assert!(middle.iter().any(|&value| value < 200), "{columns:?}");
}
