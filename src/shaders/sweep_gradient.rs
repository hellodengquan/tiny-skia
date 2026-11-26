// Copyright 2006 The Android Open Source Project
//
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use alloc::vec::Vec;

use tiny_skia_path::Scalar;

use crate::{ColorSpace, GradientStop, Point, Shader, SpreadMode, Transform};

use super::gradient::{Gradient, DEGENERATE_THRESHOLD};
use crate::pipeline::{RasterPipelineBuilder, Stage, SweepGradientCtx};

#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use tiny_skia_path::NoStdFloat;

/// A sweep gradient shader.
#[derive(Clone, PartialEq, Debug)]
pub struct SweepGradient {
    pub(crate) base: Gradient,
    t0: f32,
    t1: f32,
    /// Whether the clockwise arc from `t0` to `t1` crosses the zero-degree seam.
    crosses_zero: bool,
}

/// Normalizes an angle expressed in turns into the [0, 1) range.
#[inline]
fn normalize_turns(t: f32) -> f32 {
    #[cfg(all(not(feature = "std"), feature = "no-std-float"))]
    {
        t - t.floor()
    }
    #[cfg(not(all(not(feature = "std"), feature = "no-std-float")))]
    {
        t.rem_euclid(1.0)
    }
}

impl SweepGradient {
    /// Creates a new sweep gradient shader.
    ///
    /// Angles are measured in degrees, clockwise from the positive x-axis
    /// (three o'clock), matching the screen coordinate system.
    ///
    /// When `start_angle <= end_angle` the gradient's stops are spread along
    /// the clockwise arc between them, just like a regular angular interval.
    ///
    /// When `start_angle > end_angle` the arc wraps around the zero-degree
    /// seam: it runs clockwise from `start_angle` to 360° and then continues
    /// from 0° to `end_angle`. This lets the caller explicitly choose the
    /// clockwise start/end direction of an arc that spans across zero degrees.
    /// Outside of the chosen arc, colors are taken according to `mode`, so
    /// every direction within a full turn gets a definite color.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        center: Point,
        start_angle: f32,
        end_angle: f32,
        stops: Vec<GradientStop>,
        mut mode: SpreadMode,
        transform: Transform,
    ) -> Option<Shader<'static>> {
        if !start_angle.is_finite() || !end_angle.is_finite() {
            return None;
        }

        match stops.as_slice() {
            [] => return None,
            [stop] => return Some(Shader::SolidColor(stop.color)),
            _ => (),
        }
        transform.invert()?;

        let crosses_zero = start_angle > end_angle;
        if !crosses_zero
            && start_angle.is_nearly_equal_within_tolerance(end_angle, DEGENERATE_THRESHOLD)
        {
            if mode == SpreadMode::Pad && end_angle > DEGENERATE_THRESHOLD {
                // In this case, the first color is repeated from 0 to the angle, then a hardstop
                // switches to the last color (all other colors are compressed to the infinitely
                // thin interpolation region).
                let front_color = stops.first().unwrap().color;
                let back_color = stops.last().unwrap().color;
                let mut new_stops = stops;
                new_stops.clear();
                new_stops.extend_from_slice(&[
                    GradientStop::new(0.0, front_color),
                    GradientStop::new(1.0, front_color),
                    GradientStop::new(1.0, back_color),
                ]);
                return SweepGradient::new(center, 0.0, end_angle, new_stops, mode, transform);
            }
            // TODO: Consider making a degenerate fallback shader similar to Skia. Tiny Skia
            // currently opts to return `None` in some places.
            return None;
        }

        let (t0, t1, crosses_zero) = if crosses_zero {
            // The arc crosses the zero-degree seam. Reduce both endpoints to
            // turns in the [0, 1) range; the arc goes t0 .. 1 and 0 .. t1.
            let t0 = normalize_turns(start_angle / 360.0);
            let t1 = normalize_turns(end_angle / 360.0);
            // The clockwise span is `(1 - t0) + t1`. A span that small (or a
            // full turn shrunk to nothing by the tolerance) cannot hold a
            // gradient.
            if (1.0 - t0) + t1 <= DEGENERATE_THRESHOLD {
                return None;
            }
            (t0, t1, true)
        } else {
            if start_angle <= 0.0 && end_angle >= 360.0 {
                mode = SpreadMode::Pad;
            }
            (start_angle / 360.0, end_angle / 360.0, false)
        };

        Some(Shader::SweepGradient(SweepGradient {
            base: Gradient::new(
                stops,
                mode,
                transform,
                Transform::from_translate(-center.x, -center.y),
            ),
            t0,
            t1,
            crosses_zero,
        }))
    }

    pub(crate) fn is_opaque(&self) -> bool {
        self.base.colors_are_opaque
    }

    pub(crate) fn push_stages(&self, cs: ColorSpace, p: &mut RasterPipelineBuilder) -> bool {
        if self.crosses_zero {
            // The arc covers `(1 - t0) + t1` turns; map that span onto [0, 1].
            let span = (1.0 - self.t0) + self.t1;
            p.ctx.sweep_gradient = SweepGradientCtx {
                t0: self.t0,
                scale: 1.0 / span,
            };
            self.base.push_stages(
                p,
                cs,
                &|p| {
                    p.push(Stage::XYToUnitAngle);
                    p.push(Stage::SweepScaleBias);
                },
                &|_| {},
            )
        } else {
            let scale = 1.0 / (self.t1 - self.t0);
            let bias = -scale * self.t0;
            p.ctx.two_point_conical_gradient.p0 = scale;
            p.ctx.two_point_conical_gradient.p1 = bias;
            self.base.push_stages(
                p,
                cs,
                &|p| {
                    p.push(Stage::XYToUnitAngle);
                    if scale != 1.0 && bias != 0.0 {
                        p.push(Stage::ApplyConcentricScaleBias)
                    }
                },
                &|_| {},
            )
        }
    }
}
