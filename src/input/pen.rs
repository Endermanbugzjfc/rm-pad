use std::io::Read;
use std::time::Instant;

use evdevil::event::{Abs, InputEvent, Key};
use evdevil::uinput::{AbsSetup, UinputDevice};
use evdevil::{AbsInfo, Bus, InputId, InputProp};

use crate::config::Config;
use crate::device::DeviceProfile;
use crate::eraser::EraserAction;
use crate::orientation::Orientation;
use crate::palm::SharedPalmState;
use crate::ssh;

use super::event::{
    key_event, parse_input_event, ABS_MT_POSITION_X, ABS_MT_POSITION_Y, ABS_MT_SLOT,
    ABS_MT_TRACKING_ID, ABS_PRESSURE, BTN_TOOL_RUBBER, EV_ABS, EV_KEY, EV_SYN, SYN_REPORT,
};

const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;
const ABS_TILT_X: u16 = 0x1a;
const ABS_TILT_Y: u16 = 0x1b;

fn abs_event(code: u16, value: i32) -> InputEvent {
    InputEvent::new(evdevil::event::EventType::from_raw(EV_ABS), code, value)
}

fn syn_event() -> InputEvent {
    InputEvent::new(evdevil::event::EventType::from_raw(EV_SYN), SYN_REPORT, 0)
}

fn create_pen_device(device: &DeviceProfile, orientation: Orientation) -> Result<UinputDevice, Box<dyn std::error::Error + Send + Sync>> {
    let (out_x_max, out_y_max) = orientation.pen_output_dimensions(device.pen_x_max, device.pen_y_max);
    let axes = [
        AbsSetup::new(Abs::X, AbsInfo::new(0, out_x_max).with_resolution(100)),
        AbsSetup::new(Abs::Y, AbsInfo::new(0, out_y_max).with_resolution(100)),
        AbsSetup::new(Abs::PRESSURE, AbsInfo::new(0, device.pen_pressure_max)),
        AbsSetup::new(Abs::DISTANCE, AbsInfo::new(0, device.pen_distance_max)),
        AbsSetup::new(Abs::TILT_X, AbsInfo::new(-device.pen_tilt_range, device.pen_tilt_range)),
        AbsSetup::new(Abs::TILT_Y, AbsInfo::new(-device.pen_tilt_range, device.pen_tilt_range)),
    ];

    let device = UinputDevice::builder()?
        .with_input_id(InputId::new(Bus::from_raw(0x03), 0x2d1f, 0x0001, 0))?
        .with_props([InputProp::DIRECT])?
        .with_abs_axes(axes)?
        .with_keys([
            Key::BTN_TOOL_PEN,
            Key::BTN_TOOL_RUBBER,
            Key::BTN_TOUCH,
            Key::BTN_STYLUS,
            // Buttons the eraser can be mapped to (held while the eraser touches).
            Key::BTN_LEFT,
            Key::BTN_RIGHT,
            Key::BTN_MIDDLE,
        ])?
        .build("reMarkable Pen")?;

    Ok(device)
}

/// Touchpad-style pointer device used when the eraser action is `touchpad`.
///
/// Mirrors the finger touch device (`InputProp::POINTER | BUTTONPAD` + MT slots)
/// so libinput classifies it as a touchpad: eraser motion moves the cursor
/// relatively and a tap left-clicks. Sized to the pen's output space.
fn create_eraser_touchpad_device(
    device: &DeviceProfile,
    orientation: Orientation,
) -> Result<UinputDevice, Box<dyn std::error::Error + Send + Sync>> {
    let (out_x_max, out_y_max) = orientation.pen_output_dimensions(device.pen_x_max, device.pen_y_max);
    let resolution = 100;

    let axes = [
        AbsSetup::new(Abs::X, AbsInfo::new(0, out_x_max).with_resolution(resolution)),
        AbsSetup::new(Abs::Y, AbsInfo::new(0, out_y_max).with_resolution(resolution)),
        AbsSetup::new(Abs::MT_SLOT, AbsInfo::new(0, 1)),
        AbsSetup::new(Abs::MT_TRACKING_ID, AbsInfo::new(-1, i32::MAX)),
        AbsSetup::new(Abs::MT_POSITION_X, AbsInfo::new(0, out_x_max).with_resolution(resolution)),
        AbsSetup::new(Abs::MT_POSITION_Y, AbsInfo::new(0, out_y_max).with_resolution(resolution)),
    ];

    let uinput = UinputDevice::builder()?
        .with_props([InputProp::POINTER, InputProp::BUTTONPAD])?
        .with_abs_axes(axes)?
        .with_keys([Key::BTN_LEFT, Key::BTN_TOUCH, Key::BTN_TOOL_FINGER])?
        .build("reMarkable Eraser")?;

    Ok(uinput)
}

