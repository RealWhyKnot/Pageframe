mod support;

use pageframe::{App, EventKind};
use support::{open, pixel, render};

#[test]
fn insets_match_chrome() {
    let cases = [
        (
            r#"clip-path:inset(10px)"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x3ffffc, 0xc00003),
                (0x3ffffc, 0xc00003),
                (0x3ffffc, 0xc00003),
                (0x3ffffc, 0xc00003),
                (0x3ffffc, 0xc00003),
                (0x3ffffc, 0xc00003),
                (0x3ffffc, 0xc00003),
                (0x3ffffc, 0xc00003),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:inset(10px 20px)"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:inset(5px 20px 15px)"#,
            [
                (0x000000, 0xffffff),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:inset(5px 10px 15px 20px)"#,
            [
                (0x000000, 0xffffff),
                (0x3ffff0, 0xc0000f),
                (0x3ffff0, 0xc0000f),
                (0x3ffff0, 0xc0000f),
                (0x3ffff0, 0xc0000f),
                (0x3ffff0, 0xc0000f),
                (0x3ffff0, 0xc0000f),
                (0x3ffff0, 0xc0000f),
                (0x3ffff0, 0xc0000f),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:inset(10% 15% round 20%)"#,
            [
                (0x000000, 0xffffff),
                (0x00ff00, 0xf8001f),
                (0x07ffe0, 0xf0000f),
                (0x0ffff0, 0xe00007),
                (0x0ffff0, 0xe00007),
                (0x0ffff0, 0xe00007),
                (0x0ffff0, 0xe00007),
                (0x0ffff0, 0xe00007),
                (0x0ffff0, 0xe00007),
                (0x07ffe0, 0xf0000f),
                (0x00ff00, 0xf8001f),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:inset(5px round 15px 25px 5px 10px / 5px 10px 25px 20px)"#,
            [
                (0x000000, 0xffffff),
                (0x0ffffc, 0xc00001),
                (0x3ffffe, 0x800001),
                (0x7ffffe, 0x800001),
                (0x7ffffe, 0x800001),
                (0x7ffffe, 0x800001),
                (0x7ffffe, 0x800001),
                (0x7ffffe, 0x800001),
                (0x7ffffe, 0x800001),
                (0x3ffffc, 0x800001),
                (0x3ffffc, 0x800003),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:inset(10px round 200px)"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x07ffe0, 0xe00007),
                (0x1ffff8, 0xe00007),
                (0x1ffff8, 0xc00003),
                (0x3ffffc, 0xc00003),
                (0x3ffffc, 0xc00003),
                (0x1ffff8, 0xc00003),
                (0x1ffff8, 0xe00007),
                (0x07ffe0, 0xf0000f),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:inset(70% 80%)"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:inset(-5px round 10px)"#,
            [
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
            ],
        ),
        (
            r#"clip-path:inset(0 round 20px 10px 30px)"#,
            [
                (0x7ffff8, 0x000001),
                (0xfffffe, 0x000001),
                (0xfffffe, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0x7fffff, 0x000000),
                (0x7fffff, 0x000000),
                (0x7fffff, 0x800000),
                (0x1fffff, 0xc00000),
                (0x07fffe, 0xe00000),
            ],
        ),
        (
            r#"clip-path:inset(0 round 15px 30px)"#,
            [
                (0x07fffc, 0xe00001),
                (0x1ffffe, 0xc00000),
                (0x3fffff, 0x800000),
                (0x7fffff, 0x000000),
                (0x7fffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xfffffe, 0x000000),
                (0xfffffe, 0x000000),
                (0xfffffe, 0x000001),
                (0x7ffff8, 0x000003),
                (0x3fffe0, 0x800007),
            ],
        ),
        (
            r#"clip-path:inset(0 round 999999px / 999999px)"#,
            [
                (0x07ffe0, 0xe00007),
                (0x1ffff8, 0xc00003),
                (0x3ffffc, 0x800001),
                (0x7ffffe, 0x000000),
                (0x7ffffe, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0x7ffffe, 0x000000),
                (0x7ffffe, 0x000000),
                (0x7ffffe, 0x800001),
                (0x1ffff8, 0xc00003),
                (0x07ffe0, 0xe00007),
            ],
        ),
    ];
    check_cases(&cases, true);
}

#[test]
fn circles_match_chrome() {
    let cases = [
        (
            r#"clip-path:circle()"#,
            [
                (0x001800, 0xff81ff),
                (0x007e00, 0xff00ff),
                (0x00ff00, 0xfe007f),
                (0x01ff80, 0xfc003f),
                (0x01ff80, 0xfc003f),
                (0x03ffc0, 0xfc003f),
                (0x03ffc0, 0xfc003f),
                (0x01ff80, 0xfc003f),
                (0x01ff80, 0xfc003f),
                (0x01ff80, 0xfe007f),
                (0x007e00, 0xff00ff),
                (0x001800, 0xff81ff),
            ],
        ),
        (
            r#"clip-path:circle(50%)"#,
            [
                (0x07ffe0, 0xf0000f),
                (0x0ffff0, 0xe00007),
                (0x0ffff0, 0xe00007),
                (0x1ffff8, 0xe00007),
                (0x1ffff8, 0xc00003),
                (0x1ffff8, 0xc00003),
                (0x1ffff8, 0xc00003),
                (0x1ffff8, 0xc00003),
                (0x1ffff8, 0xe00007),
                (0x0ffff0, 0xe00007),
                (0x0ffff0, 0xe00007),
                (0x07ffe0, 0xf0000f),
            ],
        ),
        (
            r#"clip-path:circle(20px)"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x001800, 0xff81ff),
                (0x007e00, 0xff81ff),
                (0x007e00, 0xff00ff),
                (0x00ff00, 0xff00ff),
                (0x00ff00, 0xff00ff),
                (0x007e00, 0xff00ff),
                (0x007e00, 0xff81ff),
                (0x001800, 0xffc3ff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:circle(closest-side at 20% 70%)"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000038, 0xffff83),
                (0x0000fc, 0xffff01),
                (0x0000fc, 0xfffe01),
                (0x0000fc, 0xfffe01),
                (0x0000fc, 0xfffe01),
                (0x00007c, 0xffff03),
                (0x000030, 0xffff87),
            ],
        ),
        (
            r#"clip-path:circle(farthest-side at 20% 70%)"#,
            [
                (0x3fffff, 0x800000),
                (0x3fffff, 0x800000),
                (0x7fffff, 0x800000),
                (0x7fffff, 0x000000),
                (0x7fffff, 0x000000),
                (0x7fffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0x7fffff, 0x000000),
            ],
        ),
        (
            r#"clip-path:circle(20px at left top)"#,
            [
                (0x00000f, 0xfffff0),
                (0x000007, 0xfffff0),
                (0x000007, 0xfffff8),
                (0x000001, 0xfffffc),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:circle(25% at right bottom)"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0x3fffff),
                (0xe00000, 0x0fffff),
                (0xf00000, 0x0fffff),
                (0xf00000, 0x07ffff),
                (0xf00000, 0x07ffff),
            ],
        ),
        (
            r#"clip-path:circle(20px at 30px 20px)"#,
            [
                (0x000060, 0xfffe07),
                (0x0001f8, 0xfffe07),
                (0x0001f8, 0xfffc03),
                (0x0003fc, 0xfffc03),
                (0x0003fc, 0xfffc03),
                (0x0001f8, 0xfffc03),
                (0x0001f8, 0xfffe07),
                (0x000060, 0xffff0f),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:circle(closest-side at -10px 20px)"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:circle(farthest-side at 120% 50%)"#,
            [
                (0xfffffe, 0x000000),
                (0xfffffe, 0x000000),
                (0xfffffe, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xfffffe, 0x000000),
                (0xfffffe, 0x000000),
                (0xfffffe, 0x000000),
            ],
        ),
        (
            r#"clip-path:circle(0)"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:none"#,
            [
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
            ],
        ),
    ];
    check_cases(&cases, true);
    let html = r#"<html><head><style>html,body{margin:0;background:white}</style></head><body><div id="under" style="position:absolute;inset:0"></div><div id="front" style="position:absolute;left:30px;top:10px;width:120px;height:60px;clip-path:circle(closest-side at -10px 30px)"><div style="position:absolute;left:-40px;top:-40px;width:200px;height:140px;background:#08c"></div></div></body></html>"#;
    let app = App::new(Vec::new(), html)
        .on("#front", EventKind::Click, |seen, _, _| seen.push("front"))
        .on("#under", EventKind::Click, |seen, _, _| seen.push("under"));
    let mut harness = open(app);
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 20, 40), [0, 136, 204, 255]);
    assert_eq!(pixel(&buffer, 5, 40), [255; 4]);
    harness.click_at(20.0, 40.0);
    assert_eq!(harness.doc.state().last(), Some(&"front"));
    harness.click_at(5.0, 40.0);
    assert_eq!(harness.doc.state().last(), Some(&"under"));
}

#[test]
fn ellipses_match_chrome() {
    let cases = [
        (
            r#"clip-path:ellipse()"#,
            [
                (0x007e00, 0xfc003f),
                (0x07ffe0, 0xe00007),
                (0x1ffff8, 0xc00003),
                (0x7ffffe, 0x800001),
                (0x7ffffe, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0x7ffffe, 0x000000),
                (0x7ffffe, 0x800001),
                (0x1ffff8, 0xc00003),
                (0x07ffe0, 0xe00007),
                (0x007e00, 0xfc003f),
            ],
        ),
        (
            r#"clip-path:ellipse(50% 30%)"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xfc007f),
                (0x0fffe0, 0xc00003),
                (0x3ffffc, 0x800001),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0x3ffffc, 0x800001),
                (0x0fffe0, 0xc00003),
                (0x000000, 0xfc007f),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:ellipse(30px 15px)"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x003c00, 0xff00ff),
                (0x01ff80, 0xfc003f),
                (0x03ffc0, 0xfc003f),
                (0x03ffc0, 0xfc003f),
                (0x01ff80, 0xfc003f),
                (0x003c00, 0xff00ff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:ellipse(closest-side farthest-side at 20% 70%)"#,
            [
                (0x000030, 0xffff87),
                (0x00007c, 0xffff03),
                (0x0000fc, 0xfffe01),
                (0x0000fe, 0xfffe01),
                (0x0001fe, 0xfffe00),
                (0x0001fe, 0xfffc00),
                (0x0001ff, 0xfffc00),
                (0x0001ff, 0xfffc00),
                (0x0001ff, 0xfffc00),
                (0x0001ff, 0xfffc00),
                (0x0001ff, 0xfffc00),
                (0x0001fe, 0xfffc00),
            ],
        ),
        (
            r#"clip-path:ellipse(farthest-side closest-side at 20% 70%)"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x003fff, 0xfc0000),
                (0x0fffff, 0xc00000),
                (0x7fffff, 0x000000),
                (0xffffff, 0x000000),
                (0x3fffff, 0x000000),
                (0x07ffff, 0xe00000),
                (0x0007ff, 0xfe0000),
            ],
        ),
        (
            r#"clip-path:ellipse(20px 10px at right top)"#,
            [
                (0xf00000, 0x0fffff),
                (0xc00000, 0x1fffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:ellipse(closest-side closest-side at -10px 20px)"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:ellipse(999999px 999999px)"#,
            [
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
            ],
        ),
        (
            r#"clip-path:ellipse(0 20px)"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
    ];
    check_cases(&cases, true);
}

#[test]
fn polygons_match_chrome() {
    let cases = [
        (
            r#"clip-path:polygon(nonzero,0 0,100% 0,100% 100%,0 100%,0 0,100% 0,100% 100%,0 100%)"#,
            [
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
            ],
        ),
        (
            r#"clip-path:polygon(evenodd,0 0,100% 0,100% 100%,0 100%,0 0,100% 0,100% 100%,0 100%)"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:polygon(evenodd,50% 0,80% 100%,0 40%,100% 40%,20% 100%)"#,
            [
                (0x000000, 0xffe7ff),
                (0x001800, 0xffe7ff),
                (0x001800, 0xffc3ff),
                (0x003c00, 0xffc3ff),
                (0x003c00, 0xff81ff),
                (0x1f00f8, 0x807e01),
                (0x0700e0, 0xf0ff0f),
                (0x000000, 0xfcff3f),
                (0x018180, 0xfe187f),
                (0x01e780, 0xfc003f),
                (0x0300c0, 0xf83c1f),
                (0x000000, 0xf9ff9f),
            ],
        ),
        (
            r#"clip-path:path("M0 0 H80 V40 H0 Z")"#,
            [
                (0x00ffff, 0xff0000),
                (0x00ffff, 0xff0000),
                (0x00ffff, 0xff0000),
                (0x00ffff, 0xff0000),
                (0x00ffff, 0xff0000),
                (0x00ffff, 0xff0000),
                (0x00ffff, 0xff0000),
                (0x00ffff, 0xff0000),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:rect(10px 100px 50px 20px round 10px)"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x07ffe0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x07ffe0, 0xf0000f),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:xywh(20px 10px 80px 40px round 10px)"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x07ffe0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x07ffe0, 0xf0000f),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"clip-path:url(#missing)"#,
            [
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
            ],
        ),
        (
            r#"clip-path:none"#,
            [
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
            ],
        ),
    ];
    check_cases(&cases, true);
}

#[test]
fn boxes_match_chrome() {
    let cases = [
        (
            r#"border:8px solid #e40;padding:7px;border-radius:25px;clip-path:border-box"#,
            [
                (0x0ffff0, 0xc00003),
                (0x37ffec, 0x800001),
                (0x5ffffa, 0x000000),
                (0x7ffffe, 0x000000),
                (0xbffffd, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xbffffd, 0x000000),
                (0x7ffffe, 0x000000),
                (0x5ffffa, 0x000000),
                (0x37ffec, 0x800001),
                (0x0ffff0, 0xc00003),
            ],
        ),
        (
            r#"border:8px solid #e40;padding:7px;border-radius:25px;clip-path:padding-box"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xf0000f),
                (0x1ffff8, 0xc00003),
                (0x3ffffc, 0xc00003),
                (0x3ffffc, 0x800001),
                (0x3ffffc, 0x800001),
                (0x3ffffc, 0x800001),
                (0x3ffffc, 0x800001),
                (0x3ffffc, 0xc00003),
                (0x1ffff8, 0xc00003),
                (0x000000, 0xf0000f),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"border:8px solid #e40;padding:7px;border-radius:25px;clip-path:content-box"#,
            [
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x0ffff0, 0xe00007),
                (0x1ffff8, 0xe00007),
                (0x1ffff8, 0xe00007),
                (0x1ffff8, 0xe00007),
                (0x1ffff8, 0xe00007),
                (0x0ffff0, 0xe00007),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
                (0x000000, 0xffffff),
            ],
        ),
        (
            r#"border:8px solid #e40;padding:7px;border-radius:25px;clip-path:inset(0)"#,
            [
                (0x0ffff0, 0xc00003),
                (0x37ffec, 0x800001),
                (0x5ffffa, 0x000000),
                (0x7ffffe, 0x000000),
                (0xbffffd, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xbffffd, 0x000000),
                (0x7ffffe, 0x000000),
                (0x5ffffa, 0x000000),
                (0x37ffec, 0x800001),
                (0x0ffff0, 0xc00003),
            ],
        ),
        (
            r#"border:8px solid #e40;padding:7px;border-radius:25px;clip-path:circle(50%) border-box"#,
            [
                (0x07ffe0, 0xf0000f),
                (0x07ffe0, 0xe00007),
                (0x0ffff0, 0xe00007),
                (0x1ffff8, 0xe00007),
                (0x1ffff8, 0xc00003),
                (0x1ffff8, 0xc00003),
                (0x1ffff8, 0xc00003),
                (0x1ffff8, 0xc00003),
                (0x1ffff8, 0xe00007),
                (0x0ffff0, 0xe00007),
                (0x07ffe0, 0xe00007),
                (0x07ffe0, 0xf0000f),
            ],
        ),
        (
            r#"border:8px solid #e40;padding:7px;border-radius:25px;clip-path:circle(50%) padding-box"#,
            [
                (0x01ff80, 0xfc003f),
                (0x03ffc0, 0xf8001f),
                (0x07ffe0, 0xf8001f),
                (0x07ffe0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x0ffff0, 0xf0000f),
                (0x07ffe0, 0xf0000f),
                (0x07ffe0, 0xf8001f),
                (0x03ffc0, 0xf8001f),
                (0x01ff80, 0xfc003f),
            ],
        ),
        (
            r#"border:8px solid #e40;padding:7px;border-radius:25px;clip-path:circle(50%) content-box"#,
            [
                (0x007e00, 0xff00ff),
                (0x01ff80, 0xfe007f),
                (0x01ff80, 0xfc003f),
                (0x03ffc0, 0xf8001f),
                (0x03ffc0, 0xf8001f),
                (0x03ffc0, 0xf8001f),
                (0x03ffc0, 0xf8001f),
                (0x03ffc0, 0xf8001f),
                (0x03ffc0, 0xf8001f),
                (0x01ff80, 0xfc003f),
                (0x01ff80, 0xfe007f),
                (0x007e00, 0xff00ff),
            ],
        ),
        (
            r#"border:8px solid #e40;padding:7px;border-radius:25px;clip-path:none"#,
            [
                (0x0ffff0, 0xc00003),
                (0x37ffec, 0x800001),
                (0x5ffffa, 0x000000),
                (0x7ffffe, 0x000000),
                (0xbffffd, 0x000000),
                (0xffffff, 0x000000),
                (0xffffff, 0x000000),
                (0xbffffd, 0x000000),
                (0x7ffffe, 0x000000),
                (0x5ffffa, 0x000000),
                (0x37ffec, 0x800001),
                (0x0ffff0, 0xc00003),
            ],
        ),
    ];
    check_cases(&cases, false);
}

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
