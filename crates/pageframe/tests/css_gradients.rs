mod scaled;
mod support;

use pageframe::App;
use scaled::render_at_scale;
use support::{WIDTH, open, pixel, render};

const PAGE: &str = r#"<html><body style="margin:0;background:white">
<div style="position:absolute;left:0;top:0;width:60px;height:60px;background:repeating-radial-gradient(circle at 30px 30px,rgb(200,0,0) 0 5px,rgb(255,255,255) 5px 10px)"></div>
<div style="position:absolute;left:60px;top:0;width:60px;height:60px;background:conic-gradient(at 20px 20px,rgb(255,0,0) 0 25%,rgb(0,0,255) 25% 50%,rgb(0,160,0) 50% 75%,rgb(0,0,0) 75%)"></div>
</body></html>"#;

const RED: [u8; 4] = [200, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const CONIC: [((usize, usize), [u8; 4]); 4] = [
    ((75, 15), [0, 0, 0, 255]),
    ((85, 15), [255, 0, 0, 255]),
    ((75, 25), [0, 160, 0, 255]),
    ((85, 25), [0, 0, 255, 255]),
];

#[test]
fn radial_and_conic_lengths_are_css_pixels_at_1x() {
    let buffer = render(&mut open(App::new((), PAGE)));
    for ((column, row), chrome) in [
        ((32, 30), RED),
        ((37, 30), WHITE),
        ((42, 30), RED),
        ((47, 30), WHITE),
    ]
    .into_iter()
    .chain(CONIC)
    {
        assert_eq!(
            pixel(&buffer, column, row),
            chrome,
            "({column},{row}) at 1x"
        );
    }
}

#[test]
fn radial_and_conic_lengths_are_css_pixels_at_2x() {
    let buffer = render_at_scale(&mut open(App::new((), PAGE)), 2);
    let width = usize::try_from(WIDTH * 2).expect("test width fits in usize");
    let at = |column: usize, row: usize| {
        let start = (row * width + column) * 4;
        let mut rgba = [0; 4];
        rgba.copy_from_slice(&buffer[start..start + 4]);
        rgba
    };
    for ((column, row), chrome) in [
        ((64, 60), RED),
        ((74, 60), WHITE),
        ((84, 60), RED),
        ((94, 60), WHITE),
        ((104, 60), RED),
        ((60, 84), RED),
    ] {
        assert_eq!(at(column, row), chrome, "({column},{row}) at 2x");
    }
    for ((column, row), chrome) in CONIC {
        assert_eq!(at(column * 2, row * 2), chrome, "({column},{row}) at 2x");
    }
}

type Spot = ((usize, usize), [u8; 3]);

struct Probe {
    boxes: &'static [(u32, u32, u32, &'static str)],
    at_1x: &'static [Spot],
    at_2x: &'static [Spot],
}

