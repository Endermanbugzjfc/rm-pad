//! Multi-monitor support: confine pen output to a single display.
//!
//! The pen uinput device is absolute, so the compositor stretches its axis
//! range across the whole virtual desktop. To pin the pen to one monitor we
//! advertise axes covering the full desktop bounding box and remap incoming
//! pen coordinates into the chosen display's rectangle.
//!
//! `display-info` reports each display's geometry divided by its own scale,
//! so scaled/HiDPI monitors come back smaller and mis-placed. [`logical_rect`]
//! multiplies by `scale_factor` to recover the compositor's logical layout —
//! the space the pen is actually mapped into. `--screen all` disables mapping
//! and restores the whole-desktop behavior.

use display_info::DisplayInfo;

/// Sub-pixel precision factor applied to desktop pixel coordinates:
/// the uinput axes are advertised as `desktop_size_px * SCALE`, so one
/// axis unit is 1/SCALE of a pixel.
///
/// ### What if I take away this constant?
///
/// If the axes were sized in raw pixels, mapping onto a 1920-px-wide
/// screen would quantize the pen to 1920 positions, discarding most of
/// the digitizer's ~21k steps (`pen_x_max`) and causing visible
/// stair-stepping on slow diagonal strokes. The compositor scales
/// whatever range the device advertises down to the screen (with
/// sub-pixel precision internally), so inflating the range is free and
/// only changes the granularity we can express.
///
/// The value must satisfy `SCALE >= pen_x_max / target_screen_width_px`
/// so no digitizer steps collapse into the same axis unit. 16 covers
/// screens down to ~1310 px wide (20966 / 16) with room to spare on
/// anything larger; raise it if the pen should target very small
/// displays. Upper bound is far off: even a 32k-px-wide desktop at x16
/// is ~524k units, well within i32.
const SCALE: i64 = 16;

/// Maps orientation-transformed pen coordinates into one display's
/// rectangle within the virtual desktop spanned by all displays.
#[derive(Debug, Clone)]
pub struct ScreenMap {
    /// uinput ABS_X/ABS_Y maximums (virtual desktop size in scaled units).
    pub axis_x_max: i32,
    pub axis_y_max: i32,
    /// Target display rectangle in scaled units, relative to the top-left
    /// corner of the virtual desktop bounding box.
    target_x: i64,
    target_y: i64,
    target_w: i64,
    target_h: i64,
    pub label: String,
}

impl ScreenMap {
    /// Map a pen coordinate in `0..=in_max` space into the target display.
    pub fn map(&self, x: i32, y: i32, in_x_max: i32, in_y_max: i32) -> (i32, i32) {
        let out_x = self.target_x + x as i64 * (self.target_w - 1) / in_x_max.max(1) as i64;
        let out_y = self.target_y + y as i64 * (self.target_h - 1) / in_y_max.max(1) as i64;
        (out_x as i32, out_y as i32)
    }
}

/// Resolve the configured screen selection to a coordinate mapping.
///
/// Returns `None` when the pen should span the whole desktop (the default,
/// backward-compatible behavior): no selection was given, the selection is
/// "all", the selection matches no display, or enumeration is unavailable.
/// A mapping is only produced when the user explicitly names a screen.
pub fn resolve(selection: Option<&str>) -> Option<ScreenMap> {
    let selection = match selection {
        // Unset or "all" => no mapping, span every screen (original behavior).
        None => return None,
        Some(s) if s.eq_ignore_ascii_case("all") => return None,
        Some(s) => s,
    };

    let displays = match DisplayInfo::all() {
        Ok(d) if !d.is_empty() => d,
        Ok(_) => {
            log::warn!("No displays detected, pen will span the whole desktop");
            return None;
        }
        Err(e) => {
            log::warn!(
                "Failed to enumerate displays ({}), pen will span the whole desktop",
                e
            );
            return None;
        }
    };

    let target = match find_display(&displays, selection) {
        Some(d) => d,
        None => {
            log::error!(
                "No display matches '{}' (available: {}), pen will span the whole desktop",
                selection,
                display_names(&displays)
            );
            return None;
        }
    };

    Some(build_map(&displays, target))
}

/// Find a display by index, exact name, or case-insensitive substring.
fn find_display<'a>(displays: &'a [DisplayInfo], selection: &str) -> Option<&'a DisplayInfo> {
    if let Ok(index) = selection.parse::<usize>() {
        return displays.get(index);
    }

    let lower = selection.to_lowercase();
    displays
        .iter()
        .find(|d| d.name.eq_ignore_ascii_case(selection))
        .or_else(|| {
            displays.iter().find(|d| {
                d.name.to_lowercase().contains(&lower)
                    || d.friendly_name.to_lowercase().contains(&lower)
            })
        })
}

