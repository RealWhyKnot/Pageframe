mod support;

use pageframe::{App, Page};
use pageframe_dom::{QualName, local_name, ns};
use pageframe_test_harness::Harness;
use support::{open, pixel, render};

const FIELDSETS: &str = "<!doctype html><style>body{margin:0;background:#fff}\
fieldset{margin:0;padding:4px;border:2px solid #000;width:100px}legend{font-size:0}\
legend span{display:inline-block;width:30px;height:18px}</style>\
<fieldset id=f1 style='background:#0f0'><legend id=l1><span></span></legend>\
<div id=c1 style='height:10px'></div></fieldset>\
<fieldset id=f2 style='border-width:10px'><legend id=l2><span style='height:6px'></span>\
</legend><div id=c2 style='height:10px'></div></fieldset>\
<fieldset id=f3><legend id=l3 style='margin:0 auto'><span></span></legend>\
<div id=c3 style='height:10px'></div></fieldset>\
<div><fieldset id=f4 style='display:inline-block'><legend id=l4><span></span></legend>\
<div id=c4 style='height:10px'></div></fieldset></div>";

const FLOATS_AND_SHADOWS: &str = "<!doctype html><style>body{margin:0;background:#fff}\
fieldset{margin:0;padding:0;border:2px solid #000}legend{font-size:0;padding:0}\
legend span{display:inline-block;width:20px;height:10px}</style>\
<div style='float:left;width:30px;height:30px;background:#00f'></div>\
<fieldset id=a><div style='height:4px'></div></fieldset><div style='clear:both'></div>\
<fieldset id=b style='margin:4px;box-shadow:0 0 0 4px #f00;width:60px'><legend id=bl>\
<span></span></legend><div style='height:10px'></div></fieldset>\
<div style='display:flex'><fieldset id=c><legend id=cl><span></span></legend>\
<div id=cc style='height:6px;width:40px'></div></fieldset></div>";

const DISABLED: &str = "<!doctype html><fieldset id=f1 disabled><legend><button id=lb></button></legend><input id=i1><button id=b1></button><select id=s1></select><textarea id=t1></textarea></fieldset><fieldset id=f2><input id=i2><fieldset id=f3 disabled><legend><input id=i3></legend><legend><input id=i4></legend><input id=i5></fieldset></fieldset><fieldset id=f4 disabled><div><legend><button id=b4></button></legend></div></fieldset>";

