use super::ElementCx;
use kurbo::Rect;

impl ElementCx<'_, '_> {
    /// Compute the rectangle (in scaled pixels, relative to the border box origin) described by
    /// the CSS 2 `clip` property, if it applies. `clip` only applies to absolutely positioned
    /// elements; `auto` components resolve to the corresponding border box edge.
    pub(super) fn css_clip_rect(&self) -> Option<Rect> {
        use style::computed_values::position::T as Position;
        use style::values::computed::LengthOrAuto;
        use style::values::generics::ClipRectOrAuto;

        if !matches!(
            self.style.get_box().position,
            Position::Absolute | Position::Fixed
        ) {
            return None;
        }
        let ClipRectOrAuto::Rect(rect) = &self.style.get_effects().clip else {
            return None;
        };

        let border_box = self.frame.border_box;
        let resolve = |component: &LengthOrAuto, auto: f64| match component {
            LengthOrAuto::Auto => auto,
            LengthOrAuto::LengthPercentage(length) => length.px() as f64 * self.scale,
        };
        let x0 = resolve(&rect.left, 0.0);
        let y0 = resolve(&rect.top, 0.0);
        let x1 = resolve(&rect.right, border_box.width());
        let y1 = resolve(&rect.bottom, border_box.height());

        Some(Rect::new(x0, y0, x1.max(x0), y1.max(y0)))
    }
}