/// Emit one frame to the eraser touchpad device for the current contact state.
///
/// Returns without writing when nothing changed (avoids empty frames).
fn emit_eraser_pad_frame(
    pad: &UinputDevice,
    now_touching: bool,
    pos: Option<(i32, i32)>,
    pad_down: &mut bool,
    tracking_id: &mut i32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut body: Vec<InputEvent> = Vec::with_capacity(8);

    if now_touching {
        if !*pad_down {
            // New contact: only start it once we have a position to report.
            let Some((x, y)) = pos else { return Ok(()) };
            *tracking_id = tracking_id.wrapping_add(1) & 0x7fff_ffff;
            body.push(abs_event(ABS_MT_TRACKING_ID, *tracking_id));
            body.push(abs_event(ABS_MT_POSITION_X, x));
            body.push(abs_event(ABS_MT_POSITION_Y, y));
            body.push(abs_event(ABS_X, x));
            body.push(abs_event(ABS_Y, y));
            body.push(key_event(Key::BTN_TOUCH.raw(), 1));
            body.push(key_event(Key::BTN_TOOL_FINGER.raw(), 1));
            *pad_down = true;
        } else if let Some((x, y)) = pos {
            body.push(abs_event(ABS_MT_POSITION_X, x));
            body.push(abs_event(ABS_MT_POSITION_Y, y));
            body.push(abs_event(ABS_X, x));
            body.push(abs_event(ABS_Y, y));
        }
    } else if *pad_down {
        body.push(abs_event(ABS_MT_TRACKING_ID, -1));
        body.push(key_event(Key::BTN_TOUCH.raw(), 0));
        body.push(key_event(Key::BTN_TOOL_FINGER.raw(), 0));
        *pad_down = false;
    }

    if body.is_empty() {
        return Ok(());
    }

    let mut frame: Vec<InputEvent> = Vec::with_capacity(body.len() + 2);
    frame.push(abs_event(ABS_MT_SLOT, 0));
    frame.extend(body);
    frame.push(syn_event());
    pad.write(&frame)?;

    Ok(())
}