fn display_names(displays: &[DisplayInfo]) -> String {
    displays
        .iter()
        .map(|d| d.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// A display rectangle in the compositor's logical coordinate space.
#[derive(Debug, Clone, Copy)]
struct Rect {
    x: i64,
    y: i64,
    w: i64,
    h: i64,
}

/// Recover a display's logical rectangle from `display-info`.
///
/// `display-info` reports x/y/width/height divided by the display's own
/// scale, so a fractional/HiDPI monitor comes back smaller and mis-placed
/// (e.g. a 1600x1000-at-(0,1080) output is reported as 800x500-at-(0,540)).
/// Multiplying by `scale_factor` restores the compositor's logical layout,
/// which is the space the pen is mapped into.
fn logical_rect(d: &DisplayInfo) -> Rect {
    let s = if d.scale_factor > 0.0 { d.scale_factor as f64 } else { 1.0 };
    Rect {
        x: (d.x as f64 * s).round() as i64,
        y: (d.y as f64 * s).round() as i64,
        w: (d.width as f64 * s).round() as i64,
        h: (d.height as f64 * s).round() as i64,
    }
}

fn build_map(displays: &[DisplayInfo], target: &DisplayInfo) -> ScreenMap {
    let rects: Vec<Rect> = displays.iter().map(logical_rect).collect();
    let target_rect = logical_rect(target);
    let label = format!(
        "{} ({}x{} at {},{})",
        target.name, target_rect.w, target_rect.h, target_rect.x, target_rect.y
    );
    compute_map(&rects, &target_rect, label)
}

/// Build the pen-to-display mapping from logical display rectangles.
///
/// Pure geometry over the virtual-desktop bounding box, split out so it can
/// be tested without a live display connection.
fn compute_map(all: &[Rect], target: &Rect, label: String) -> ScreenMap {
    let min_x = all.iter().map(|r| r.x).min().unwrap();
    let min_y = all.iter().map(|r| r.y).min().unwrap();
    let max_x = all.iter().map(|r| r.x + r.w).max().unwrap();
    let max_y = all.iter().map(|r| r.y + r.h).max().unwrap();

    ScreenMap {
        axis_x_max: ((max_x - min_x) * SCALE - 1) as i32,
        axis_y_max: ((max_y - min_y) * SCALE - 1) as i32,
        target_x: (target.x - min_x) * SCALE,
        target_y: (target.y - min_y) * SCALE,
        target_w: target.w * SCALE,
        target_h: target.h * SCALE,
        label,
    }
}

/// Print connected displays for the `screens` subcommand.
pub fn print_displays() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let displays = DisplayInfo::all().map_err(|e| format!("Failed to enumerate displays: {}", e))?;

    if displays.is_empty() {
        println!("No displays found");
        return Ok(());
    }

    for (index, d) in displays.iter().enumerate() {
        let friendly = if d.friendly_name.is_empty() || d.friendly_name == d.name {
            String::new()
        } else {
            format!(" \"{}\"", d.friendly_name)
        };
        let r = logical_rect(d);
        println!(
            "{}: {}{} — {}x{} at ({}, {}){}",
            index,
            d.name,
            friendly,
            r.w,
            r.h,
            r.x,
            r.y,
            if d.is_primary { " [primary]" } else { "" }
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_for(target_x: i64, target_y: i64, target_w: i64, target_h: i64, desk_w: i64, desk_h: i64) -> ScreenMap {
        ScreenMap {
            axis_x_max: (desk_w * SCALE - 1) as i32,
            axis_y_max: (desk_h * SCALE - 1) as i32,
            target_x: target_x * SCALE,
            target_y: target_y * SCALE,
            target_w: target_w * SCALE,
            target_h: target_h * SCALE,
            label: "test".into(),
        }
    }

    #[test]
    fn maps_full_pen_range_onto_target_rect() {
        // Second monitor of a side-by-side 1920x1080 pair
        let map = map_for(1920, 0, 1920, 1080, 3840, 1080);

        assert_eq!(map.map(0, 0, 20966, 15725), (1920 * SCALE as i32, 0));
        let (x, y) = map.map(20966, 15725, 20966, 15725);
        assert_eq!(x, (3840 * SCALE - 1) as i32);
        assert_eq!(y, (1080 * SCALE - 1) as i32);
    }

    #[test]
    fn midpoint_lands_in_target_center() {
        let map = map_for(1920, 0, 1920, 1080, 3840, 1080);
        let (x, y) = map.map(20966 / 2, 15725 / 2, 20966, 15725);
        let cx = (1920 + 960) * SCALE as i32;
        let cy = 540 * SCALE as i32;
        assert!((x - cx).abs() <= SCALE as i32);
        assert!((y - cy).abs() <= SCALE as i32);
    }

    fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn compute_map_uses_full_logical_bounding_box() {
        // The real 3-monitor logical layout: DP-2 and HDMI side by side,
        // plus a scaled monitor below that extends the desktop to 2080 tall.
        let dp2 = rect(0, 0, 1920, 1080);
        let hdmi = rect(1920, 0, 1920, 1080);
        let dp3 = rect(0, 1080, 1600, 1000);
        let all = [dp2, hdmi, dp3];

        let map = compute_map(&all, &hdmi, "hdmi".into());

        // Desktop bounding box is 3840x2080, not 3840x1080.
        assert_eq!(map.axis_x_max, (3840 * SCALE - 1) as i32);
        assert_eq!(map.axis_y_max, (2080 * SCALE - 1) as i32);

        // Pen center must land at HDMI's center within the whole desktop:
        // x = 2880/3840 = 0.75, y = 540/2080 ≈ 0.26 of the axis range.
        let (x, y) = map.map(20966 / 2, 15725 / 2, 20966, 15725);
        let frac_x = x as f64 / (map.axis_x_max as f64);
        let frac_y = y as f64 / (map.axis_y_max as f64);
        assert!((frac_x - 0.75).abs() < 0.01, "frac_x={frac_x}");
        assert!((frac_y - 540.0 / 2080.0).abs() < 0.01, "frac_y={frac_y}");
    }
}
