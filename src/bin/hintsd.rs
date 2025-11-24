use anyhow::Result;
use gdk4;
use gtk4;
use gtk4::prelude::{Cast, DisplayExt, ListModelExt, MonitorExt};
use rust_hintsx::consts::UNIX_DOMAIN_SOCKET_FILE;
use rust_hintsx::ipc::{Request, Response};
use rust_hintsx::mouse::{MouseButton, MouseButtonState, VirtualMouse};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::UnixListener;

fn main() -> Result<()> {
    env_logger::init();

    if std::path::Path::new(UNIX_DOMAIN_SOCKET_FILE).exists() {
        fs::remove_file(UNIX_DOMAIN_SOCKET_FILE)?;
    }

    gtk4::init().ok();
    let display = gdk4::Display::default().expect("no display");
    let monitor_list = display.monitors();
    let monitor = monitor_list
        .item(0)
        .and_then(|o| o.downcast::<gdk4::Monitor>().ok())
        .expect("no monitor 0");
    let geo = monitor.geometry();
    let screen_width = geo.width();
    let screen_height = geo.height();
    let scale_factor = monitor.scale_factor();

    let mut mouse = VirtualMouse::new(screen_width, screen_height, scale_factor)?;
    let listener = UnixListener::bind(UNIX_DOMAIN_SOCKET_FILE)?;
    log::info!("hintsd listening on {}", UNIX_DOMAIN_SOCKET_FILE);

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                if let Err(err) = handle_connection(&mut stream, &mut mouse) {
                    log::warn!("connection error: {err}");
                }
            }
            Err(err) => {
                log::warn!("listener error: {err}");
            }
        }
    }
    Ok(())
}

fn handle_connection(
    stream: &mut std::os::unix::net::UnixStream,
    mouse: &mut VirtualMouse,
) -> Result<()> {
    log::info!("════════════════════════════════════════════════════════════════");
    log::info!("DAEMON: New connection received on socket");

    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes)?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    log::info!("DAEMON: Request length: {} bytes", len);

    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)?;
    log::info!("DAEMON: Request data received");

    let req: Request = bincode::deserialize(&buf)?;
    log::info!("DAEMON: Request deserialized successfully");
    log::info!(
        "DAEMON: Request type: {:?}",
        match &req {
            Request::Move { .. } => "Move",
            Request::Scroll { .. } => "Scroll",
            Request::Click { .. } => "Click",
            Request::DoMouseAction { .. } => "DoMouseAction",
        }
    );

    let result = match req {
        Request::Move { x, y, absolute } => {
            log::info!("DAEMON: Processing Move request");
            log::info!("  x={}, y={}, absolute={}", x, y, absolute);
            mouse.r#move(x, y, absolute)
        }
        Request::Scroll { x, y } => {
            log::info!("DAEMON: Processing Scroll request");
            log::info!("  x={}, y={}", x, y);
            mouse.scroll(x, y)
        }
        Request::Click {
            x,
            y,
            button,
            button_states,
            repeat,
            absolute,
        } => {
            log::info!("DAEMON: Processing Click request");
            log::info!(
                "  x={}, y={}, button={}, button_states={:?}, repeat={}, absolute={}",
                x,
                y,
                button,
                button_states,
                repeat,
                absolute
            );

            // Wait for overlay to fully close and release input grab
            // GTK/layer-shell windows take time to release, especially on Wayland
            log::info!("DAEMON: Waiting 500ms for overlay to close and focus to settle...");
            std::thread::sleep(std::time::Duration::from_millis(500));
            log::info!("DAEMON: Wait complete, proceeding with click");

            let btn = match button {
                2 => MouseButton::Right,
                1 => MouseButton::Middle,
                _ => MouseButton::Left,
            };
            log::info!("DAEMON: Button mapped: {} -> {:?}", button, btn);

            let states: Vec<MouseButtonState> = button_states
                .into_iter()
                .map(|s| {
                    if s == 0 {
                        MouseButtonState::Up
                    } else {
                        MouseButtonState::Down
                    }
                })
                .collect();
            log::info!("DAEMON: Button states converted: {:?}", states);

            mouse.click(x, y, btn, &states, repeat, absolute)
        }
        Request::DoMouseAction { key, mode } => {
            log::info!("DAEMON: Processing DoMouseAction request (not implemented)");
            log::info!("  key={}, mode={:?}", key, mode);
            Ok(())
        }
    };

    log::info!("DAEMON: Request processing completed");
    let resp = match result {
        Ok(_) => {
            log::info!("DAEMON: Request successful, sending OK response");
            Response::Ok
        }
        Err(err) => {
            log::error!("DAEMON: Request failed with error: {}", err);
            Response::Error(format!("{err}"))
        }
    };

    let payload = bincode::serialize(&resp)?;
    log::info!("DAEMON: Response serialized, {} bytes", payload.len());

    stream.write_all(&(payload.len() as u32).to_le_bytes())?;
    stream.write_all(&payload)?;
    stream.flush()?;
    log::info!("DAEMON: Response sent successfully");
    log::info!("════════════════════════════════════════════════════════════════");
    Ok(())
}
