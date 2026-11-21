use peniko::Gradient;

const LUT_ENTRIES: f32 = 4096.0;

pub(super) fn snap_hard_stops(gradient: &mut Gradient) {
    let stops = &mut *gradient.stops;
    let mut start = 0;
    while start < stops.len() {
        let offset = stops[start].offset;
        let end = stops[start + 1..]
            .iter()
            .position(|stop| stop.offset != offset)
            .map_or(stops.len(), |gap| start + 1 + gap);
        if end - start > 1 && offset > 0.0 && offset < 1.0 {
            let snapped = ((offset * (LUT_ENTRIES - 1.0)).floor() + 0.5) / LUT_ENTRIES;
            let below = start
                .checked_sub(1)
                .map_or(0.0, |before| stops[before].offset);
            let above = stops.get(end).map_or(1.0, |after| after.offset);
            if (below..=above).contains(&snapped) {
                for stop in &mut stops[start..end] {
                    stop.offset = snapped;
                }
            }
        }
        start = end;
    }
}

#[cfg(test)]
mod tests {
    use super::{LUT_ENTRIES, snap_hard_stops};
    use peniko::color::{AlphaColor, Srgb};
    use peniko::{ColorStop, Gradient};

    fn stop(offset: f32, red: f32) -> ColorStop {
        ColorStop::from((offset, AlphaColor::<Srgb>::new([red, 0.0, 0.0, 1.0])))
    }

    fn lut_entry(t: f32) -> f32 {
        (t * (LUT_ENTRIES - 1.0)).trunc()
    }

    #[test]
    fn a_tie_at_a_hard_stop_takes_the_later_colour() {
        let mut gradient = Gradient::new_linear((0.0, 0.0), (21.0, 0.0)).with_stops([
            stop(0.0, 0.0),
            stop(0.5, 0.0),
            stop(0.5, 1.0),
            stop(1.0, 1.0),
        ]);
        snap_hard_stops(&mut gradient);
        let boundary = (gradient.stops[2].offset * LUT_ENTRIES).floor();
        assert_eq!(gradient.stops[1].offset, gradient.stops[2].offset);
        assert!(lut_entry(0.5) >= boundary);
        assert!(lut_entry(0.5 - 1.0 / 4000.0) < boundary);
    }

    #[test]
    fn smooth_stops_and_the_ends_stay_put() {
        let mut gradient = Gradient::new_linear((0.0, 0.0), (1.0, 0.0)).with_stops([
            stop(0.0, 0.0),
            stop(0.0, 0.5),
            stop(0.3, 0.2),
            stop(1.0, 0.7),
            stop(1.0, 1.0),
        ]);
        snap_hard_stops(&mut gradient);
        let offsets: Vec<f32> = gradient.stops.iter().map(|stop| stop.offset).collect();
        assert_eq!(offsets, [0.0, 0.0, 0.3, 1.0, 1.0]);
    }

    #[test]
    fn every_stop_in_a_tie_moves_together() {
        let mut gradient = Gradient::new_linear((0.0, 0.0), (1.0, 0.0)).with_stops([
            stop(0.0, 0.0),
            stop(0.5, 0.0),
            stop(0.5, 0.5),
            stop(0.5, 1.0),
            stop(1.0, 1.0),
        ]);
        snap_hard_stops(&mut gradient);
        let snapped = (lut_entry(0.5) + 0.5) / LUT_ENTRIES;
        let offsets: Vec<f32> = gradient.stops.iter().map(|stop| stop.offset).collect();
        assert_eq!(offsets, [0.0, snapped, snapped, snapped, 1.0]);
    }

    #[test]
    fn unusable_offsets_stay_put() {
        let mut gradient = Gradient::new_linear((0.0, 0.0), (1.0, 0.0)).with_stops([
            stop(f32::NAN, 0.0),
            stop(f32::NAN, 0.5),
            stop(f32::INFINITY, 0.7),
            stop(f32::INFINITY, 1.0),
        ]);
        snap_hard_stops(&mut gradient);
        assert!(gradient.stops[..2].iter().all(|stop| stop.offset.is_nan()));
        assert!(
            gradient.stops[2..]
                .iter()
                .all(|stop| stop.offset == f32::INFINITY)
        );
        let mut empty = Gradient::new_linear((0.0, 0.0), (1.0, 0.0));
        snap_hard_stops(&mut empty);
        assert!(empty.stops.is_empty());
    }
}
