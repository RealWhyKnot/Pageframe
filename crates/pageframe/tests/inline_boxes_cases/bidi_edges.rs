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
