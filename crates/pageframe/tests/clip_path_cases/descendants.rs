use super::*;

#[test]
fn effects_clip_descendants_after_filtering() {
    for (effect, color) in [
        ("display:inline-block", [204, 68, 0, 255]),
        (
            "transform:translate(5px,3px) rotate(10deg)",
            [204, 68, 0, 255],
        ),
        ("opacity:.5", [229, 161, 127, 255]),
        ("overflow:hidden", [204, 68, 0, 255]),
        ("filter:blur(3px)", [204, 68, 0, 255]),
        ("filter:opacity(.5)", [230, 162, 128, 255]),
        (
            "border:8px solid #e40;padding:7px;border-radius:25px",
            [204, 68, 0, 255],
        ),
        (
            "box-shadow:0 0 0 8px #e40;outline:4px solid #0a0",
            [204, 68, 0, 255],
        ),
    ] {
        let html = format!(
            r#"<html><head><style>html,body{{margin:0;background:white}}</style></head><body><div id="under" style="position:absolute;inset:0"></div><div id="front" style="position:absolute;left:10px;top:10px;width:100px;height:60px;box-sizing:border-box;background:#08c;clip-path:inset(5px round 15px);{effect}"><div style="position:absolute;left:-20px;top:20px;width:160px;height:20px;background:#c40"></div></div></body></html>"#
        );
        let app = App::new(Vec::new(), &html)
            .on("#front", EventKind::Click, |seen, _, _| seen.push("front"))
            .on("#under", EventKind::Click, |seen, _, _| seen.push("under"));
        let mut harness = open(app);
        let buffer = render(&mut harness);
        assert_eq!(pixel(&buffer, 15, 15), [255; 4], "{effect}");
        assert_eq!(pixel(&buffer, 5, 40), [255; 4], "{effect}");
        let actual = pixel(&buffer, 60, 40);
        assert!(
            actual
                .into_iter()
                .zip(color)
                .all(|(a, b)| a.abs_diff(b) <= 1),
            "{effect}: {actual:?}"
        );
        harness.click_at(15.0, 15.0);
        assert_eq!(harness.doc.state().last(), Some(&"under"), "{effect}");
        harness.click_at(60.0, 40.0);
        assert_eq!(harness.doc.state().last(), Some(&"front"), "{effect}");
    }
}

#[test]
fn circle_clips_fixed_descendants_and_long_text() {
    let text = "x".repeat(20000);
    let html = format!(
        r#"<html><head><style>html,body{{margin:0;background:white}}</style></head><body><div id="under" style="position:absolute;inset:0"></div><div id="front" style="position:absolute;left:10px;top:10px;width:100px;height:60px;clip-path:circle(30px);background:#08c;white-space:nowrap">{text}<div style="position:fixed;left:0;top:30px;width:999999px;height:20px;background:#c40"></div></div></body></html>"#
    );
    let app = App::new(Vec::new(), &html)
        .on("#front", EventKind::Click, |seen, _, _| seen.push("front"))
        .on("#under", EventKind::Click, |seen, _, _| seen.push("under"));
    let mut harness = open(app);
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 5, 40), [255; 4]);
    assert_eq!(pixel(&buffer, 60, 40), [204, 68, 0, 255]);
    harness.click_at(5.0, 40.0);
    assert_eq!(harness.doc.state().last(), Some(&"under"));
    harness.click_at(60.0, 40.0);
    assert_eq!(harness.doc.state().last(), Some(&"front"));
    let doc = harness.base();
    let id = doc.query_selector("#front").unwrap().unwrap();
    assert_eq!(doc.get_node(id).unwrap().final_layout().size.width, 100.0);
}

#[test]
fn ellipse_clips_an_image() {
    let html = r#"<html><head><style>html,body{margin:0;background:white}</style></head><body><img style="width:120px;height:60px;clip-path:ellipse()" src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='120' height='60'%3E%3Cpath fill='%2308c' d='M0 0h120v60H0z'/%3E%3C/svg%3E"></body></html>"#;
    let mut harness = open(App::new((), html));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 2, 2), [255; 4]);
    assert_eq!(pixel(&buffer, 10, 30), [0, 136, 204, 255]);
}

#[test]
fn reference_boxes_clip_overflowing_children() {
    for (clip, samples) in [
        ("border-box", [false, true, true, true]),
        ("padding-box", [false, false, true, true]),
        ("content-box", [false, false, false, false]),
    ] {
        let html = format!(
            r#"<html><head><style>html,body{{margin:0;background:white}}</style></head><body><div id="under" style="position:absolute;inset:0"></div><div id="front" style="position:absolute;left:0;top:0;width:120px;height:60px;box-sizing:border-box;border:8px solid #e40;padding:7px;border-radius:25px;clip-path:{clip}"><div style="position:absolute;inset:-8px;background:#08c"></div></div></body></html>"#
        );
        let app = App::new(Vec::new(), &html)
            .on("#front", EventKind::Click, |seen, _, _| seen.push("front"))
            .on("#under", EventKind::Click, |seen, _, _| seen.push("under"));
        let mut harness = open(app);
        let buffer = render(&mut harness);
        for ((x, y, click_x, click_y), inside) in [
            (2, 2, 2.0, 2.0),
            (9, 9, 9.0, 9.0),
            (16, 16, 16.0, 16.0),
            (10, 30, 10.0, 30.0),
        ]
        .into_iter()
        .zip(samples)
        {
            let expected = if inside { [0, 136, 204, 255] } else { [255; 4] };
            assert_eq!(pixel(&buffer, x, y), expected, "{clip} at {x},{y}");
            harness.click_at(click_x, click_y);
            let target = if inside { "front" } else { "under" };
            assert_eq!(
                harness.doc.state().last(),
                Some(&target),
                "{clip} at {x},{y}"
            );
        }
    }
}

