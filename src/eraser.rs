//! Configurable behaviour for the pen's eraser end.
//!
//! The reMarkable stylus reports `BTN_TOOL_RUBBER` when flipped to the eraser
//! end. This enum lets the user map eraser contact to a mouse button, a
//! touchpad-style pointer, or nothing at all.

use evdevil::event::Key;
use serde::Deserialize;
use std::fmt;
use std::str::FromStr;

/// What the eraser end of the pen should do when it touches the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EraserAction {
    /// Ignore the eraser entirely (cursor may hover, but no click).
    None,
    /// Hold the right mouse button while the eraser is in contact.
    #[default]
    RightClick,
    /// Hold the left mouse button while the eraser is in contact.
    LeftClick,
    /// Hold the middle mouse button while the eraser is in contact.
    MiddleClick,
    /// Drive a dedicated touchpad-style pointer device (relative motion + tap).
    Touchpad,
    /// Reserved: drive an absolute touchscreen device. Not yet implemented.
    Touchscreen,
}

impl EraserAction {
    /// The mouse button held while the eraser touches, for click actions.
    pub fn button_key(&self) -> Option<Key> {
        match self {
            EraserAction::LeftClick => Some(Key::BTN_LEFT),
            EraserAction::RightClick => Some(Key::BTN_RIGHT),
            EraserAction::MiddleClick => Some(Key::BTN_MIDDLE),
            _ => None,
        }
    }

    /// Whether this action routes the eraser through a touchpad-style device.
    pub fn is_touchpad(&self) -> bool {
        matches!(self, EraserAction::Touchpad)
    }

    /// Whether this action is implemented. `Touchscreen` is reserved for later.
    pub fn is_implemented(&self) -> bool {
        !matches!(self, EraserAction::Touchscreen)
    }
}

impl fmt::Display for EraserAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EraserAction::None => write!(f, "none"),
            EraserAction::RightClick => write!(f, "right-click"),
            EraserAction::LeftClick => write!(f, "left-click"),
            EraserAction::MiddleClick => write!(f, "middle-click"),
            EraserAction::Touchpad => write!(f, "touchpad"),
            EraserAction::Touchscreen => write!(f, "touchscreen"),
        }
    }
}

impl FromStr for EraserAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" => Ok(EraserAction::None),
            "right-click" | "rightclick" | "right_click" => Ok(EraserAction::RightClick),
            "left-click" | "leftclick" | "left_click" => Ok(EraserAction::LeftClick),
            "middle-click" | "middleclick" | "middle_click" => Ok(EraserAction::MiddleClick),
            "touchpad" => Ok(EraserAction::Touchpad),
            "touchscreen" => Ok(EraserAction::Touchscreen),
            _ => Err(format!(
                "Invalid eraser action '{}'. Valid values: none, left-click, middle-click, right-click, touchpad, touchscreen",
                s
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_right_click() {
        assert_eq!(EraserAction::default(), EraserAction::RightClick);
    }

    #[test]
    fn test_from_str() {
        assert_eq!("none".parse::<EraserAction>().unwrap(), EraserAction::None);
        assert_eq!("right-click".parse::<EraserAction>().unwrap(), EraserAction::RightClick);
        assert_eq!("left_click".parse::<EraserAction>().unwrap(), EraserAction::LeftClick);
        assert_eq!("MiddleClick".parse::<EraserAction>().unwrap(), EraserAction::MiddleClick);
        assert_eq!("touchpad".parse::<EraserAction>().unwrap(), EraserAction::Touchpad);
        assert_eq!("touchscreen".parse::<EraserAction>().unwrap(), EraserAction::Touchscreen);
        assert!("invalid".parse::<EraserAction>().is_err());
    }

    #[test]
    fn test_display_roundtrip() {
        for action in [
            EraserAction::None,
            EraserAction::RightClick,
            EraserAction::LeftClick,
            EraserAction::MiddleClick,
            EraserAction::Touchpad,
            EraserAction::Touchscreen,
        ] {
            assert_eq!(action.to_string().parse::<EraserAction>().unwrap(), action);
        }
    }

    #[test]
    fn test_button_key() {
        assert_eq!(EraserAction::LeftClick.button_key(), Some(Key::BTN_LEFT));
        assert_eq!(EraserAction::RightClick.button_key(), Some(Key::BTN_RIGHT));
        assert_eq!(EraserAction::MiddleClick.button_key(), Some(Key::BTN_MIDDLE));
        assert_eq!(EraserAction::None.button_key(), None);
        assert_eq!(EraserAction::Touchpad.button_key(), None);
    }

    #[test]
    fn test_flags() {
        assert!(EraserAction::Touchpad.is_touchpad());
        assert!(!EraserAction::RightClick.is_touchpad());
        assert!(EraserAction::Touchpad.is_implemented());
        assert!(!EraserAction::Touchscreen.is_implemented());
    }
}
