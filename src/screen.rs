//! Fit the pen area onto the whole desktop with a chosen aspect-ratio mode.
//!
//! The pen uinput device is absolute, so the compositor stretches its axis
//! range across the whole virtual desktop. To apply a [`FitMode`] we advertise
//! axes covering the full desktop bounding box (so the compositor maps them
//! 1:1) and remap incoming pen coordinates into a fitted rectangle within it.
//!
//! `display-info` reports each display's geometry divided by its own scale, so
//! scaled/HiDPI monitors come back smaller and mis-placed. [`logical_rect`]
//! multiplies by `scale_factor` to recover the compositor's logical layout —
//! the space the pen is mapped into.
//!
//! This module is deliberately shaped like the multi-screen mapping so the two
//! can be merged with minimal changes: [`ScreenMap`] carries a target rectangle
//! plus a [`FitMode`], and only [`resolve`] differs — here the target is always
//! the whole desktop, driven by the fit mode rather than a screen selection.

use display_info::DisplayInfo;

use crate::fit::{FitMode, Rect};

/// Sub-pixel precision factor applied to desktop pixel coordinates: the uinput
/// axes are advertised as `desktop_size_px * SCALE`, so one axis unit is
/// 1/SCALE of a pixel. Without it, mapping onto screen-sized ranges would
/// quantize the pen far below its ~21k-step digitizer resolution and stair-step
/// on slow strokes. Inflating the range is free (the compositor scales it back
/// down), so we keep generous headroom; 16 stays well within `i32`.
const SCALE: i64 = 16;

/// Maps orientation-transformed pen coordinates into a target rectangle within
/// the virtual desktop, applying a [`FitMode`].
#[derive(Debug, Clone)]
pub struct ScreenMap {
    /// uinput ABS_X/ABS_Y maximums (virtual desktop size in scaled units).
    pub axis_x_max: i32,
    pub axis_y_max: i32,
    /// Target rectangle in scaled units, relative to the desktop's top-left.
    target: Rect,
    fit: FitMode,
    pub label: String,
}

impl ScreenMap {
    /// Map a pen coordinate in `0..=in_max` space into the target rectangle.
    pub fn map(&self, x: i32, y: i32, in_x_max: i32, in_y_max: i32) -> (i32, i32) {
        let (out_x, out_y) =
            self.fit.map(x as i64, y as i64, in_x_max as i64, in_y_max as i64, self.target);
        (out_x as i32, out_y as i32)
    }
}

/// Resolve the configured fit mode to a coordinate mapping over the desktop.
///
/// Returns `None` for [`FitMode::Fill`] (the default): the pen keeps spanning
/// the whole desktop the historical way, with no display enumeration needed.
/// `Contain`/`Cover` need the desktop geometry; if it can't be read the pen
/// falls back to the same whole-desktop stretch.
pub fn resolve(fit: FitMode) -> Option<ScreenMap> {
    if fit == FitMode::Fill {
        return None;
    }

    let displays = match DisplayInfo::all() {
        Ok(d) if !d.is_empty() => d,
        Ok(_) => {
            log::warn!("No displays detected, pen will stretch across the whole desktop");
            return None;
        }
        Err(e) => {
            log::warn!(
                "Failed to enumerate displays ({}), pen will stretch across the whole desktop",
                e
            );
            return None;
        }
    };

    Some(build_desktop_map(&displays, fit))
}

/// Recover a display's logical rectangle from `display-info`.
///
/// `display-info` reports x/y/width/height divided by the display's own scale,
/// so a fractional/HiDPI monitor comes back smaller and mis-placed (e.g. a
/// 1600x1000-at-(0,1080) output is reported as 800x500-at-(0,540)). Multiplying
/// by `scale_factor` restores the compositor's logical layout.
fn logical_rect(d: &DisplayInfo) -> Rect {
    let s = if d.scale_factor > 0.0 { d.scale_factor as f64 } else { 1.0 };
    Rect {
        x: (d.x as f64 * s).round() as i64,
        y: (d.y as f64 * s).round() as i64,
        w: (d.width as f64 * s).round() as i64,
        h: (d.height as f64 * s).round() as i64,
    }
}

fn build_desktop_map(displays: &[DisplayInfo], fit: FitMode) -> ScreenMap {
    let rects: Vec<Rect> = displays.iter().map(logical_rect).collect();
    compute_desktop_map(&rects, fit)
}

/// Build a whole-desktop mapping from logical display rectangles.
///
/// The target rectangle is the full desktop bounding box; [`FitMode`] then
/// decides how the pen area sits inside it. Split out for testing without a
/// live display connection.
fn compute_desktop_map(rects: &[Rect], fit: FitMode) -> ScreenMap {
    let min_x = rects.iter().map(|r| r.x).min().unwrap();
    let min_y = rects.iter().map(|r| r.y).min().unwrap();
    let max_x = rects.iter().map(|r| r.x + r.w).max().unwrap();
    let max_y = rects.iter().map(|r| r.y + r.h).max().unwrap();
    let (w, h) = (max_x - min_x, max_y - min_y);

    ScreenMap {
        axis_x_max: (w * SCALE - 1) as i32,
        axis_y_max: (h * SCALE - 1) as i32,
        target: Rect { x: 0, y: 0, w: w * SCALE, h: h * SCALE },
        fit,
        label: format!("whole desktop {}x{} ({} fit)", w, h, fit),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn desktop_map_spans_full_logical_bounding_box() {
        // DP-2 + HDMI side by side, plus a scaled monitor below (2080 tall).
        let rects = [rect(0, 0, 1920, 1080), rect(1920, 0, 1920, 1080), rect(0, 1080, 1600, 1000)];
        let map = compute_desktop_map(&rects, FitMode::Contain);
        assert_eq!(map.axis_x_max, (3840 * SCALE - 1) as i32);
        assert_eq!(map.axis_y_max, (2080 * SCALE - 1) as i32);
    }

    #[test]
    fn contain_letterboxes_wide_pen_on_tall_desktop() {
        // Single tall desktop 1000x2000; a 2:1 pen must letterbox to a
        // 1000x500 band centered vertically.
        let map = compute_desktop_map(&[rect(0, 0, 1000, 2000)], FitMode::Contain);
        // Pen top-left -> left edge, and vertically inside the band (y>0).
        let (x0, y0) = map.map(0, 0, 2000, 1000);
        let (x1, y1) = map.map(2000, 1000, 2000, 1000);
        assert_eq!(x0, 0);
        assert_eq!(x1, (1000 * SCALE - 1) as i32);
        assert!(y0 > 0, "top should be letterboxed down, y0={y0}");
        assert!(y1 < (2000 * SCALE - 1) as i32, "bottom letterboxed up, y1={y1}");
    }

    #[test]
    fn fill_stretches_corner_to_corner() {
        let map = compute_desktop_map(&[rect(0, 0, 1000, 2000)], FitMode::Fill);
        assert_eq!(map.map(0, 0, 2000, 1000), (0, 0));
        assert_eq!(
            map.map(2000, 1000, 2000, 1000),
            ((1000 * SCALE - 1) as i32, (2000 * SCALE - 1) as i32)
        );
    }
}
