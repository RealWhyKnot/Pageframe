use euclid::default::{Rect, Transform3D};
use kurbo::Affine;
use style::{
    properties::generated::style_structs::Box as BoxStyleStruct,
    values::{
        computed::{CSSPixelLength, Rotate, transform::TransformOperation},
        generics::transform::{Scale, Transform, Translate},
    },
};

// 6. Current Transformation Matrix
//
// The transformation matrix is computed from the transform, transform-origin, translate, rotate, scale, and offset properties as follows:
//
//   - Start with the identity matrix.
//   - Translate by the computed X, Y, and Z values of transform-origin.
//   - Translate by the computed X, Y, and Z values of translate.
//   - Rotate by the computed <angle> about the specified axis of rotate.
//   - Scale by the computed X, Y, and Z values of scale.
//   - Translate and rotate by the transform specified by offset.
//   - Multiply by each of the transform functions in transform from left to right.
//   - Translate by the negated computed X, Y and Z values of transform-origin.
//
// <https://drafts.csswg.org/css-transforms-2/#ctm>
//
// The whole matrix is built in 3D and flattened once at the end: flattening each
// step on its own goes wrong as soon as one rotation tips the plane and a later
// one brings its depth back into view.
pub fn resolve_2d_transform(
    box_styles: &BoxStyleStruct,
    reference_box: Rect<CSSPixelLength>,
) -> Option<Affine> {
    let mut operations = Vec::new();

    if let Translate::Translate(x, y, z) = &box_styles.translate {
        operations.push(TransformOperation::Translate3D(x.clone(), y.clone(), *z));
    }

    match &box_styles.rotate {
        Rotate::None => {}
        Rotate::Rotate(angle) => operations.push(TransformOperation::Rotate(*angle)),
        Rotate::Rotate3D(x, y, z, angle) => {
            operations.push(TransformOperation::Rotate3D(*x, *y, *z, *angle))
        }
    }

    if let Scale::Scale(x, y, z) = &box_styles.scale {
        operations.push(TransformOperation::Scale3D(*x, *y, *z));
    }

    // TODO: support the "offset" property
    // <https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/offset>

    operations.extend(box_styles.transform.0.iter().cloned());
    if operations.is_empty() {
        return None;
    }

    let (matrix, _has_3d) = Transform(operations.into())
        .to_transform_3d_matrix_f64(Some(&reference_box))
        .ok()?;

    // Apply the transform origin by:
    //   - Translating by the origin offset
    //   - Applying our transform
    //   - Translating by the inverse of the origin offset
    let transform_origin = &box_styles.transform_origin;
    let origin_x = transform_origin
        .horizontal
        .resolve(reference_box.width())
        .px() as f64;
    let origin_y = transform_origin
        .vertical
        .resolve(reference_box.height())
        .px() as f64;
    let origin_z = transform_origin.depth.px() as f64;
    let matrix = Transform3D::translation(-origin_x, -origin_y, -origin_z)
        .then(&matrix)
        .then(&Transform3D::translation(origin_x, origin_y, origin_z));

    let resolved = flatten(
        &matrix,
        kurbo::Rect::new(
            reference_box.min_x().px() as f64,
            reference_box.min_y().px() as f64,
            reference_box.max_x().px() as f64,
            reference_box.max_y().px() as f64,
        ),
    )?;
    if resolved != Affine::IDENTITY {
        Some(resolved)
    } else {
        None
    }
}

/// Project a 3D transform onto the plane of the screen, as painting a flat element
/// with it does outside a 3D rendering context.
///
/// The element lies in the z = 0 plane, so only the x, y and w rows of the matrix
/// act on it. Without perspective w stays constant and the result is exactly
/// affine. With perspective the true result is a projective map, which an affine
/// transform can't express, so the affine transform that best fits (in the
/// least-squares sense) where the corners of `bounds` are projected to is used
/// instead. The element isn't painted when part of it ends up behind the viewer.
///
/// See: https://drafts.csswg.org/css-transforms-2/#3d-transform-rendering
fn flatten(m: &Transform3D<f64>, bounds: kurbo::Rect) -> Option<Affine> {
    let has_perspective = m.m14 != 0.0 || m.m24 != 0.0;
    if !has_perspective || bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        if m.m44 == 0.0 || !m.m44.is_finite() {
            return None;
        }
        let affine = Affine::new([m.m11, m.m12, m.m21, m.m22, m.m41, m.m42].map(|v| v / m.m44));
        return affine.is_finite().then_some(affine);
    }

    let project = |x: f64, y: f64| {
        m.transform_point2d(euclid::default::Point2D::new(x, y))
            .map(|point| kurbo::Vec2::new(point.x, point.y))
    };
    let top_left = project(bounds.x0, bounds.y0)?;
    let top_right = project(bounds.x1, bounds.y0)?;
    let bottom_right = project(bounds.x1, bounds.y1)?;
    let bottom_left = project(bounds.x0, bounds.y1)?;

    // For the four corners of a rectangle the least-squares fit has a closed form:
    // the centre maps to the mean of the projected corners, and each axis to the
    // difference between the means of the opposite edges.
    let x_axis = ((top_right + bottom_right) - (top_left + bottom_left)) / (2.0 * bounds.width());
    let y_axis = ((bottom_left + bottom_right) - (top_left + top_right)) / (2.0 * bounds.height());
    let centre = (top_left + top_right + bottom_right + bottom_left) / 4.0;
    let local_centre = bounds.center().to_vec2();
    let offset = centre - x_axis * local_centre.x - y_axis * local_centre.y;
    let affine = Affine::new([x_axis.x, x_axis.y, y_axis.x, y_axis.y, offset.x, offset.y]);
    affine.is_finite().then_some(affine)
}
