//! Shared display-geometry helpers over `display-info`.
//!
//! Enumeration happens once here (`enumerate`) so the pen-mapping strategies
//! don't each call `DisplayInfo::all()`. `logical_rect` recovers the
//! compositor's logical layout, and `desktop_bounds` gives the bounding box the
//! pen is fitted against.

use display_info::DisplayInfo;

/// A display rectangle in the compositor's logical coordinate space.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

/// Enumerate connected displays, or `None` if none are available.
///
/// This is the single `DisplayInfo::all()` call site: it logs and returns
/// `None` on error or when no displays are reported, so callers have one place
/// that handles "display info is unavailable" and fall back to whole-desktop
/// behavior.
pub fn enumerate() -> Option<Vec<DisplayInfo>> {
    match DisplayInfo::all() {
        Ok(d) if !d.is_empty() => Some(d),
        Ok(_) => {
            log::warn!("No displays detected, pen will span the whole desktop");
            None
        }
        Err(e) => {
            log::warn!(
                "Failed to enumerate displays ({}), pen will span the whole desktop",
                e
            );
            None
        }
    }
}

/// Recover a display's logical rectangle from `display-info`.
///
/// `display-info` reports x/y/width/height divided by the display's own scale,
/// so a fractional/HiDPI monitor comes back smaller and mis-placed (e.g. a
/// 1600x1000-at-(0,1080) output is reported as 800x500-at-(0,540)). Multiplying
/// by `scale_factor` restores the compositor's logical layout, which is the
/// space the pen is mapped into.
pub fn logical_rect(d: &DisplayInfo) -> Rect {
    let s = if d.scale_factor > 0.0 { d.scale_factor as f64 } else { 1.0 };
    Rect {
        x: (d.x as f64 * s).round() as i64,
        y: (d.y as f64 * s).round() as i64,
        w: (d.width as f64 * s).round() as i64,
        h: (d.height as f64 * s).round() as i64,
    }
}

/// The bounding box spanning every display, in logical coordinates.
pub fn desktop_bounds(displays: &[DisplayInfo]) -> Rect {
    let rects: Vec<Rect> = displays.iter().map(logical_rect).collect();
    let min_x = rects.iter().map(|r| r.x).min().unwrap();
    let min_y = rects.iter().map(|r| r.y).min().unwrap();
    let max_x = rects.iter().map(|r| r.x + r.w).max().unwrap();
    let max_y = rects.iter().map(|r| r.y + r.h).max().unwrap();
    Rect { x: min_x, y: min_y, w: max_x - min_x, h: max_y - min_y }
}
