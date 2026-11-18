mod support;

use pageframe::App;
use support::{HEIGHT, open, pixel, render};

fn stem_column(size: u32) -> Vec<u8> {
    let html = format!(
        "<html><head><style>html,body{{margin:0;background:#fff}}\
         div{{position:absolute;left:4px;top:4px;height:76px;writing-mode:vertical-rl;\
         font:{size}px/30px Arial}}</style></head><body><div>HHHH</div></body></html>"
    );
    let mut harness = open(App::new((), &html));
    let buffer = render(&mut harness);
    let column = (4..34)
        .max_by_key(|&column| {
            (1..HEIGHT)
                .filter(|&row| ink(&buffer, column, row) > 8 && ink(&buffer, column, row - 1) <= 8)
                .count()
        })
        .expect("the box has columns");
    (0..HEIGHT).map(|row| ink(&buffer, column, row)).collect()
}

fn ink(buffer: &[u8], column: usize, row: u32) -> u8 {
    let row = usize::try_from(row).expect("rows fit in usize");
    let [red, _, _, _] = pixel(buffer, column, row);
    255 - red
}

#[test]
fn small_sideways_stems_land_on_whole_pixels_like_chrome() {
    let stems = stem_column(14);
    let inked: Vec<u8> = stems.iter().copied().filter(|&value| value > 8).collect();
    assert_eq!(inked.len(), 8, "{stems:?}");
    assert!(inked.iter().all(|&value| value == 255), "{stems:?}");
}

#[test]
fn smoothed_sideways_sizes_keep_soft_edges() {
    let stems = stem_column(20);
    assert!(
        stems.iter().any(|&value| value > 8 && value < 200),
        "{stems:?}"
    );
}
