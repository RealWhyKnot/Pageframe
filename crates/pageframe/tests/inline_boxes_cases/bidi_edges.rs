use super::*;

fn row_of(buffer: &[u8], width: usize) -> Vec<(usize, [u8; 4])> {
    (0..width)
        .map(|column| (column, pixel(buffer, column, 20)))
        .collect()
}

fn columns_where(row: &[(usize, [u8; 4])], wanted: [u8; 4]) -> Vec<usize> {
    row.iter()
        .filter(|(_, colour)| *colour == wanted)
        .map(|(column, _)| *column)
        .collect()
}

fn span_text_insets(direction: &str, before: &str, after: &str) -> (usize, usize) {
    let buffer = draw(&format!(
        "<div dir=\"{direction}\" style=\"line-height:40px;width:90px\">{before}<span \
         style=\"background:#0f0;padding:0 9px 0 2px\">aa</span>{after}</div>"
    ));
    let row = row_of(&buffer, 90);
    let lime: Vec<usize> = row
        .iter()
        .filter(|(_, colour)| colour[1] > 200 && colour[0] < 60 && colour[2] < 60)
        .map(|(column, _)| *column)
        .collect();
    let (box_left, box_right) = (lime[0], lime[lime.len() - 1]);
    let ink: Vec<usize> = row[box_left..=box_right]
        .iter()
        .filter(|(_, colour)| colour[1] < 128)
        .map(|(column, _)| *column)
        .collect();
    (ink[0] - box_left, box_right - ink[ink.len() - 1])
}

#[test]
fn right_to_left_spans_keep_their_physical_padding_around_their_text() {
    let expected = span_text_insets("ltr", "", "");
    assert!(expected.1 > expected.0 + 4);
    assert_eq!(span_text_insets("rtl", "", ""), expected);
    assert_eq!(span_text_insets("rtl", "x ", " y"), expected);
}

#[test]
fn an_inline_block_beside_a_right_to_left_span_stays_outside_its_padding() {
    let block = "<i style=\"display:inline-block;width:10px;height:10px;background:#00f\"></i>";
    let span = "<span style=\"background:#0f0;padding:0 9px 0 2px\">bb</span>";
    for inner in [format!("{span}{block}"), format!("{block}{span}")] {
        let buffer = draw(&format!(
            "<div dir=\"rtl\" style=\"line-height:40px;width:90px;color:transparent\">\
             aa {inner} cc</div>"
        ));
        let row = row_of(&buffer, 90);
        let lime = columns_where(&row, LIME);
        let blue = columns_where(&row, BLUE);
        assert!(!lime.is_empty(), "{inner}");
        assert_eq!(blue.len(), 10, "{inner}");
        assert!(
            blue[0] > lime[lime.len() - 1] || blue[blue.len() - 1] < lime[0],
            "{inner}"
        );
    }
}

#[test]
fn a_one_sided_margin_stays_beside_its_span_in_right_to_left_text() {
    let placed = |side: &str| {
        let buffer = draw(&format!(
            "<div dir=\"rtl\" style=\"line-height:40px;width:90px;color:transparent\">\
             \u{5d0}\u{5d0} <span style=\"direction:ltr;{side}:9px\">\
             <u style=\"background:#0f0;text-decoration:none\">bb</u></span> \
             <b style=\"background:#00f\">\u{5d2}\u{5d2}</b></div>"
        ));
        let row = row_of(&buffer, 90);
        (columns_where(&row, LIME), columns_where(&row, BLUE))
    };
    let (lime, blue) = placed("margin-left");
    assert!(!lime.is_empty() && !blue.is_empty());
    assert!(lime[0] >= blue[blue.len() - 1] + 9);
    assert_eq!((lime, blue), placed("padding-left"));
}

#[test]
fn a_one_sided_margin_span_still_wraps_at_the_space_after_it() {
    let wrapped = |margins: &str| {
        let buffer = draw(&format!(
            "<div style=\"line-height:40px;width:50px;color:transparent\">aa \
             <span style=\"{margins}\">bb</span> <b style=\"background:#00f\">cc</b></div>"
        ));
        (0..80)
            .find_map(|down| {
                (0..50)
                    .find(|across| pixel(&buffer, *across, down) == BLUE)
                    .map(|across| (across, down))
            })
            .expect("the blue word is drawn")
    };
    let both = wrapped("margin:0 9px");
    assert_eq!(both.0, 0);
    assert!(both.1 >= 40);
    assert_eq!(wrapped("margin-left:9px"), both);
    assert_eq!(wrapped("margin-right:9px"), both);
}

