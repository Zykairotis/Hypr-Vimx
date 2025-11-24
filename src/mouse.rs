use anyhow::Result;
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode,
    UinputAbsSetup, uinput::VirtualDevice,
};
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy)]
pub enum MouseButtonState {
    Down,
    Up,
}

#[derive(Debug)]
pub struct VirtualMouse {
    abs_device: VirtualDevice,
    rel_device: VirtualDevice,
    write_pause: Duration,
    scale_factor: i32,
}

impl VirtualMouse {
    pub fn new(screen_width: i32, screen_height: i32, scale_factor: i32) -> Result<Self> {
        log::info!("Creating virtual mouse device...");
        log::info!(
            "Screen dimensions: {}x{}, Scale factor: {}",
            screen_width,
            screen_height,
            scale_factor
        );

        // Buttons for relative device (standard mouse)
        let mut rel_keys = AttributeSet::<KeyCode>::new();
        rel_keys.insert(KeyCode::BTN_LEFT);
        rel_keys.insert(KeyCode::BTN_RIGHT);
        rel_keys.insert(KeyCode::BTN_MIDDLE);

        // Buttons for absolute device (touch/tablet-like)
        let mut abs_keys = rel_keys.clone();
        abs_keys.insert(KeyCode::BTN_TOUCH);

        // Relative axes for scrolling and relative motion
        let mut rel_axes = AttributeSet::<RelativeAxisCode>::new();
        rel_axes.insert(RelativeAxisCode::REL_X);
        rel_axes.insert(RelativeAxisCode::REL_Y);
        rel_axes.insert(RelativeAxisCode::REL_WHEEL);
        rel_axes.insert(RelativeAxisCode::REL_HWHEEL);

        log::info!("Building relative mouse device...");
        let rel_device = VirtualDevice::builder()
            .map_err(|e| {
                log::error!("Failed to create relative device builder: {}", e);
                anyhow::anyhow!("Relative device builder failed: {}", e)
            })?
            .name("hintsx-mouse-rel")
            .with_keys(&rel_keys)
            .map_err(|e| {
                log::error!("Failed to add keys to relative device: {}", e);
                anyhow::anyhow!("Failed to add keys to relative device: {}", e)
            })?
            .with_relative_axes(&rel_axes)
            .map_err(|e| {
                log::error!("Failed to add relative axes: {}", e);
                anyhow::anyhow!("Failed to add relative axes: {}", e)
            })?
            .build()
            .map_err(|e| {
                log::error!("Failed to build relative device: {}. Make sure you're in the 'input' group or run as root.", e);
                anyhow::anyhow!("Failed to build relative device: {}", e)
            })?;

        log::info!("Building absolute mouse device...");
        let abs_device = VirtualDevice::builder()
            .map_err(|e| {
                log::error!("Failed to create absolute device builder: {}", e);
                e
            })?
            .name("hintsx-mouse-abs")
            .with_keys(&abs_keys) // include buttons for completeness
            .map_err(|e| {
                log::error!("Failed to add keys to absolute device: {}", e);
                e
            })?
            .with_absolute_axis(&UinputAbsSetup::new(
                AbsoluteAxisCode::ABS_X,
                AbsInfo::new(0, 0, screen_width * scale_factor, 0, 0, 0),
            ))?
            .with_absolute_axis(&UinputAbsSetup::new(
                AbsoluteAxisCode::ABS_Y,
                AbsInfo::new(0, 0, screen_height * scale_factor, 0, 0, 0),
            ))?
            .build()
            .map_err(|e| {
                log::error!("Failed to build absolute device: {}. Make sure you're in the 'input' group or run as root.", e);
                e
            })?;

        log::info!("Virtual mouse devices created successfully");
        Ok(Self {
            abs_device,
            rel_device,
            write_pause: Duration::from_millis(30), // Match Python service timing
            scale_factor,
        })
    }

