use super::*;

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