#[test]
fn changing_clip_path_rebuilds_descendant_paint_ownership() {
    let html = r#"<html><head><style>html,body{margin:0;background:white}</style></head><body><div id="front" style="position:absolute;left:10px;top:10px;width:100px;height:60px;clip-path:circle(30px);background:#08c"><div><div style="position:fixed;left:0;top:30px;width:120px;height:20px;background:#c40"></div></div></div></body></html>"#;
    let mut harness = open(App::new((), html));
    for (clip, expected) in [
        ("circle(30px)", [255; 4]),
        ("none", [204, 68, 0, 255]),
        ("inset(0 round 30px)", [255; 4]),
        ("none", [204, 68, 0, 255]),
    ] {
        harness
            .doc
            .ui()
            .select("#front")
            .set_style("clip-path", clip);
        harness.pump();
        let buffer = render(&mut harness);
        assert_eq!(pixel(&buffer, 5, 40), expected, "{clip}");
        assert_eq!(pixel(&buffer, 60, 40), [204, 68, 0, 255], "{clip}");
    }
}

#[test]
fn inline_blocks_use_their_own_shape_box() {
    for (clip, point, expected) in [
        ("circle(50%)", (5, 30), [255; 4]),
        ("ellipse()", (5, 30), [0, 136, 204, 255]),
        ("inset(5px round 25% / 50%)", (8, 8), [255; 4]),
    ] {
        let html = format!(
            r#"<html><head><style>html,body{{margin:0;background:white}}</style></head><body><span style="display:inline-block;vertical-align:top;width:120px;height:60px;background:#08c;clip-path:{clip}"><span style="display:block;width:160px;height:30px;background:#c40"></span></span></body></html>"#
        );
        let mut harness = open(App::new((), &html));
        let buffer = render(&mut harness);
        assert_eq!(pixel(&buffer, point.0, point.1), expected, "{clip}");
        assert_eq!(pixel(&buffer, 60, 40), [0, 136, 204, 255], "{clip}");
    }
}

#[test]
fn a_clip_on_a_static_box_clips_its_absolute_child() {
    let html = r#"<html><head><style>html,body{margin:0;background:white}</style></head><body><div style="position:relative;width:120px;height:80px"><div style="width:60px;height:40px;clip-path:circle(15px);background:#08c"><div id="front" style="position:absolute;left:0;top:0;width:120px;height:80px;background:#c40"></div></div></div></body></html>"#;
    let app =
        App::new(Vec::new(), html).on("#front", EventKind::Click, |seen, _, _| seen.push("front"));
    let mut harness = open(app);
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 30, 20), [204, 68, 0, 255]);
    assert_eq!(pixel(&buffer, 90, 60), [255; 4]);
    assert_eq!(pixel(&buffer, 5, 5), [255; 4]);
    harness.click_at(90.0, 60.0);
    assert_eq!(harness.doc.state().last(), None);
    harness.click_at(30.0, 20.0);
    assert_eq!(harness.doc.state().last(), Some(&"front"));
}

#[test]
fn a_missing_clip_reference_leaves_an_absolute_child_alone() {
    let html = r#"<html><head><style>html,body{margin:0;background:white}</style></head><body><div style="position:relative;width:120px;height:80px"><div style="width:60px;height:40px;clip-path:url(#missing)"><div style="position:absolute;left:0;top:0;width:120px;height:80px;background:#c40"></div></div><div style="position:absolute;left:60px;top:0;width:30px;height:30px;background:#08c"></div></div></body></html>"#;
    let mut harness = open(App::new((), html));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 30, 70), [204, 68, 0, 255]);
    assert_eq!(pixel(&buffer, 70, 10), [0, 136, 204, 255]);
}

#[test]
fn a_scrolled_and_scaled_box_is_hit_inside_its_clip() {
    let html = r#"<html><head><style>html,body{margin:0;background:white}</style></head><body><div id="under" style="position:absolute;inset:0"></div><div id="front" style="position:absolute;left:0;top:0;width:30px;height:30px;overflow:hidden;transform-origin:0 0;transform:scale(2);clip-path:circle(10px at 15px 15px);background:#08c"><div style="height:150px"></div></div></body></html>"#;
    let app = App::new(Vec::new(), html)
        .on("#front", EventKind::Click, |seen, _, _| seen.push("front"))
        .on("#under", EventKind::Click, |seen, _, _| seen.push("under"));
    let mut harness = open(app);
    let id = harness.base().query_selector("#front").unwrap().unwrap();
    harness
        .base_mut()
        .get_node_mut(id)
        .unwrap()
        .scroll_offset_mut()
        .y = 20.0;
    let buffer = render(&mut harness);
    for (y, inside) in [(14_u16, true), (46, true), (54, false), (6, false)] {
        let p = pixel(&buffer, 30, usize::from(y));
        assert_eq!(p == [0, 136, 204, 255], inside, "pixel at 30,{y}");
        harness.click_at(30.0, f32::from(y));
        let expected = if inside { "front" } else { "under" };
        assert_eq!(harness.doc.state().last(), Some(&expected), "hit at 30,{y}");
    }
}