    pub fn scroll(&mut self, x: i32, y: i32) -> Result<()> {
        self.rel_device.emit(&[
            InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_HWHEEL.0, x),
            InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_WHEEL.0, y),
            InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0),
        ])?;
        Ok(())
    }

    pub fn r#move(&mut self, x: i32, y: i32, absolute: bool) -> Result<()> {
        log::info!("========== MOVE START ==========");
        log::info!("Input coordinates: x={}, y={}, absolute={}", x, y, absolute);
        log::info!("Scale factor: {}", self.scale_factor);

        let x_scaled = x * self.scale_factor;
        let y_scaled = y * self.scale_factor;
        log::info!("Scaled coordinates: x={}, y={}", x_scaled, y_scaled);

        if absolute {
            log::info!("Using ABSOLUTE positioning mode");

            // Try ydotool first (best for Wayland)
            // ydotool uses a 32768x32768 coordinate system (0-32767)
            // We need to convert from screen pixels to ydotool coordinates
            // But we don't know screen resolution here, so let's try hyprctl first

            // Use hyprctl for movement (it uses screen coordinates directly)
            log::info!(
                "Attempting hyprctl dispatch movecursor {} {}",
                x_scaled,
                y_scaled
            );
            let output = Command::new("hyprctl")
                .args(&[
                    "dispatch",
                    "movecursor",
                    &x_scaled.to_string(),
                    &y_scaled.to_string(),
                ])
                .output();

            match output {
                Ok(result) if result.status.success() => {
                    log::info!("✓ hyprctl command executed successfully");
                    log::info!("  stdout: {}", String::from_utf8_lossy(&result.stdout));
                    log::info!("  stderr: {}", String::from_utf8_lossy(&result.stderr));
                    log::info!("Sleeping 50ms for hyprctl to process...");
                    sleep(Duration::from_millis(50));
                    log::info!("Sleep complete");
                }
                Ok(result) => {
                    log::warn!("✗ hyprctl returned error code: {:?}", result.status.code());
                    log::warn!("  stdout: {}", String::from_utf8_lossy(&result.stdout));
                    log::warn!("  stderr: {}", String::from_utf8_lossy(&result.stderr));
                    log::info!("Falling back to uinput...");

                    self.abs_device.emit(&[
                        InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_X.0, x_scaled),
                        InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_Y.0, y_scaled),
                        InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0),
                    ])?;
                    sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    log::warn!("✗ Failed to execute hyprctl: {}", e);
                    log::info!("Falling back to uinput...");

                    self.abs_device.emit(&[
                        InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_X.0, x_scaled),
                        InputEvent::new(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_Y.0, y_scaled),
                        InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0),
                    ])?;
                    sleep(Duration::from_millis(50));
                }
            }
        } else {
            log::info!("Using RELATIVE positioning mode");
            log::info!(
                "Emitting REL_X={}, REL_Y={} via rel_device",
                x_scaled,
                y_scaled
            );
            self.rel_device.emit(&[
                InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_X.0, x_scaled),
                InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_Y.0, y_scaled),
                InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0),
            ])?;
            log::info!("Relative move events emitted, sleeping 30ms...");
            sleep(Duration::from_millis(30));
            log::info!("Sleep complete");
        }
        log::info!("========== MOVE COMPLETE ==========");
        Ok(())
    }

    pub fn click(
        &mut self,
        x: i32,
        y: i32,
        button: MouseButton,
        button_states: &[MouseButtonState],
        repeat: u32,
        absolute: bool,
    ) -> Result<()> {
        log::info!("╔════════════════════════════════════════════════════════════════════╗");
        log::info!("║                      CLICK OPERATION START                         ║");
        log::info!("╚════════════════════════════════════════════════════════════════════╝");
        log::info!("Click parameters:");
        log::info!("  Target position: ({}, {})", x, y);
        log::info!("  Button: {:?}", button);
        log::info!("  Button states: {:?}", button_states);
        log::info!("  Repeat count: {}", repeat);
        log::info!("  Absolute positioning: {}", absolute);

        // FIRST: Move mouse to target position
        log::info!("");
        log::info!("STEP 1: Moving mouse to target position...");
        self.r#move(x, y, absolute)?;
        log::info!("STEP 1: Move completed successfully");

        // Add extra delay to ensure move is fully processed before clicking
        log::info!("");
        log::info!("STEP 2: Waiting 100ms for move to settle...");
        sleep(Duration::from_millis(100));
        log::info!("STEP 2: Wait complete");

        let btn_code = match button {
            MouseButton::Left => KeyCode::BTN_LEFT,
            MouseButton::Right => KeyCode::BTN_RIGHT,
            MouseButton::Middle => KeyCode::BTN_MIDDLE,
        };
        log::info!("Button mapped to keycode: {:?}", btn_code);

        // Try ydotool for clicking (with proper socket path)
        log::info!("");
        log::info!("STEP 3: Attempting click via ydotool...");

        let ydotool_button = match button {
            MouseButton::Left => "0xC0",   // 0xC0 = left button click (down + up)
            MouseButton::Right => "0xC1",  // 0xC1 = right button click
            MouseButton::Middle => "0xC2", // 0xC2 = middle button click
        };

        log::info!("  Command: ydotool click {}", ydotool_button);
        log::info!("  Repeat count: {}", repeat);

        // Determine the correct ydotool socket path
        // Try to get from environment, or construct from UID
        let ydotool_socket = std::env::var("YDOTOOL_SOCKET").unwrap_or_else(|_| {
            // Get UID from /proc/self/loginuid or default to 1000
            let uid = std::fs::read_to_string("/proc/self/loginuid")
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
                .unwrap_or(1000);
            format!("/run/user/{}/.ydotool_socket", uid)
        });
        log::info!("  Using YDOTOOL_SOCKET: {}", ydotool_socket);

        let mut ydotool_worked = false;
        for iteration in 0..repeat {
            log::info!("  Attempt {}/{}", iteration + 1, repeat);
            let ydotool_cmd = format!("ydotool click -D 25 {}", ydotool_button);
            log::info!(
                "  Shell command: YDOTOOL_SOCKET={} {}",
                ydotool_socket,
                ydotool_cmd
            );

            let output = Command::new("sh")
                .args(&[
                    "-c",
                    &format!("YDOTOOL_SOCKET={} {}", ydotool_socket, ydotool_cmd),
                ])
                .output();

            match output {
                Ok(result) if result.status.success() => {
                    log::info!("  ✓ ydotool click successful!");
                    log::info!("    stdout: {}", String::from_utf8_lossy(&result.stdout));
                    log::info!("    stderr: {}", String::from_utf8_lossy(&result.stderr));
                    ydotool_worked = true;
                    log::info!("  Sleeping 100ms for ydotool click to process...");
                    sleep(Duration::from_millis(100));
                    log::info!("  Sleep complete");
                    if iteration < repeat - 1 {
                        log::info!("  Sleeping 50ms between repeat clicks...");
                        sleep(Duration::from_millis(50));
                    }
                }
                Ok(result) => {
                    log::warn!(
                        "  ✗ ydotool failed with exit code {:?}",
                        result.status.code()
                    );
                    log::warn!("    stdout: {}", String::from_utf8_lossy(&result.stdout));
                    log::warn!("    stderr: {}", String::from_utf8_lossy(&result.stderr));
                    log::info!("  Breaking ydotool attempts, will fall back to uinput");
                    break;
                }
                Err(e) => {
                    log::warn!("  ✗ Failed to execute ydotool command: {}", e);
                    log::info!("  Breaking ydotool attempts, will fall back to uinput");
                    break;
                }
            }
        }

        // Fallback to uinput if ydotool didn't work
        if !ydotool_worked {
            log::info!("");
            log::info!("STEP 3 (fallback): Using uinput for click events");
            log::info!("  Button states to send: {:?}", button_states);
            log::info!("  Repeat count: {}", repeat);

            for iteration in 0..repeat {
                log::info!("  Repeat iteration {}/{}", iteration + 1, repeat);
                for (state_idx, state) in button_states.iter().enumerate() {
                    let value = match state {
                        MouseButtonState::Down => 1,
                        MouseButtonState::Up => 0,
                    };

                    let button_name = if matches!(button, MouseButton::Left) {
                        "LEFT"
                    } else if matches!(button, MouseButton::Right) {
                        "RIGHT"
                    } else {
                        "MIDDLE"
                    };
                    let state_name = if value == 1 { "DOWN" } else { "UP" };

                    log::info!(
                        "    State {}/{}: Sending {} {}",
                        state_idx + 1,
                        button_states.len(),
                        button_name,
                        state_name
                    );
                    log::info!("      Emitting: KeyCode={:?}, value={}", btn_code, value);

                    self.rel_device.emit(&[
                        InputEvent::new(EventType::KEY.0, btn_code.0, value),
                        InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0),
                    ])?;
                    log::info!("      Event emitted successfully");

                    log::info!("      Sleeping 50ms...");
                    sleep(Duration::from_millis(50));
                    log::info!("      Sleep complete");
                }
            }
            log::info!("  All uinput button events completed");
        }

        log::info!("");
        log::info!("╔════════════════════════════════════════════════════════════════════╗");
        log::info!("║                    CLICK OPERATION COMPLETE                        ║");
        log::info!("╚════════════════════════════════════════════════════════════════════╝");

        // Add extra delay to ensure click is fully processed before returning
        log::info!("Final safety delay: waiting 200ms for click to fully register...");
        sleep(Duration::from_millis(200));
        log::info!("All done!");

        Ok(())
    }
}
