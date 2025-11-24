use crate::config::Config;
use crate::hints::HintMap;
use crate::ipc::{Request, send};
use crate::window_system::{WindowSystem, WindowSystemType};
use gtk4::gio::ListModel;
use gtk4::gio::prelude::ApplicationExtManual;
use gtk4::glib::{ControlFlow, Propagation, translate::IntoGlib};
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, CssProvider, EventControllerKey,
    STYLE_PROVIDER_PRIORITY_APPLICATION, StyleContext, gdk,
};
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(feature = "layer-shell")]
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

pub fn launch_overlay(
    config: Config,
    ws: WindowSystem,
    focus_extents: Option<(i32, i32, i32, i32)>,
    hints: HintMap,
    debug_overlay: bool,
) {
    let app = Application::builder().application_id("xyz.hintsx").build();

    let hints_rc = Rc::new(hints);
    let ws_clone = ws.clone();
    app.connect_activate(move |app| {
        build_ui(
            app,
            &config,
            &ws_clone,
            focus_extents,
            hints_rc.clone(),
            debug_overlay,
        );
    });

    app.run();
}

fn build_ui(
    app: &Application,
    cfg: &Config,
    ws: &WindowSystem,
    focus_extents: Option<(i32, i32, i32, i32)>,
    hints: Rc<HintMap>,
    debug_overlay: bool,
) {
    // Ensure the window itself is transparent and not painted by the theme.
    let provider = CssProvider::new();
    let _ = provider.load_from_data(
        "
        /* Force our overlay window and everything inside it to be transparent. */
        window.overlay-window,
        window.overlay-window * {
            background-color: transparent;
            background: transparent;
            box-shadow: none;
            border: none;
        }
        drawingarea.overlay-area {
            background-color: transparent;
            background: transparent;
            box-shadow: none;
            border: none;
        }
        /* Ensure no theme overrides with proper GTK4 CSS */
        window {
            background: transparent;
        }
        ",
    );
    if let Some(display) = gdk::Display::default() {
        StyleContext::add_provider_for_display(
            &display,
            &provider,
            STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        if debug_overlay {
            println!(
                "DEBUG: CSS provider applied at priority {} on display {:?}",
                STYLE_PROVIDER_PRIORITY_APPLICATION, display
            );
        }
    }

    let use_focus_anchor = focus_extents.is_some();
    let (origin_x, origin_y, width, height) = if use_focus_anchor {
        let (fx, fy, fw, fh) = focus_extents.unwrap();
        (fx, fy, fw, fh)
    } else {
        let (max_x, max_y) = hints.values().fold((0i32, 0i32), |acc, child| {
            (
                acc.0.max(child.absolute_x + child.width),
                acc.1.max(child.absolute_y + child.height),
            )
        });
        (0, 0, max_x, max_y)
    };

    let window = ApplicationWindow::builder()
        .application(app)
        .title("HintsX")
        .decorated(false)
        .resizable(false)
        .build();

    // Force RGBA visual for transparency
    if let Some(display) = gdk::Display::default() {
        if display.is_rgba() {
            if debug_overlay {
                println!("DEBUG: Display supports RGBA");
            }
        }
    }

    window.set_can_focus(true);
    // Remove the default "background" class if configured
    if cfg.overlay.remove_background_class {
        window.remove_css_class("background");
    }
    window.add_css_class("overlay-window");

    if debug_overlay {
        let classes: Vec<_> = window.css_classes().into_iter().collect();
        println!("DEBUG: window css classes = {:?}", classes);
    }

    #[cfg(feature = "layer-shell")]
    if ws.window_system_type == WindowSystemType::Wayland && cfg.overlay.use_layer_shell {
        window.init_layer_shell();
        window.set_namespace(Some(&cfg.overlay.layer_shell_namespace));
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::Exclusive);
        window.set_anchor(Edge::Top, true);
        window.set_anchor(Edge::Left, true);
        // Set exclusive zone from config (-1 for transparency)
        window.set_exclusive_zone(cfg.overlay.layer_shell_exclusive_zone);

        if use_focus_anchor {
            if let Some((monitor, geo)) = monitor_for_point(origin_x, origin_y) {
                window.set_monitor(Some(&monitor));
                let margin_top = origin_y - geo.y() + cfg.overlay_y_offset;
                let margin_left = origin_x - geo.x() + cfg.overlay_x_offset;
                window.set_margin(Edge::Top, margin_top);
                window.set_margin(Edge::Left, margin_left);
                if debug_overlay {
                    println!(
                        "DEBUG: Wayland layer-shell monitor {:?}, margins ({}, {})",
                        geo, margin_left, margin_top
                    );
                }
            }
        }
        // Don't auto-enable exclusive zone as it can interfere with transparency
        // window.auto_exclusive_zone_enable();
        if debug_overlay {
            println!("DEBUG: layer-shell enabled on Wayland");
        }
    }

    let window_width = width.max(100);
    let window_height = height.max(100);
    window.set_default_size(window_width, window_height);

    // Use DrawingArea for transparent rendering
    use gtk4::DrawingArea;
    let drawing_area = DrawingArea::new();
    drawing_area.add_css_class("overlay-area");
    drawing_area.set_hexpand(true);
    drawing_area.set_vexpand(true);

    // Set drawing area to have transparent background
    drawing_area.set_opacity(1.0); // Ensure widget itself is visible
    window.set_child(Some(&drawing_area));

    // Set window background to transparent
    window.set_opacity(1.0); // Keep window visible but let background be transparent

    // Ensure the surface itself is non-opaque so alpha is respected.
    window.connect_realize(move |w| {
        if let Some(surface) = w.surface() {
            surface.set_opaque_region(None);
            if debug_overlay {
                println!(
                    "DEBUG: set_opaque_region(None) applied to surface {:?}",
                    surface
                );
            }
        }
    });

    // Clone data for drawing callback
    let hints_for_draw = hints.clone();
    let cfg_for_draw = cfg.clone();
    let offset_x = if use_focus_anchor { origin_x } else { 0 };
    let offset_y = if use_focus_anchor { origin_y } else { 0 };

    drawing_area.set_draw_func(move |_area, cr, w, h| {
        // Clear entire surface to transparent if configured
        if cfg_for_draw.overlay.clear_background {
            cr.set_source_rgba(
                cfg_for_draw.overlay.background_color.0,
                cfg_for_draw.overlay.background_color.1,
                cfg_for_draw.overlay.background_color.2,
                cfg_for_draw.overlay.background_color.3,
            );
            cr.set_operator(gtk4::cairo::Operator::Clear);
            cr.paint().ok();
        }

        // Now switch to normal compositing
        cr.set_operator(gtk4::cairo::Operator::Over);

        if cfg_for_draw.overlay.debug_overlay_enabled || debug_overlay {
            // Draw a debug overlay with configured color
            cr.set_source_rgba(
                cfg_for_draw.overlay.debug_overlay_color.0,
                cfg_for_draw.overlay.debug_overlay_color.1,
                cfg_for_draw.overlay.debug_overlay_color.2,
                cfg_for_draw.overlay.debug_overlay_color.3,
            );
            cr.rectangle(0.0, 0.0, w as f64, h as f64);
            let _ = cr.fill();
        }

        // Draw hints
        for (label_text, child) in hints_for_draw.iter() {
            let center_x =
                child.absolute_x - offset_x + cfg_for_draw.overlay_x_offset + child.width / 2
                    - cfg_for_draw.hints.hint_width_padding;
            let center_y =
                child.absolute_y - offset_y + cfg_for_draw.overlay_y_offset + child.height / 2
                    - cfg_for_draw.hints.hint_height / 2;

            let text = if cfg_for_draw.hints.hint_uppercase {
                label_text.to_uppercase()
            } else {
                label_text.to_string()
            };

            // Set font
            cr.select_font_face(
                &cfg_for_draw.hints.hint_font_face,
                gtk4::cairo::FontSlant::Normal,
                gtk4::cairo::FontWeight::Bold,
            );
            cr.set_font_size(cfg_for_draw.hints.hint_font_size as f64);

            let extents = cr.text_extents(&text).unwrap();
            let hint_width = extents.width() + (cfg_for_draw.hints.hint_width_padding * 2) as f64;
            let hint_height = cfg_for_draw.hints.hint_height as f64;

            // Draw background
            cr.set_source_rgba(
                cfg_for_draw.hints.hint_background_color.0,
                cfg_for_draw.hints.hint_background_color.1,
                cfg_for_draw.hints.hint_background_color.2,
                cfg_for_draw.hints.hint_background_color.3,
            );
            let _ = cr.rectangle(center_x as f64, center_y as f64, hint_width, hint_height);
            let _ = cr.fill();

            // Draw text
            cr.set_source_rgba(
                cfg_for_draw.hints.hint_font_color.0,
                cfg_for_draw.hints.hint_font_color.1,
                cfg_for_draw.hints.hint_font_color.2,
                cfg_for_draw.hints.hint_font_color.3,
            );
            let text_x = center_x as f64 + cfg_for_draw.hints.hint_width_padding as f64;
            let text_y = center_y as f64 + hint_height / 2.0 + extents.height() / 2.0;
            cr.move_to(text_x, text_y);
            let _ = cr.show_text(&text);
        }
    });

    let input = Rc::new(RefCell::new(String::new()));
    let repeat_count = Rc::new(RefCell::new(0u32));
    let hints_for_key = hints.clone();
    let cfg_mouse = cfg.mouse.clone();
    let key_controller = EventControllerKey::new();
    let window_weak = window.downgrade();
    let app_handle = app.clone();

    key_controller.connect_key_pressed(move |_ctrl, keyval, _keycode, state| {
        let keyval_raw = keyval.into_glib();

        // Check for exit key
        if keyval_raw == cfg_mouse.exit_key {
            if let Some(w) = window_weak.upgrade() {
                w.close();
            }
            return Propagation::Stop;
        }

        // Handle vim movement keys for scrolling/moving
        if let Some(ch) = keyval.to_unicode() {
            let ch_lower = ch.to_ascii_lowercase();
            let is_uppercase = ch.is_ascii_uppercase();

            // Check for movement/scroll keys
            if ch_lower == 'h' || ch_lower == 'j' || ch_lower == 'k' || ch_lower == 'l' {
                let (dx, dy) = match ch_lower {
                    'h' => (-cfg_mouse.move_pixel_sensitivity, 0),
                    'l' => (cfg_mouse.move_pixel_sensitivity, 0),
                    'k' => (0, -cfg_mouse.move_pixel_sensitivity),
                    'j' => (0, cfg_mouse.move_pixel_sensitivity),
                    _ => (0, 0),
                };

                // Check if scrolling (could be configurable)
                if state.contains(gdk::ModifierType::SHIFT_MASK) {
                    let _ = send(Request::Scroll {
                        x: dx * cfg_mouse.scroll_pixel_sensitivity
                            / cfg_mouse.move_pixel_sensitivity,
                        y: dy * cfg_mouse.scroll_pixel_sensitivity
                            / cfg_mouse.move_pixel_sensitivity,
                    });
                } else {
                    let _ = send(Request::Move {
                        x: dx,
                        y: dy,
                        absolute: false,
                    });
                }
                return Propagation::Stop;
            }

            // Check for numeric prefix (repeat count)
            if ch_lower.is_ascii_digit() {
                let digit = ch_lower.to_digit(10).unwrap_or(0);
                let current_repeat = *repeat_count.borrow();
                *repeat_count.borrow_mut() = current_repeat * 10 + digit;
                return Propagation::Stop;
            }

            // Regular hint character
            input.borrow_mut().push(ch_lower);
            let current = input.borrow().clone();

            // If no hint starts with the current buffer, reset
            if !hints_for_key.keys().any(|h| h.starts_with(&current)) {
                input.borrow_mut().clear();
                repeat_count.borrow_mut().clone_from(&0);
                return Propagation::Stop;
            }

            // Check if we have a complete hint
            if let Some(child) = hints_for_key.get(&current) {
                log::info!("╔══════════════════════════════════════════════════════════════╗");
                log::info!("║              OVERLAY: Hint Match Found!                      ║");
                log::info!("╚══════════════════════════════════════════════════════════════╝");
                log::info!("OVERLAY: Matched hint label: '{}'", current);
                log::info!("OVERLAY: Child element details:");
                log::info!("  absolute_x: {}", child.absolute_x);
                log::info!("  absolute_y: {}", child.absolute_y);
                log::info!("  width: {}", child.width);
                log::info!("  height: {}", child.height);

                let click_x = child.absolute_x + child.width / 2;
                let click_y = child.absolute_y + child.height / 2;
                log::info!(
                    "OVERLAY: Calculated click position (center): ({}, {})",
                    click_x,
                    click_y
                );

                // Determine action based on modifiers
                let mut button = 0u16; // Left click
                let mut action_type = "click";

                log::info!("OVERLAY: Checking modifiers...");
                log::info!("  is_uppercase: {}", is_uppercase);
                log::info!(
                    "  SHIFT_MASK: {}",
                    state.contains(gdk::ModifierType::SHIFT_MASK)
                );
                log::info!(
                    "  ALT_MASK: {}",
                    state.contains(gdk::ModifierType::ALT_MASK)
                );
                log::info!(
                    "  CONTROL_MASK: {}",
                    state.contains(gdk::ModifierType::CONTROL_MASK)
                );

                // Check modifiers
                // Right click: uppercase letter (Shift was pressed) OR explicit Shift modifier
                if is_uppercase || state.contains(gdk::ModifierType::SHIFT_MASK) {
                    // Right click
                    button = 2;
                    log::info!("OVERLAY: Action determined: RIGHT CLICK (button=2)");
                } else if state.contains(gdk::ModifierType::ALT_MASK) {
                    // Drag/grab - send mouse down, move, then up
                    action_type = "drag";
                    log::info!("OVERLAY: Action determined: DRAG");
                } else if state.contains(gdk::ModifierType::CONTROL_MASK) {
                    // Hover - just move the mouse there
                    log::info!("OVERLAY: Action determined: HOVER (move only)");
                    log::info!("OVERLAY: Closing overlay window FIRST");
                    if let Some(w) = window_weak.upgrade() {
                        w.hide();
                    }

                    // Keep the application alive while the overlay unmaps, then fire the move.
                    let app_ref = app_handle.clone();
                    let mut app_guard = Some(app_ref.hold());
                    let (tx, ty) = (click_x, click_y);
                    gtk4::glib::timeout_add_local(
                        std::time::Duration::from_millis(100),
                        move || {
                            log::info!("OVERLAY: Sending Move request to ({}, {})", tx, ty);
                            let result = send(Request::Move {
                                x: tx,
                                y: ty,
                                absolute: true,
                            });
                            log::info!("OVERLAY: Move request result: {:?}", result);
                            if let Some(guard) = app_guard.take() {
                                drop(guard);
                            }
                            app_ref.quit();
                            ControlFlow::Break
                        },
                    );
                    return Propagation::Stop;
                } else {
                    log::info!("OVERLAY: Action determined: LEFT CLICK (button=0)");
                }

                // Get repeat count (default to 1 if not set)
                let repeat = if *repeat_count.borrow() > 0 {
                    *repeat_count.borrow()
                } else {
                    1
                };
                log::info!("OVERLAY: Repeat count: {}", repeat);

                // Close overlay FIRST, then send requests after the window fully unmaps.
                log::info!("OVERLAY: Closing overlay window FIRST");
                if let Some(w) = window_weak.upgrade() {
                    w.hide();
                }

                let app_ref = app_handle.clone();
                let mut app_guard = Some(app_ref.hold());
                let is_drag = action_type == "drag";
                let (tx, ty, btn, rep) = (click_x, click_y, button, repeat);
                gtk4::glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                    if is_drag {
                        log::info!("OVERLAY: Executing DRAG sequence asynchronously:");
                        log::info!("  1. Mouse down at current position");
                        log::info!("  2. Move to ({}, {})", tx, ty);
                        log::info!("  3. Mouse up at target");

                        let result1 = send(Request::Click {
                            x: 0,
                            y: 0,
                            button: 0,
                            button_states: vec![1], // Mouse down
                            repeat: 1,
                            absolute: false,
                        });
                        log::info!("OVERLAY: Mouse DOWN result: {:?}", result1);

                        let result2 = send(Request::Move {
                            x: tx,
                            y: ty,
                            absolute: true,
                        });
                        log::info!("OVERLAY: MOVE result: {:?}", result2);

                        let result3 = send(Request::Click {
                            x: tx,
                            y: ty,
                            button: 0,
                            button_states: vec![0], // Mouse up
                            repeat: 1,
                            absolute: true,
                        });
                        log::info!("OVERLAY: Mouse UP result: {:?}", result3);
                    } else {
                        // Regular click (left or right)
                        log::info!("OVERLAY: Executing CLICK asynchronously:");
                        log::info!("  Position: ({}, {})", tx, ty);
                        log::info!("  Button: {}", btn);
                        log::info!("  Button states: [1, 0] (DOWN then UP)");
                        log::info!("  Repeat: {}", rep);
                        log::info!("  Absolute: true");

                        let result = send(Request::Click {
                            x: tx,
                            y: ty,
                            button: btn,
                            button_states: vec![1, 0],
                            repeat: rep,
                            absolute: true,
                        });
                        log::info!("OVERLAY: Click request result: {:?}", result);
                    }
                    if let Some(guard) = app_guard.take() {
                        drop(guard);
                    }
                    app_ref.quit();
                    ControlFlow::Break
                });

                log::info!("╔══════════════════════════════════════════════════════════════╗");
                log::info!("║            OVERLAY: Action Complete                          ║");
                log::info!("╚══════════════════════════════════════════════════════════════╝");
            }
        }
        Propagation::Stop
    });
    window.add_controller(key_controller);

    let ws_clone = ws.clone();
    let cfg_clone = cfg.clone();
    window.connect_realize(move |window| {
        #[cfg(feature = "x11")]
        {
            if ws_clone.window_system_type == WindowSystemType::X11 && use_focus_anchor {
                if let Some(surface) = window.surface() {
                    if let Ok(x11_surface) = surface.downcast::<gdk4_x11::X11Surface>() {
                        let xid = x11_surface.xid();
                        let target_x = origin_x + cfg_clone.overlay_x_offset;
                        let target_y = origin_y + cfg_clone.overlay_y_offset;

                        // Spawn a thread to move the window to avoid blocking and allow WM to map it
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            let _ = std::process::Command::new("xdotool")
                                .arg("windowmove")
                                .arg(xid.to_string())
                                .arg(target_x.to_string())
                                .arg(target_y.to_string())
                                .output();
                        });
                    }
                }
            }
        }
    });

    // Present the window for better transparency support
    window.present();
}

fn monitor_for_point(x: i32, y: i32) -> Option<(gdk::Monitor, gdk::Rectangle)> {
    let display = gdk::Display::default()?;
    let monitors: ListModel = display.monitors();
    for idx in 0..monitors.n_items() {
        if let Some(item) = monitors.item(idx) {
            if let Ok(monitor) = item.downcast::<gdk::Monitor>() {
                let geo = monitor.geometry();
                if x >= geo.x()
                    && y >= geo.y()
                    && x < geo.x() + geo.width()
                    && y < geo.y() + geo.height()
                {
                    return Some((monitor, geo));
                }
            }
        }
    }
    None
}
