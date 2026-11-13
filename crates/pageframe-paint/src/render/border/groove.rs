use style::values::computed::BorderStyle;

pub(super) fn effective_style(style: BorderStyle, width: f64) -> BorderStyle {
    if width <= 1.0 && matches!(style, BorderStyle::Groove | BorderStyle::Ridge) {
        BorderStyle::Solid
    } else {
        style
    }
}
