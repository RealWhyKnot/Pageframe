#[path = "clip_path_cases/boxes.rs"]
mod boxes;
#[path = "clip_path_cases/circles.rs"]
mod circles;
#[path = "clip_path_cases/descendants.rs"]
mod descendants;
#[path = "clip_path_cases/ellipses.rs"]
mod ellipses;
#[path = "clip_path_cases/insets.rs"]
mod insets;
#[path = "clip_path_cases/polygons.rs"]
mod polygons;

mod support;

use pageframe::{App, EventKind};
use support::{open, pixel, render};

fn check_cases(cases: &[(&str, [(u32, u32); 12])], check_hits: bool) {
    for &(style, rows) in cases {
        let html = format!(
            r#"<html><head><style>html,body{{margin:0;background:white}}</style></head><body><div id='under' style='position:absolute;inset:0'></div><div id='front' style='position:absolute;left:0;top:0;width:120px;height:60px;box-sizing:border-box;background:#08c;{style}'></div></body></html>"#
        );
        let app = App::new(Vec::new(), &html)
            .on("#front", EventKind::Click, |seen, _, _| seen.push("front"))
            .on("#under", EventKind::Click, |seen, _, _| seen.push("under"));
        let mut harness = open(app);
        let buffer = render(&mut harness);
        for (row, (inside, outside)) in (0_u16..).zip(rows) {
            let y = row * 5 + 2;
            for col in 0_u16..24 {
                let x = col * 5 + 2;
                let p = pixel(&buffer, usize::from(x), usize::from(y));
                if check_hits
                    && row % 6 == 0
                    && col % 12 == 0
                    && (inside | outside) & (1 << col) != 0
                {
                    harness.click_at(f32::from(x), f32::from(y));
                    let expected = if inside & (1 << col) != 0 {
                        "front"
                    } else {
                        "under"
                    };
                    assert_eq!(
                        harness.doc.state().last(),
                        Some(&expected),
                        "hit {style} at {x},{y}"
                    );
                }
                if inside & (1 << col) != 0 {
                    assert!(
                        p[0] < 250 || p[1] < 250 || p[2] < 250,
                        "{style} at {x},{y}: {p:?}"
                    );
                }
                if outside & (1 << col) != 0 {
                    assert_eq!(p, [255, 255, 255, 255], "{style} at {x},{y}");
                }
            }
        }
    }
}