#[test]
fn an_inline_block_takes_its_own_bidi_level_like_chrome() {
    let block = "<i style=\"display:inline-block;width:10px;height:10px;background:#00f\"></i>";
    for (direction, before, after, left) in [
        ("rtl", "", " abcd", 90),
        ("rtl", "abcd ", "", 56),
        ("rtl", "\u{5d0}\u{5d1}\u{5d2} ", " abcd", 65),
        ("ltr", "abc ", " \u{5d0}\u{5d1}\u{5d2}", 26),
        ("rtl", "ab", "\u{5d0}\u{5d1}\u{5d2}", 74),
        ("ltr", "\u{5d0}\u{5d1}\u{5d2}", "ab", 21),
        ("rtl", "ab", "", 74),
    ] {
        let buffer = draw(&format!(
            "<div dir=\"{direction}\" style=\"font:14px/40px Arial;width:100px;color:transparent\">\
             {before}{block}{after}</div>"
        ));
        let blue = columns_where(&row_of(&buffer, 100), BLUE);
        assert_eq!(blue.first(), Some(&left), "{direction} {before}|{after}");
        assert_eq!(blue.len(), 10, "{direction} {before}|{after}");
    }
}

#[test]
fn a_cloned_span_follows_the_paragraph_direction_not_the_block_direction() {
    let row = |dir: &str, text: &str, style: &str, y: usize| {
        let buffer = draw(&format!(
            "<div dir=\"{dir}\" style=\"unicode-bidi:plaintext;line-height:40px;width:30px\">\
             <span style=\"background:#0f0;color:transparent;{style}\">{text}<br>{text}</span>\
             </div>"
        ));
        let row = painted(&buffer, y);
        (row.first().map(|(x, _)| *x), row.len())
    };
    let clone = "box-decoration-break:clone";
    let (_, word) = row("rtl", "aaa", "", 20);
    for y in [20, 60] {
        let padded = row("rtl", "aaa", &format!("padding:0 3px 0 12px;{clone}"), y);
        assert_eq!(padded, (Some(0), word + 15), "latin in rtl, line at {y}");
    }
    let (_, hebrew) = row("ltr", "&#1488;&#1488;", "", 20);
    for y in [20, 60] {
        let padded = row(
            "ltr",
            "&#1488;&#1488;",
            &format!("padding:0 2px 0 7px;{clone}"),
            y,
        );
        assert_eq!(
            padded,
            (Some(30 - hebrew - 9), hebrew + 9),
            "hebrew in ltr, line at {y}"
        );
    }
}

#[test]
fn a_cloned_span_keeps_its_right_padding_inside_a_right_to_left_continuation() {
    let lime_columns = |buffer: &[u8], top| {
        (0..120)
            .filter(|&x| (top..top + 20).any(|y| pixel(buffer, x, y) == LIME))
            .collect::<Vec<_>>()
    };
    let text = draw(
        "<div style=\"line-height:20px\"><span style=\"background:lime;color:transparent\">\
         aaaa aaaa</span></div>",
    );
    let text_width = lime_columns(&text, 0).len();
    let buffer = draw(
        "<div dir=\"rtl\" style=\"width:100px;line-height:20px\">א \
         <span style=\"background:lime;box-decoration-break:clone;padding:0 30px 0 0\">\
         aaaa aaaa aaaa aaaa</span></div>",
    );
    let lime = lime_columns(&buffer, 20);
    assert_eq!(
        (lime.first().copied(), lime.last().copied()),
        (Some(100 - 30 - text_width), Some(99))
    );
}

#[test]
fn a_cloned_closing_inset_never_pushes_words_to_the_next_line() {
    let lines = |padding_left: usize| {
        let buffer = draw(&format!(
            "<div dir=\"rtl\" style=\"unicode-bidi:plaintext;line-height:20px;width:100px\">\
             <span style=\"background:#0f0;box-decoration-break:clone;\
             padding:0 2px 0 {padding_left}px\">aaaa aaaa aaaa aaaa aaaa</span></div>"
        ));
        (0..4)
            .filter(|line| (0..120).any(|x| pixel(&buffer, x, line * 20 + 10) == LIME))
            .count()
    };
    assert_eq!([lines(0), lines(40)], [2, 2]);
}
