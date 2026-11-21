const LAYOUT_UNITS_PER_PX: f64 = 64.0;
const UNIT_SLACK: f64 = 1.0 / 32.0;
const NOISE_ULPS: f64 = 4.0;

pub fn hinted_baseline(device_y: f64) -> f64 {
    let units = device_y * LAYOUT_UNITS_PER_PX;
    let slack = (units.abs() * NOISE_ULPS * f64::from(f32::EPSILON)).max(UNIT_SLACK);
    let truncated = (units.abs() + slack).floor().copysign(units);
    ((truncated + LAYOUT_UNITS_PER_PX / 2.0) / LAYOUT_UNITS_PER_PX).floor()
}
