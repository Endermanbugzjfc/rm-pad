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
    pub fn dest_rect(
        &self,
        in_w: i64,
        in_h: i64,
        tx: i64,
        ty: i64,
        tw: i64,
        th: i64,
    ) -> (i64, i64, i64, i64) {
        let in_w = in_w.max(1);
        let in_h = in_h.max(1);

        // Compare pen aspect (in_w/in_h) with target aspect (tw/th) via
        // cross-multiplication to stay in integer arithmetic.
        let width_limited = tw * in_h <= th * in_w;
        let (ew, eh) = match self {
            FitMode::Fill => (tw, th),
            // Contain shrinks to the tighter of the two axes.
            FitMode::Contain => {
                if width_limited {
                    (tw, in_h * tw / in_w)
                } else {
                    (in_w * th / in_h, th)
                }
            }
            // Cover grows to the looser of the two axes (opposite choice).
            FitMode::Cover => {
                if width_limited {
                    (in_w * th / in_h, th)
                } else {
                    (tw, in_h * tw / in_w)
                }
            }
        };

        let ox = tx + (tw - ew) / 2;
        let oy = ty + (th - eh) / 2;
        (ox, oy, ew, eh)
    }

    /// Map a pen point in `0..=in_*` space into the target rectangle using
    /// this fit mode, clamped to the target so the cursor never leaves it
    /// (needed for [`FitMode::Cover`], harmless otherwise).
    // Scalar target params (rather than a Rect) mirror the multiscreen
    // `ScreenMap`'s `target_x/y/w/h` fields, to keep the eventual merge small.
    #[allow(clippy::too_many_arguments)]
    pub fn map(
        &self,
        x: i64,
        y: i64,
        in_w: i64,
        in_h: i64,
        tx: i64,
        ty: i64,
        tw: i64,
        th: i64,
    ) -> (i64, i64) {
        let (ox, oy, ew, eh) = self.dest_rect(in_w, in_h, tx, ty, tw, th);
        let in_w = in_w.max(1);
        let in_h = in_h.max(1);
        let out_x = ox + x * (ew - 1).max(0) / in_w;
        let out_y = oy + y * (eh - 1).max(0) / in_h;
        (out_x.clamp(tx, tx + tw - 1), out_y.clamp(ty, ty + th - 1))
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
                s,
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
    const TW: i64 = 100;
    const TH: i64 = 200;

    #[test]
    fn fill_uses_whole_target() {
        assert_eq!(FitMode::Fill.dest_rect(IN_W, IN_H, 0, 0, TW, TH), (0, 0, 100, 200));
    }

    #[test]
    fn contain_letterboxes_inside_target() {
        // Pen is 2:1, target is 100 wide -> height 50, centered vertically.
        let (x, y, w, h) = FitMode::Contain.dest_rect(IN_W, IN_H, 0, 0, TW, TH);
        assert_eq!((w, h), (100, 50));
        assert_eq!((x, y), (0, 75)); // (200-50)/2
    }

    #[test]
    fn cover_overflows_and_centers() {
        // To cover a 200-tall target with a 2:1 pen, width -> 400, centered.
        let (x, y, w, h) = FitMode::Cover.dest_rect(IN_W, IN_H, 0, 0, TW, TH);
        assert_eq!((w, h), (400, 200));
        assert_eq!((x, y), (-150, 0)); // (100-400)/2
    }

    #[test]
    fn cover_clamps_to_target() {
        // Extremes of the pen must stay within the target rectangle.
        let tl = FitMode::Cover.map(0, 0, IN_W, IN_H, 0, 0, TW, TH);
        let br = FitMode::Cover.map(IN_W, IN_H, IN_W, IN_H, 0, 0, TW, TH);
        assert_eq!(tl, (0, 0));
        assert_eq!(br, (TW - 1, TH - 1));
    }

    #[test]
    fn contain_maps_center_to_target_center() {
        let (cx, cy) = FitMode::Contain.map(IN_W / 2, IN_H / 2, IN_W, IN_H, 0, 0, TW, TH);
        assert!((cx - TW / 2).abs() <= 1, "cx={cx}");
        assert!((cy - TH / 2).abs() <= 1, "cy={cy}");
    }

    #[test]
    fn fill_matches_plain_linear_mapping() {
        // Fill with an offset target must reproduce origin + x*(w-1)/in.
        let (x, y) = FitMode::Fill.map(IN_W, 0, IN_W, IN_H, 10, 20, TW, TH);
        assert_eq!((x, y), (10 + (TW - 1), 20));
    }
}
