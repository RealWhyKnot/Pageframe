use num_traits::ToPrimitive;

#[derive(Clone, Copy, Debug)]
pub(crate) struct HitProbe {
    pub(crate) scale: f64,
    reach: taffy::Rect<f32>,
}

impl HitProbe {
    pub(crate) fn device_pixel(scale: f64) -> Self {
        let pixel = scale
            .recip()
            .to_f32()
            .filter(|pixel| pixel.is_finite())
            .unwrap_or(1.0);
        Self {
            scale,
            reach: taffy::Rect {
                left: 0.0,
                top: 0.0,
                right: pixel,
                bottom: pixel,
            },
        }
    }

    pub(crate) fn through(self, inverse: kurbo::Affine, x: f32, y: f32) -> (f32, f32, Self) {
        let scale = self.scale;
        let local = |corner_x: f32, corner_y: f32| {
            let point = inverse
                * kurbo::Point::new(f64::from(corner_x) * scale, f64::from(corner_y) * scale);
            (
                (point.x / scale).to_f32().unwrap_or(f32::NAN),
                (point.y / scale).to_f32().unwrap_or(f32::NAN),
            )
        };
        let (local_x, local_y) = local(x, y);
        let reach = self.reach;
        let corners = [
            local(x + reach.left, y + reach.top),
            local(x + reach.right, y + reach.top),
            local(x + reach.left, y + reach.bottom),
            local(x + reach.right, y + reach.bottom),
        ];
        let mut bounds = taffy::Rect {
            left: f32::INFINITY,
            top: f32::INFINITY,
            right: f32::NEG_INFINITY,
            bottom: f32::NEG_INFINITY,
        };
        for (corner_x, corner_y) in corners {
            bounds.left = bounds.left.min(corner_x - local_x);
            bounds.top = bounds.top.min(corner_y - local_y);
            bounds.right = bounds.right.max(corner_x - local_x);
            bounds.bottom = bounds.bottom.max(corner_y - local_y);
        }
        (
            local_x,
            local_y,
            Self {
                scale,
                reach: bounds,
            },
        )
    }

    pub(crate) fn span(&self, x: f32, y: f32) -> taffy::Rect<f32> {
        taffy::Rect {
            left: x + self.reach.left,
            top: y + self.reach.top,
            right: x + self.reach.right,
            bottom: y + self.reach.bottom,
        }
    }

    pub(crate) fn holds(&self, rect: &taffy::Rect<f32>, x: f32, y: f32) -> bool {
        let span = self.span(x, y);
        rect.left < rect.right
            && rect.top < rect.bottom
            && span.right > rect.left
            && span.left < rect.right
            && span.bottom > rect.top
            && span.top < rect.bottom
    }
}