type Expected = (&'static str, Option<f32>, Option<f32>, f32, f32);

fn layout_mismatches(page: &'static str, chrome: &[Expected]) -> Vec<String> {
    let harness = open(App::new((), page));
    let base = harness.base();
    let layout = |selector: &str| {
        let id = base
            .query_selector_all(selector)
            .expect("the selector parses")[0];
        let layout = base.get_node(id).expect("the node exists").final_layout();
        (
            layout.location.x,
            layout.location.y,
            layout.size.width,
            layout.size.height,
        )
    };
    let mismatches = chrome
        .iter()
        .filter_map(|&(selector, left, top, width, height)| {
            let ours = layout(selector);
            let matches = left.is_none_or(|left| ours.0 == left)
                && top.is_none_or(|top| ours.1 == top)
                && ours.2 == width
                && ours.3 == height;
            (!matches).then(|| {
                format!("{selector} is {ours:?}, Chrome has {left:?} {top:?} {width} {height}")
            })
        })
        .collect();
    drop(base);
    mismatches
}

fn assert_pixels(page: &'static str, samples: &[((usize, usize), [u8; 3])]) {
    let mut harness = open(App::new((), page));
    let buffer = render(&mut harness);
    for &((column, row), expected) in samples {
        let [red, green, blue, _] = pixel(&buffer, column, row);
        assert_eq!(
            [red, green, blue],
            expected,
            "pixel at ({column}, {row}) differs from Chrome"
        );
    }
}

#[test]
fn legends_sit_on_the_top_border_like_chrome() {
    let chrome = [
        ("#f1", None, None, 112.0, 38.0),
        ("#l1", Some(6.0), Some(0.0), 34.0, 18.0),
        ("#c1", Some(6.0), Some(22.0), 100.0, 10.0),
        ("#f2", None, None, 128.0, 38.0),
        ("#l2", Some(14.0), Some(2.0), 34.0, 6.0),
        ("#c2", Some(14.0), Some(14.0), 100.0, 10.0),
        ("#f3", None, None, 112.0, 38.0),
        ("#l3", Some(39.0), Some(0.0), 34.0, 18.0),
        ("#c3", Some(6.0), Some(22.0), 100.0, 10.0),
        ("#f4", None, None, 112.0, 38.0),
        ("#l4", Some(6.0), Some(0.0), 34.0, 18.0),
        ("#c4", Some(6.0), Some(22.0), 100.0, 10.0),
    ];
    let mismatches = layout_mismatches(FIELDSETS, &chrome);
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn fieldsets_avoid_floats_and_size_legends_as_flex_items_like_chrome() {
    let chrome = [
        ("#a", Some(30.0), Some(0.0), 90.0, 8.0),
        ("#b", Some(4.0), Some(34.0), 64.0, 22.0),
        ("#bl", Some(2.0), Some(0.0), 20.0, 10.0),
        ("#c", Some(0.0), Some(0.0), 44.0, 18.0),
        ("#cl", Some(2.0), Some(0.0), 20.0, 10.0),
        ("#cc", Some(2.0), Some(10.0), 40.0, 6.0),
    ];
    let mismatches = layout_mismatches(FLOATS_AND_SHADOWS, &chrome);
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

#[test]
fn fieldset_border_breaks_behind_the_legend_like_chrome() {
    assert_pixels(
        FIELDSETS,
        &[
            ((60, 4), [0xff, 0xff, 0xff]),
            ((20, 4), [0xff, 0xff, 0xff]),
            ((60, 8), [0x00, 0x00, 0x00]),
            ((60, 9), [0x00, 0x00, 0x00]),
            ((20, 9), [0x00, 0xff, 0x00]),
            ((60, 12), [0x00, 0xff, 0x00]),
            ((1, 30), [0x00, 0x00, 0x00]),
            ((5, 30), [0x00, 0xff, 0x00]),
            ((60, 37), [0x00, 0x00, 0x00]),
        ],
    );
}

#[test]
fn fieldset_shadow_follows_the_border_under_the_legend_like_chrome() {
    assert_pixels(
        FLOATS_AND_SHADOWS,
        &[
            ((40, 32), [0xff, 0xff, 0xff]),
            ((40, 34), [0xff, 0x00, 0x00]),
            ((40, 37), [0xff, 0x00, 0x00]),
            ((40, 38), [0x00, 0x00, 0x00]),
            ((15, 38), [0xff, 0xff, 0xff]),
            ((70, 45), [0xff, 0x00, 0x00]),
            ((31, 2), [0x00, 0x00, 0x00]),
        ],
    );
}

const MARGIN_LEGEND: &str = "<!doctype html><style>body{margin:0;background:#fff}\
fieldset{margin:0;padding:0;border:2px solid #000;width:100px;display:inline-block;\
vertical-align:top}legend{font-size:0;padding:0;margin:0 30px}\
legend span{display:inline-block;width:30px;height:10px}</style>\
<fieldset id=f><legend id=l><span></span><span></span><span></span></legend>\
<div style='height:4px'></div></fieldset>";

#[test]
fn legend_margins_narrow_the_space_it_wraps_in_like_chrome() {
    let chrome = [
        ("#f", Some(0.0), Some(0.0), 104.0, 36.0),
        ("#l", Some(32.0), Some(0.0), 40.0, 30.0),
    ];
    assert_eq!(
        layout_mismatches(MARGIN_LEGEND, &chrome),
        Vec::<String>::new()
    );
}

const LEGEND_BASELINES: &str = "<!doctype html><style>body{margin:0;background:#fff;font-size:0}\
fieldset{margin:0;padding:0;border:2px solid #000}legend{padding:0}\
span{display:inline-block;width:20px;height:10px}</style>\
<div><fieldset id=b style='display:inline-block'><legend><span></span></legend>\
<div style='height:4px'></div></fieldset><span id=t style='height:30px'></span></div>\
<div style='height:2px'></div><div style='display:flex;align-items:baseline'>\
<fieldset id=a><legend><span></span></legend><div><span style='height:20px'></span></div>\
</fieldset><div id=s><span></span></div></div>";

#[test]
fn fieldset_baselines_skip_the_legend_like_chrome() {
    let chrome = [
        ("#b", Some(0.0), Some(14.0), 24.0, 16.0),
        ("#t", Some(24.0), Some(0.0), 20.0, 30.0),
        ("#a", Some(0.0), Some(0.0), 24.0, 32.0),
        ("#s", Some(24.0), Some(20.0), 20.0, 10.0),
    ];
    assert_eq!(
        layout_mismatches(LEGEND_BASELINES, &chrome),
        Vec::<String>::new()
    );
}

fn disabled_ids(harness: &Harness<Page<()>>) -> Vec<String> {
    let base = harness.base();
    base.query_selector_all(":disabled")
        .expect("the selector parses")
        .into_iter()
        .filter_map(|id| {
            let node = base.get_node(id)?;
            node.attr(local_name!("id")).map(str::to_owned)
        })
        .collect()
}

fn first(harness: &Harness<Page<()>>, selector: &str) -> pageframe::NodeId {
    harness
        .base()
        .query_selector_all(selector)
        .expect("the selector parses")[0]
}

#[test]
fn a_disabled_fieldset_disables_its_controls_outside_the_first_legend_like_chrome() {
    let mut harness = open(App::new((), DISABLED));
    assert_eq!(
        disabled_ids(&harness),
        ["f1", "i1", "b1", "s1", "t1", "f3", "i4", "i5", "f4", "b4"]
    );
    let disabled = QualName::new(None, ns!(), local_name!("disabled"));
    let f1 = first(&harness, "#f1");
    harness
        .base_mut()
        .mutate()
        .clear_attribute(f1, disabled.clone());
    let f2 = first(&harness, "#f2");
    harness
        .base_mut()
        .mutate()
        .set_attribute(f2, disabled.clone(), "");
    assert_eq!(
        disabled_ids(&harness),
        ["f2", "i2", "f3", "i3", "i4", "i5", "f4", "b4"]
    );
    harness.base_mut().mutate().clear_attribute(f2, disabled);
    let legend = first(&harness, "#f3 > legend");
    harness.base_mut().mutate().remove_node(legend);
    assert_eq!(disabled_ids(&harness), ["f3", "i5", "f4", "b4"]);
    let i5 = first(&harness, "#i5");
    let i2 = first(&harness, "#i2");
    harness.base_mut().mutate().insert_nodes_before(i2, &[i5]);
    assert_eq!(disabled_ids(&harness), ["f3", "f4", "b4"]);
}

#[test]
fn disabled_fields_and_buttons_grey_their_text_like_chrome() {
    let harness = open(App::new((), DISABLED));
    let base = harness.base();
    let color = |selector: &str| {
        let id = base
            .query_selector_all(selector)
            .expect("the selector parses")[0];
        let styles = base
            .get_node(id)
            .expect("the node exists")
            .primary_styles()
            .expect("the control is styled");
        let color = styles.get_inherited_text().color;
        let channels = [
            color.components.0,
            color.components.1,
            color.components.2,
            color.alpha,
        ];
        channels.map(|channel| (channel * 255.0).round())
    };
    let field = [84.0, 84.0, 84.0, 255.0];
    let button = [16.0, 16.0, 16.0, 77.0];
    assert_eq!(color("#i1"), field);
    assert_eq!(color("#t1"), field);
    assert_eq!(color("#b1"), button);
    assert_eq!(color("#b4"), button);
    assert_eq!(color("#i2"), [0.0, 0.0, 0.0, 255.0]);
    drop(base);
}

#[test]
fn a_field_disabled_by_its_fieldset_stops_taking_keys() {
    let mut harness = open(App::new((), "<fieldset id=set><input id=field></fieldset>"));
    harness.click("#field");
    harness.type_text("ab");
    let set = first(&harness, "#set");
    harness.base_mut().mutate().set_attribute(
        set,
        QualName::new(None, ns!(), local_name!("disabled")),
        "",
    );
    harness.type_text("cd");
    harness.pump();
    assert_eq!(
        harness.doc.ui().select("#field").value().as_deref(),
        Some("ab")
    );
}