const PROBES: [Probe; 7] = [
    Probe {
        boxes: &[
            (0, 120, 20, "linear-gradient(90deg in oklch,blue,red)"),
            (
                20,
                120,
                20,
                "linear-gradient(90deg in srgb-linear,blue,red)",
            ),
            (
                40,
                120,
                20,
                "linear-gradient(90deg in hsl longer hue,blue,red)",
            ),
            (
                60,
                120,
                20,
                "linear-gradient(90deg,oklch(0.5 0.2 30),oklch(0.8 0.1 200))",
            ),
        ],
        at_1x: &[
            ((30, 10), [123, 0, 243]),
            ((60, 10), [187, 0, 192]),
            ((90, 10), [233, 0, 121]),
            ((30, 30), [138, 0, 224]),
            ((60, 30), [188, 0, 186]),
            ((90, 30), [225, 0, 136]),
            ((30, 50), [0, 255, 250]),
            ((60, 50), [4, 255, 0]),
            ((90, 50), [255, 250, 0]),
            ((30, 70), [182, 89, 72]),
            ((60, 70), [171, 133, 120]),
            ((90, 70), [147, 172, 167]),
        ],
        at_2x: &[
            ((30, 10), [123, 0, 243]),
            ((60, 10), [186, 0, 193]),
            ((90, 10), [232, 0, 122]),
            ((30, 30), [137, 0, 224]),
            ((60, 30), [187, 0, 187]),
            ((90, 30), [224, 0, 136]),
            ((30, 50), [0, 255, 252]),
            ((60, 50), [2, 255, 0]),
            ((90, 50), [255, 252, 0]),
            ((30, 70), [182, 89, 72]),
            ((60, 70), [171, 133, 120]),
            ((90, 70), [148, 172, 167]),
        ],
    },
    Probe {
        boxes: &[
            (
                0,
                120,
                20,
                "linear-gradient(in oklch 90deg,oklch(0.5 0.2 30),20%,oklch(0.8 0.1 200))",
            ),
            (
                20,
                120,
                60,
                "radial-gradient(circle at 30% 40%,white,rgb(51,102,204) 60%,black)",
            ),
        ],
        at_1x: &[
            ((30, 10), [129, 162, 47]),
            ((60, 10), [89, 189, 133]),
            ((100, 10), [85, 204, 192]),
            ((0, 44), [122, 155, 222]),
            ((60, 44), [163, 186, 232]),
            ((110, 44), [23, 47, 94]),
            ((30, 70), [154, 179, 229]),
        ],
        at_2x: &[
            ((30, 10), [129, 161, 45]),
            ((60, 10), [89, 188, 132]),
            ((100, 10), [84, 204, 192]),
            ((0, 44), [122, 155, 221]),
            ((60, 44), [164, 187, 232]),
            ((110, 44), [23, 47, 95]),
            ((30, 70), [155, 180, 230]),
        ],
    },
    Probe {
        boxes: &[
            (
                0,
                120,
                40,
                "radial-gradient(circle closest-corner at 30% 40%,white,black)",
            ),
            (
                40,
                120,
                40,
                "radial-gradient(ellipse closest-side at 25% 50%,white,black)",
            ),
        ],
        at_1x: &[
            ((10, 16), [90, 90, 90]),
            ((60, 16), [96, 96, 96]),
            ((15, 60), [132, 132, 132]),
            ((30, 70), [121, 121, 121]),
            ((45, 60), [123, 123, 123]),
        ],
        at_2x: &[
            ((10, 16), [88, 88, 88]),
            ((60, 16), [98, 98, 98]),
            ((15, 60), [129, 129, 129]),
            ((30, 70), [124, 124, 124]),
            ((45, 60), [125, 125, 125]),
        ],
    },
    Probe {
        boxes: &[(
            0,
            80,
            80,
            "conic-gradient(from 45deg,red,yellow,lime,cyan,blue,magenta,red)",
        )],
        at_1x: &[
            ((60, 10), [255, 0, 43]),
            ((70, 60), [174, 255, 0]),
            ((10, 60), [0, 211, 255]),
            ((20, 10), [176, 0, 255]),
        ],
        at_2x: &[
            ((60, 10), [255, 0, 45]),
            ((70, 60), [175, 255, 0]),
            ((10, 60), [0, 209, 255]),
            ((20, 10), [176, 0, 255]),
        ],
    },
    Probe {
        boxes: &[
            (
                0,
                120,
                20,
                "linear-gradient(90deg in oklch,oklch(0.6 0.2 none),oklch(0.6 0.2 240))",
            ),
            (
                20,
                120,
                20,
                "linear-gradient(90deg in hsl,hsl(none 80% 50%),hsl(120 80% 50%))",
            ),
            (
                40,
                120,
                20,
                "linear-gradient(90deg in oklab,oklab(0.6 none 0.1),oklab(0.6 0.2 -0.1))",
            ),
            (
                60,
                120,
                20,
                "linear-gradient(90deg in oklch,red,transparent)",
            ),
        ],
        at_1x: &[
            ((20, 10), [0, 136, 233]),
            ((60, 10), [0, 136, 233]),
            ((100, 10), [0, 136, 233]),
            ((20, 30), [25, 229, 25]),
            ((60, 30), [25, 229, 25]),
            ((100, 30), [25, 229, 25]),
            ((20, 50), [226, 48, 77]),
            ((60, 50), [215, 57, 123]),
            ((100, 50), [205, 58, 164]),
            ((20, 70), [255, 44, 44]),
            ((60, 70), [255, 129, 129]),
            ((100, 70), [255, 214, 214]),
        ],
        at_2x: &[
            ((20, 10), [0, 136, 233]),
            ((60, 10), [0, 136, 233]),
            ((100, 10), [0, 136, 233]),
            ((20, 30), [25, 229, 25]),
            ((60, 30), [25, 229, 25]),
            ((100, 30), [25, 229, 25]),
            ((20, 50), [226, 48, 77]),
            ((60, 50), [215, 57, 123]),
            ((100, 50), [205, 58, 163]),
            ((20, 70), [255, 43, 43]),
            ((60, 70), [255, 128, 128]),
            ((100, 70), [255, 213, 213]),
        ],
    },
    Probe {
        boxes: &[
            (
                0,
                120,
                20,
                "linear-gradient(90deg in display-p3-linear,red,blue)",
            ),
            (
                20,
                120,
                20,
                "repeating-linear-gradient(90deg in oklch,blue 0,red 40px)",
            ),
            (
                40,
                120,
                20,
                "linear-gradient(90deg in oklch,red 30%,transparent 30% 60%,blue 60%)",
            ),
            (60, 120, 20, "linear-gradient(90deg in hsl,white,blue)"),
        ],
        at_1x: &[
            ((20, 10), [234, 0, 115]),
            ((60, 10), [187, 0, 188]),
            ((100, 10), [112, 0, 236]),
            ((20, 30), [189, 0, 190]),
            ((60, 30), [189, 0, 190]),
            ((100, 30), [189, 0, 190]),
            ((20, 50), [255, 0, 0]),
            ((60, 50), [255, 255, 255]),
            ((100, 50), [0, 0, 255]),
            ((20, 70), [229, 229, 237]),
            ((60, 70), [158, 158, 223]),
            ((100, 70), [58, 58, 237]),
        ],
        at_2x: &[
            ((20, 10), [235, 0, 114]),
            ((60, 10), [187, 0, 187]),
            ((100, 10), [112, 0, 235]),
            ((20, 30), [187, 0, 192]),
            ((60, 30), [187, 0, 192]),
            ((100, 30), [187, 0, 192]),
            ((20, 50), [255, 0, 0]),
            ((60, 50), [255, 255, 255]),
            ((100, 50), [0, 0, 255]),
            ((20, 70), [229, 229, 237]),
            ((60, 70), [158, 158, 223]),
            ((100, 70), [59, 59, 237]),
        ],
    },
    Probe {
        boxes: &[
            (0, 120, 20, "linear-gradient(90deg in oklch,white,blue)"),
            (
                20,
                120,
                20,
                "linear-gradient(90deg in oklch,oklch(1 0 264),blue)",
            ),
            (
                40,
                120,
                20,
                "linear-gradient(90deg in oklch,rgb(255 255 247),blue)",
            ),
            (60, 120, 20, "linear-gradient(90deg in oklch,#808080,lime)"),
        ],
        at_1x: &[
            ((20, 10), [192, 230, 255]),
            ((60, 10), [74, 171, 255]),
            ((100, 10), [0, 91, 255]),
            ((20, 30), [184, 234, 245]),
            ((60, 30), [0, 180, 248]),
            ((100, 30), [0, 97, 255]),
            ((20, 50), [202, 224, 255]),
            ((60, 50), [112, 161, 255]),
            ((100, 50), [22, 85, 255]),
            ((20, 70), [111, 152, 136]),
            ((60, 70), [63, 196, 132]),
            ((100, 70), [0, 237, 90]),
        ],
        at_2x: &[
            ((20, 10), [193, 230, 255]),
            ((60, 10), [74, 171, 255]),
            ((100, 10), [0, 92, 255]),
            ((20, 30), [184, 234, 245]),
            ((60, 30), [0, 180, 248]),
            ((100, 30), [0, 98, 255]),
            ((20, 50), [203, 224, 255]),
            ((60, 50), [112, 161, 255]),
            ((100, 50), [23, 85, 255]),
            ((20, 70), [111, 151, 136]),
            ((60, 70), [63, 196, 132]),
            ((100, 70), [0, 237, 90]),
        ],
    },
];

#[test]
fn interpolation_spaces_and_extents_match_chrome() {
    for probe in PROBES {
        let html: String = probe
            .boxes
            .iter()
            .map(|(top, width, height, background)| {
                format!(
                    r#"<div style="position:absolute;left:0;top:{top}px;width:{width}px;height:{height}px;background:{background}"></div>"#
                )
            })
            .collect();
        let html = format!(r#"<html><body style="margin:0;background:white">{html}</body></html>"#);
        for (scale, spots) in [(1_u16, probe.at_1x), (2, probe.at_2x)] {
            let buffer = render_at_scale(&mut open(App::new((), &html)), scale);
            let factor = usize::from(scale);
            let width = usize::try_from(WIDTH).expect("test width fits in usize") * factor;
            for &((column, row), chrome) in spots {
                let start = (row * factor * width + column * factor) * 4;
                let actual = &buffer[start..start + 3];
                let near = actual
                    .iter()
                    .zip(chrome)
                    .all(|(&channel, expected)| channel.abs_diff(expected) <= 2);
                assert!(
                    near,
                    "({column},{row}) {actual:?} vs {chrome:?} at {scale}x in {html}"
                );
            }
        }
    }
}
