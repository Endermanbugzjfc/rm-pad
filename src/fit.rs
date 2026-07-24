//! Aspect-ratio fit modes for mapping the pen area into a target rectangle.
//!
//! The reMarkable's active area and the target (a screen, or the whole
//! desktop) rarely share an aspect ratio. The fit mode decides how the pen
//! area is placed inside the target:
//!
//! - [`FitMode::Fill`]: stretch to fill the target, ignoring aspect ratio
//!   (the historical behavior).
//! - [`FitMode::Contain`]: fit the whole pen area inside the target,
//!   preserving aspect ratio — letterboxed, so the pen cannot reach two of
//!   the target's edges.
//! - [`FitMode::Cover`]: cover the whole target, preserving aspect ratio —
//!   the pen area is cropped, its overflowing edges clamped to the target.
//!
//! The math is a pure function of the pen dimensions and a target rectangle,
//! so it is reused unchanged whether the target is the whole desktop or a
//! single display.

use serde::Deserialize;
use std::fmt;
use std::str::FromStr;

/// A rectangle in the compositor's logical coordinate space.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

/// How the pen's active area is fitted into the target rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FitMode {
    /// Stretch to fill the target, ignoring aspect ratio.
    #[default]
    Fill,
    /// Fit the whole pen area inside the target, preserving aspect ratio.
    Contain,
    /// Cover the whole target, preserving aspect ratio (pen edges cropped).
    Cover,
}

impl FitMode {
    /// The effective destination rectangle `(x, y, w, h)` that a pen area of
    /// `in_w x in_h` maps onto within target `(tx, ty, tw, th)`, centered.
    ///
    /// For [`FitMode::Fill`] this is exactly the target. For `Contain` it is
    /// the largest aspect-preserving rectangle that fits inside the target;
    /// for `Cover`, the smallest that covers it (so it may exceed the target).
    pub fn dest_rect(&self, in_w: i64, in_h: i64, t: Rect) -> Rect {
        let in_w = in_w.max(1);
        let in_h = in_h.max(1);

        // Compare pen aspect (in_w/in_h) with target aspect (t.w/t.h) via
        // cross-multiplication to stay in integer arithmetic.
        let width_limited = t.w * in_h <= t.h * in_w;
        let (ew, eh) = match self {
            FitMode::Fill => (t.w, t.h),
            // Contain shrinks to the tighter of the two axes.
            FitMode::Contain => {
                if width_limited {
                    (t.w, in_h * t.w / in_w)
                } else {
                    (in_w * t.h / in_h, t.h)
                }
            }
            // Cover grows to the looser of the two axes (opposite choice).
            FitMode::Cover => {
                if width_limited {
                    (in_w * t.h / in_h, t.h)
                } else {
                    (t.w, in_h * t.w / in_w)
                }
            }
        };

        Rect {
            x: t.x + (t.w - ew) / 2,
            y: t.y + (t.h - eh) / 2,
            w: ew,
            h: eh,
        }
    }

    /// Map a pen point in `0..=in_*` space into the target rectangle using
    /// this fit mode, clamped to the target so the cursor never leaves it
    /// (needed for [`FitMode::Cover`], harmless otherwise).
    pub fn map(&self, x: i64, y: i64, in_w: i64, in_h: i64, t: Rect) -> (i64, i64) {
        let d = self.dest_rect(in_w, in_h, t);
        let in_w = in_w.max(1);
        let in_h = in_h.max(1);
        let out_x = d.x + x * (d.w - 1).max(0) / in_w;
        let out_y = d.y + y * (d.h - 1).max(0) / in_h;
        (out_x.clamp(t.x, t.x + t.w - 1), out_y.clamp(t.y, t.y + t.h - 1))
    }
}

impl fmt::Display for FitMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FitMode::Fill => write!(f, "fill"),
            FitMode::Contain => write!(f, "contain"),
            FitMode::Cover => write!(f, "cover"),
        }
    }
}

impl FromStr for FitMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "fill" | "stretch" => Ok(FitMode::Fill),
            "contain" | "fit" => Ok(FitMode::Contain),
            "cover" => Ok(FitMode::Cover),
            _ => Err(format!(
                "Invalid fit mode '{}'. Valid values: fill, contain, cover",
                s
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A wide pen area (2:1) fitted into a tall target (1:2), origin at 0.
    const IN_W: i64 = 200;
    const IN_H: i64 = 100;
    const TARGET: Rect = Rect { x: 0, y: 0, w: 100, h: 200 };

    #[test]
    fn fill_uses_whole_target() {
        let d = FitMode::Fill.dest_rect(IN_W, IN_H, TARGET);
        assert_eq!((d.x, d.y, d.w, d.h), (0, 0, 100, 200));
    }

    #[test]
    fn contain_letterboxes_inside_target() {
        // Pen is 2:1, target is 100 wide -> height 50, centered vertically.
        let d = FitMode::Contain.dest_rect(IN_W, IN_H, TARGET);
        assert_eq!((d.w, d.h), (100, 50));
        assert_eq!((d.x, d.y), (0, 75)); // (200-50)/2
    }

    #[test]
    fn cover_overflows_and_centers() {
        // To cover a 200-tall target with a 2:1 pen, width -> 400, centered.
        let d = FitMode::Cover.dest_rect(IN_W, IN_H, TARGET);
        assert_eq!((d.w, d.h), (400, 200));
        assert_eq!((d.x, d.y), (-150, 0)); // (100-400)/2
    }

    #[test]
    fn cover_clamps_to_target() {
        // Extremes of the pen must stay within the target rectangle.
        let tl = FitMode::Cover.map(0, 0, IN_W, IN_H, TARGET);
        let br = FitMode::Cover.map(IN_W, IN_H, IN_W, IN_H, TARGET);
        assert_eq!(tl, (0, 0));
        assert_eq!(br, (TARGET.w - 1, TARGET.h - 1));
    }

    #[test]
    fn contain_maps_center_to_target_center() {
        let (cx, cy) = FitMode::Contain.map(IN_W / 2, IN_H / 2, IN_W, IN_H, TARGET);
        assert!((cx - TARGET.w / 2).abs() <= 1, "cx={cx}");
        assert!((cy - TARGET.h / 2).abs() <= 1, "cy={cy}");
    }

    #[test]
    fn fill_matches_plain_linear_mapping() {
        // Fill with an offset target must reproduce origin + x*(w-1)/in.
        let t = Rect { x: 10, y: 20, w: 100, h: 200 };
        let (x, y) = FitMode::Fill.map(IN_W, 0, IN_W, IN_H, t);
        assert_eq!((x, y), (10 + (t.w - 1), 20));
    }
}
