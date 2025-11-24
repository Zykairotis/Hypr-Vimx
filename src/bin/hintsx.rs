use anyhow::{Result, anyhow};
use rust_hintsx::backends::build_backends;
use rust_hintsx::config::Config;
use rust_hintsx::generate_hints;
use rust_hintsx::ipc::ensure_daemon_running;
use rust_hintsx::ui::overlay::launch_overlay;
use rust_hintsx::window_system::WindowSystem;

fn main() -> Result<()> {
    env_logger::init();
    let start_total = std::time::Instant::now();

    let t0 = std::time::Instant::now();
    let cfg = Config::load();
    println!("[BENCH] Config load: {:?}", t0.elapsed());

    let t1 = std::time::Instant::now();
    let window_system = if std::env::var("HINTSX_FAST_MODE").is_ok() {
        // Fast mode: assume Wayland, skip detection
        rust_hintsx::window_system::WindowSystem {
            window_system_type: rust_hintsx::window_system::WindowSystemType::Wayland,
            window_system_name: "wayland".into(),
            bar_height: 0,
        }
    } else {
        WindowSystem::detect(&cfg.window_system)?
    };
    println!("[BENCH] Window detection: {:?}", t1.elapsed());

    let debug_overlay = std::env::var("HINTSX_DEBUG_OVERLAY")
        .map(|v| v.eq_ignore_ascii_case("1") || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let t2 = std::time::Instant::now();
    ensure_daemon_running()?;
    println!("[BENCH] Daemon check: {:?}", t2.elapsed());

    let mut children = Vec::new();
    let mut focus_extents = None;
    let mut backend_used = String::new();

    let t3 = std::time::Instant::now();
    for mut backend in build_backends(&cfg, &window_system) {
        let t_backend = std::time::Instant::now();
        match backend.get_children() {
            Ok(result) if !result.children.is_empty() => {
                println!(
                    "[BENCH] Backend {} success: {:?}",
                    backend.name(),
                    t_backend.elapsed()
                );
                children = result.children;
                focus_extents = result.focus_extents;
                backend_used = backend.name().into();
                break;
            }
            Ok(_) => {
                println!(
                    "[BENCH] Backend {} empty: {:?}",
                    backend.name(),
                    t_backend.elapsed()
                );
                log::warn!("backend {} returned zero children", backend.name());
            }
            Err(err) => {
                println!(
                    "[BENCH] Backend {} failed: {:?}",
                    backend.name(),
                    t_backend.elapsed()
                );
                log::warn!("backend {} failed: {err}", backend.name());
            }
        }
    }
    println!("[BENCH] Total backend search: {:?}", t3.elapsed());

    // If no extents came back but we still want window-scoped overlay, try xdotool geometry.
    if focus_extents.is_none() && cfg.overlay_target == rust_hintsx::config::OverlayTarget::Window {
        let t_fallback = std::time::Instant::now();
        if let Some(extents) = window_system.get_active_window_geometry_x11() {
            println!(
                "DEBUG: post-backend xdotool geometry fallback: {:?}",
                extents
            );
            focus_extents = Some(extents);
        } else {
            println!("DEBUG: no focus extents available; overlay will size to all hints");
        }
        println!("[BENCH] Fallback geometry: {:?}", t_fallback.elapsed());
    }

    if children.is_empty() {
        return Err(anyhow!(
            "no children gathered from any backend; check accessibility setup"
        ));
    }

    let t4 = std::time::Instant::now();
    let hints = generate_hints(&children, &cfg.alphabet);
    println!("[BENCH] Hint generation: {:?}", t4.elapsed());

    log::info!(
        "rendering {} hints via backend {}",
        hints.len(),
        backend_used
    );

    println!("[BENCH] Pre-launch total: {:?}", start_total.elapsed());
    launch_overlay(cfg, window_system, focus_extents, hints, debug_overlay);
    Ok(())
}
