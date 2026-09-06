//! Portable building blocks shared between the rm-pad host binary and
//! on-device consumers (e.g. a native reMarkable program).
//!
//! Everything here is pure coordinate/event math plus device constants: no
//! SSH, no uinput, no host-side assumptions. The host forwarding binary lives
//! behind the `host` feature (enabled by default) and layers transport and
//! output on top of these modules.

pub mod device;
pub mod display;
pub mod fit;
pub mod orientation;
pub mod palm;
pub mod pen_map;
pub mod tilt;
