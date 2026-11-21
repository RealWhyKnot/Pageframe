mod scaled;
mod support;

use pageframe::App;
use scaled::{render_at_ratio, render_at_scale};
use support::{HEIGHT, WIDTH, open, pixel, render};

const OPEN: &str = "<style>html,body{margin:0;background:white;font:14px Arial}</style><details open><summary>Summary</summary>Body</details>";
const CLOSED_RTL: &str = "<style>html,body{margin:0;background:white;font:14px Arial}</style><details dir=rtl><summary>Summary</summary></details>";
const WHITE: [u8; 4] = [255, 255, 255, 255];

struct Marker {
    rows: Vec<Vec<u8>>,
}

impl Marker {
    fn render(html: &str, ratio: (u16, u16), css_columns: std::ops::Range<u32>) -> Self {
        let mut harness = open(App::new((), html));
        let buffer = match ratio {
            (1, 1) => render(&mut harness),
            (2, 1) => render_at_scale(&mut harness, 2),
            _ => render_at_ratio(&mut harness, ratio, WIDTH),
        };
        let device = |css: u32| css * u32::from(ratio.0) / u32::from(ratio.1);
        let width = device(WIDTH);
        let rows = (0..device(HEIGHT / 4))
            .map(|y| {
                (device(css_columns.start)..device(css_columns.end))
                    .map(|x| {
                        let offset = usize::try_from((y * width + x) * 4)
                            .expect("pixel offset fits in usize");
                        buffer[offset]
                    })
                    .collect()
            })
            .collect();
        if ratio == (1, 1) {
            assert_eq!(pixel(&buffer, 0, 0), WHITE);
        }
        Self { rows }
    }

    fn inked(&self) -> Vec<&Vec<u8>> {
        self.rows
            .iter()
            .skip_while(|row| row.iter().all(|&value| value >= 200))
            .take_while(|row| row.iter().any(|&value| value < 200))
            .collect()
    }

    fn ink_width(row: &[u8]) -> usize {
        row.iter().filter(|&&value| value < 128).count()
    }

    fn assert_quarter_pixel_top_edge(&self, label: &str) {
        let top = self
            .rows
            .iter()
            .position(|row| row.iter().any(|&value| value < 200))
            .expect("the marker has ink");
        assert!(top > 0, "{label}: the marker starts below the first row");
        let row = &self.rows[top];
        let edge: Vec<u8> = row.iter().copied().filter(|&value| value < 250).collect();
        assert!(edge.len() >= 6, "{label}: top edge {row:?}");
        for &value in &edge[1..edge.len() - 1] {
            let quarters = (1.0 - f32::from(value) / 255.0) * 4.0;
            assert!(
                (quarters - quarters.round()).abs() < 0.04,
                "{label}: top edge {value} is not a quarter of a pixel in {row:?}"
            );
        }
    }
}

#[test]
fn an_open_disclosure_triangle_points_down_with_a_quarter_pixel_top_edge() {
    for ratio in [(1, 1), (2, 1), (7, 4)] {
        let marker = Marker::render(OPEN, ratio, 0..12);
        let label = format!("{}/{}", ratio.0, ratio.1);
        marker.assert_quarter_pixel_top_edge(&label);
        let inked = marker.inked();
        let widest = inked
            .iter()
            .map(|row| Marker::ink_width(row))
            .max()
            .unwrap_or_default();
        let bottom = Marker::ink_width(inked.last().expect("the marker has ink"));
        assert!(bottom < widest / 2, "{label}: the open marker points down");
    }
}

#[test]
fn a_closed_right_to_left_disclosure_triangle_points_left_from_the_inline_start() {
    for ratio in [(1, 1), (2, 1), (7, 4)] {
        let marker = Marker::render(CLOSED_RTL, ratio, WIDTH - 10..WIDTH);
        let label = format!("{}/{}", ratio.0, ratio.1);
        let inked = marker.inked();
        let heights: Vec<usize> = (0..marker.rows[0].len())
            .map(|column| inked.iter().filter(|row| row[column] < 128).count())
            .collect();
        let first = heights.iter().position(|&height| height > 0);
        let last = heights.iter().rposition(|&height| height > 0);
        let (Some(first), Some(last)) = (first, last) else {
            panic!("{label}: no marker ink in {heights:?}");
        };
        assert_eq!(last, heights.len() - 1, "{label}: {heights:?}");
        assert!(
            heights[last] > 2 * heights[first],
            "{label}: the marker points left in {heights:?}"
        );
    }
}
