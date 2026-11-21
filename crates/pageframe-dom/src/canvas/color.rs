use color::{Srgb, parse_color};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.eq_ignore_ascii_case("currentcolor") {
            return Some(Self::BLACK);
        }
        let [r, g, b, a] = parse_color(text).ok()?.to_alpha_color::<Srgb>().components;
        let unit = |v: f32| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        Some(Self {
            r: unit(r),
            g: unit(g),
            b: unit(b),
            a: unit(a),
        })
    }

    pub(crate) fn premultiplied(self, alpha: f32) -> [f32; 4] {
        let a = self.a * alpha;
        [self.r * a, self.g * a, self.b * a, a]
    }

    pub fn serialize(self) -> String {
        let byte = |v: f32| (v * 255.0).round() as u8;
        let (r, g, b) = (byte(self.r), byte(self.g), byte(self.b));
        if self.a >= 1.0 {
            return format!("#{r:02x}{g:02x}{b:02x}");
        }
        format!("rgba({r}, {g}, {b}, {})", format_alpha(self.a))
    }
}

fn format_alpha(alpha: f32) -> String {
    if alpha <= 0.0 {
        return "0".to_string();
    }
    let rounded = (f64::from(alpha) * 1000.0).round() / 1000.0;
    let text = format!("{rounded:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_serializes_like_canvas() {
        assert_eq!(Rgba::parse("#4D9FFF").unwrap().serialize(), "#4d9fff");
        assert_eq!(Rgba::parse("red").unwrap().serialize(), "#ff0000");
        assert_eq!(
            Rgba::parse("rgba(77, 159, 255, 0.12)").unwrap().serialize(),
            "rgba(77, 159, 255, 0.12)"
        );
        assert_eq!(
            Rgba::parse("transparent").unwrap().serialize(),
            "rgba(0, 0, 0, 0)"
        );
        assert!(Rgba::parse("nonsense").is_none());
    }
}
