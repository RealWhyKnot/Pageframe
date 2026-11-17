mod support;

use pageframe::App;
use support::open;

const CONTENT: &str = "<style>html,body{margin:0;font:16px Arial}\
input,textarea{border:2px solid;padding:3px;width:100px;height:24px}</style>\
<form id='form'><input id='input'><textarea id='area'></textarea></form>\
<div id='after'>after</div>\
<img id='image' align='left' style='width:10px;height:10px'>";

struct Measured {
    input: (f32, f32),
    area: (f32, f32),
    after_top: f32,
    image_margin_right: f32,
}

fn measure(standards: bool) -> Measured {
    let doctype = if standards { "<!doctype html>" } else { "" };
    let harness = open(App::new((), &format!("{doctype}{CONTENT}")));
    let base = harness.base();
    let layout = |selector: &str| {
        let id = base
            .query_selector_all(selector)
            .expect("the selector parses")[0];
        base.get_node(id).expect("the node exists").final_layout()
    };
    let input = layout("#input");
    let area = layout("#area");
    Measured {
        input: (input.size.width, input.size.height),
        area: (area.size.width, area.size.height),
        after_top: layout("#after").location.y - layout("#form").size.height,
        image_margin_right: layout("#image").margin.right,
    }
}

#[test]
fn quirks_mode_text_fields_use_border_box_sizing() {
    let quirks = measure(false);
    assert_eq!(quirks.input, (100.0, 24.0));
    assert_eq!(quirks.area, (100.0, 24.0));
    let standards = measure(true);
    assert_eq!(standards.input, (110.0, 34.0));
    assert_eq!(standards.area, (110.0, 34.0));
}

#[test]
fn quirks_mode_forms_keep_a_bottom_margin() {
    assert_eq!(measure(false).after_top, 16.0);
    assert_eq!(measure(true).after_top, 0.0);
}

#[test]
fn quirks_mode_aligned_images_get_side_margins() {
    assert_eq!(measure(false).image_margin_right, 3.0);
    assert_eq!(measure(true).image_margin_right, 0.0);
}