pub fn run_pen(
    config: &Config,
    device_profile: &DeviceProfile,
    palm: Option<SharedPalmState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (_cleanup, mut channel) =
        ssh::open_input_stream(&config.pen_device, config, config.grab_input)?;

    log::info!("Creating pen uinput device");
    let uinput = create_pen_device(device_profile, config.orientation)?;

    if let Ok(name) = uinput.sysname() {
        log::info!("Pen device ready: /sys/devices/virtual/input/{}", name.to_string_lossy());
    }

    std::thread::sleep(std::time::Duration::from_secs(1));
    log::info!("Pen forwarding started");

    let eraser_action = config.eraser_action;

    // The touchpad-style eraser device only exists when that action is selected.
    let eraser_pad = if eraser_action.is_touchpad() {
        log::info!("Creating eraser touchpad uinput device");
        Some(create_eraser_touchpad_device(device_profile, config.orientation)?)
    } else {
        None
    };

    let btn_touch_code = Key::BTN_TOUCH.raw();
    let mut buf = vec![0u8; device_profile.input_event_size];
    let mut batch: Vec<InputEvent> = Vec::with_capacity(32);
    let mut touch_down = false;
    let mut frame_count: u64 = 0;

    // Eraser state
    let mut eraser_active = false;
    let mut eraser_button_held: Option<u16> = None;
    let mut pad_down = false;
    let mut pad_tracking_id: i32 = 0;
    let mut warned_unimplemented = false;

    // For collecting X/Y/tilt values within a frame
    let mut pending_x: Option<i32> = None;
    let mut pending_y: Option<i32> = None;
    let mut pending_tilt_x: Option<i32> = None;
    let mut pending_tilt_y: Option<i32> = None;
    let orientation = config.orientation;

    loop {
        channel.read_exact(&mut buf)?;

        let Some(ev) = parse_input_event(&buf) else {
            continue;
        };

        let ty = ev.event_type().raw();
        let code = ev.raw_code();
        let value = ev.raw_value();

        // Track eraser tool engagement (pen flipped to the rubber end).
        if ty == EV_KEY && code == BTN_TOOL_RUBBER {
            eraser_active = value != 0;
        }

        // Collect position and tilt values, defer transformation until SYN_REPORT
        if ty == EV_ABS {
            match code {
                ABS_X => {
                    pending_x = Some(value);
                    continue;
                }
                ABS_Y => {
                    pending_y = Some(value);
                    continue;
                }
                ABS_TILT_X => {
                    pending_tilt_x = Some(value);
                    continue;
                }
                ABS_TILT_Y => {
                    pending_tilt_y = Some(value);
                    continue;
                }
                _ => {}
            }
        }

        batch.push(ev);

        if ty != EV_SYN || code != SYN_REPORT {
            continue;
        }

        // Transform position/tilt once; route them per the active tool below.
        let pos = match (pending_x.take(), pending_y.take()) {
            (Some(x), Some(y)) => Some(orientation.transform_pen(
                x, y,
                device_profile.pen_x_max,
                device_profile.pen_y_max,
            )),
            _ => None,
        };
        let tilt = match (pending_tilt_x.take(), pending_tilt_y.take()) {
            (Some(tx), Some(ty)) => Some(orientation.transform_tilt(tx, ty)),
            _ => None,
        };

        let pressure = batch
            .iter()
            .rfind(|e| e.event_type().raw() == EV_ABS && e.raw_code() == ABS_PRESSURE)
            .map(|e| e.raw_value())
            .unwrap_or(0);

        let now_touching = pressure > 0;
        update_palm_state(&palm, now_touching);

        // Resolve the eraser action, treating unimplemented ones as a no-op.
        let effective_action = if eraser_active && !eraser_action.is_implemented() {
            if !warned_unimplemented {
                log::warn!("Eraser action '{}' is not yet implemented; ignoring eraser", eraser_action);
                warned_unimplemented = true;
            }
            EraserAction::None
        } else {
            eraser_action
        };

        if eraser_active && effective_action.is_touchpad() {
            // Route the eraser to its own touchpad device. Make sure nothing is
            // left asserted on the pen device first.
            let mut release: Vec<InputEvent> = Vec::new();
            if touch_down {
                release.push(key_event(btn_touch_code, 0));
                touch_down = false;
            }
            if let Some(btn) = eraser_button_held.take() {
                release.push(key_event(btn, 0));
            }
            if !release.is_empty() {
                release.push(syn_event());
                uinput.write(&release)?;
            }

            if let Some(pad) = eraser_pad.as_ref() {
                emit_eraser_pad_frame(pad, now_touching, pos, &mut pad_down, &mut pad_tracking_id)?;
            }

            batch.clear();
            log_pen_frame(&mut frame_count);
            continue;
        }

        // Pen-device path (normal pen tip, or eraser mapped to a click / none).
        // If we just left touchpad mode with a contact still down, lift it.
        if pad_down {
            if let Some(pad) = eraser_pad.as_ref() {
                emit_eraser_pad_frame(pad, false, None, &mut pad_down, &mut pad_tracking_id)?;
            }
        }

        if let Some((out_x, out_y)) = pos {
            batch.insert(0, abs_event(Abs::X.raw(), out_x));
            batch.insert(1, abs_event(Abs::Y.raw(), out_y));
        }
        if let Some((out_tx, out_ty)) = tilt {
            batch.insert(0, abs_event(Abs::TILT_X.raw(), out_tx));
            batch.insert(1, abs_event(Abs::TILT_Y.raw(), out_ty));
        }

        // Decide the pen contact / eraser button for this frame.
        let (desired_touch, desired_button) = if eraser_active {
            let button = if now_touching {
                effective_action.button_key().map(|k| k.raw())
            } else {
                None
            };
            (false, button)
        } else {
            (now_touching, None)
        };

        if desired_button != eraser_button_held {
            if let Some(old) = eraser_button_held {
                batch.insert(0, key_event(old, 0));
            }
            if let Some(new) = desired_button {
                batch.insert(0, key_event(new, 1));
            }
            eraser_button_held = desired_button;
        }

        if desired_touch != touch_down {
            batch.insert(0, key_event(btn_touch_code, if desired_touch { 1 } else { 0 }));
            touch_down = desired_touch;
        }

        uinput.write(&batch)?;
        batch.clear();
        log_pen_frame(&mut frame_count);
    }
}

fn log_pen_frame(frame_count: &mut u64) {
    if *frame_count == 0 {
        log::info!("Pen events flowing");
    }
    *frame_count += 1;

    if frame_count.is_multiple_of(500) {
        log::debug!("Pen frames forwarded: {}", frame_count);
    }
}

fn update_palm_state(palm: &Option<SharedPalmState>, now_touching: bool) {
    let Some(palm_state) = palm else { return };
    let Ok(mut state) = palm_state.lock() else { return };

    state.pen_down = now_touching;
    if !now_touching {
        state.last_pen_up = Some(Instant::now());
    }
}
