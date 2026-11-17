mod support;

use pageframe::App;
use support::{open, pixel, render};

const CHROME: &[(&str, [u8; 3], [u8; 3])] = &[
    ("AccentColor", [0, 117, 255], [0, 117, 255]),
    ("AccentColorText", [255, 255, 255], [255, 255, 255]),
    ("ActiveText", [0, 102, 204], [255, 0, 0]),
    ("LinkText", [0, 102, 204], [158, 158, 255]),
    ("VisitedText", [0, 102, 204], [208, 173, 240]),
    ("ButtonBorder", [0, 0, 0], [255, 255, 255]),
    ("ButtonFace", [240, 240, 240], [107, 107, 107]),
    ("ButtonText", [0, 0, 0], [255, 255, 255]),
    ("Canvas", [255, 255, 255], [18, 18, 18]),
    ("CanvasText", [0, 0, 0], [255, 255, 255]),
    ("Field", [255, 255, 255], [59, 59, 59]),
    ("FieldText", [0, 0, 0], [255, 255, 255]),
    ("GrayText", [109, 109, 109], [128, 128, 128]),
    ("Highlight", [0, 120, 215], [25, 103, 210]),
    ("HighlightText", [255, 255, 255], [255, 255, 255]),
    ("Mark", [255, 255, 0], [255, 255, 0]),
    ("MarkText", [0, 0, 0], [0, 0, 0]),
    ("SelectedItem", [25, 103, 210], [153, 200, 255]),
    ("SelectedItemText", [255, 255, 255], [59, 59, 59]),
    ("ThreeDDarkShadow", [0, 0, 0], [255, 255, 255]),
    ("ThreeDFace", [240, 240, 240], [107, 107, 107]),
    ("ThreeDHighlight", [0, 0, 0], [255, 255, 255]),
    ("ThreeDLightShadow", [0, 0, 0], [255, 255, 255]),
    ("ThreeDShadow", [0, 0, 0], [255, 255, 255]),
    ("ActiveBorder", [0, 0, 0], [255, 255, 255]),
    ("InactiveBorder", [0, 0, 0], [255, 255, 255]),
    ("WindowFrame", [0, 0, 0], [255, 255, 255]),
    ("ButtonHighlight", [240, 240, 240], [107, 107, 107]),
    ("ButtonShadow", [240, 240, 240], [107, 107, 107]),
    ("ActiveCaption", [255, 255, 255], [18, 18, 18]),
    ("AppWorkspace", [255, 255, 255], [18, 18, 18]),
    ("Background", [255, 255, 255], [18, 18, 18]),
    ("InactiveCaption", [255, 255, 255], [18, 18, 18]),
    ("InfoBackground", [255, 255, 255], [18, 18, 18]),
    ("Menu", [255, 255, 255], [18, 18, 18]),
    ("Scrollbar", [255, 255, 255], [18, 18, 18]),
    ("Window", [255, 255, 255], [18, 18, 18]),
    ("CaptionText", [0, 0, 0], [255, 255, 255]),
    ("InfoText", [0, 0, 0], [255, 255, 255]),
    ("MenuText", [0, 0, 0], [255, 255, 255]),
    ("WindowText", [0, 0, 0], [255, 255, 255]),
    ("InactiveCaptionText", [128, 128, 128], [128, 128, 128]),
];

const PER_ROW: usize = 12;

fn swatches(scheme: &str) -> Vec<[u8; 3]> {
    let cells: String = CHROME
        .iter()
        .map(|(name, _, _)| format!("<i style=\"background:{name}\"></i>"))
        .collect();
    let html = format!(
        "<html style=\"color-scheme:{scheme}\"><head><style>html,body{{margin:0;background:white}}\
         i{{display:block;float:left;width:10px;height:10px}}</style></head>\
         <body>{cells}</body></html>"
    );
    let buffer = render(&mut open(App::new((), &html)));
    (0..CHROME.len())
        .map(|index| {
            let rgba = pixel(&buffer, index % PER_ROW * 10 + 5, index / PER_ROW * 10 + 5);
            [rgba[0], rgba[1], rgba[2]]
        })
        .collect()
}

#[test]
fn light_system_colors_match_chrome() {
    for ((name, chrome, _), found) in CHROME.iter().zip(swatches("light")) {
        assert_eq!(found, *chrome, "light {name}");
    }
}

#[test]
fn dark_system_colors_match_chrome() {
    for ((name, _, chrome), found) in CHROME.iter().zip(swatches("dark")) {
        assert_eq!(found, *chrome, "dark {name}");
    }
}
