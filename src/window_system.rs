use anyhow::Result;
use regex::Regex;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowSystemType {
    X11,
    Wayland,
}

#[derive(Debug, Clone)]
pub struct WindowSystem {
    pub window_system_type: WindowSystemType,
    pub window_system_name: String,
    pub bar_height: i32,
}

impl WindowSystem {
    pub fn detect(preferred: &str) -> Result<Self> {
        if !preferred.is_empty() {
            return Ok(Self {
                window_system_type: if preferred.to_lowercase() == "x11" {
                    WindowSystemType::X11
                } else {
                    WindowSystemType::Wayland
                },
                window_system_name: preferred.to_lowercase(),
                bar_height: 0,
            });
        }

        let session_type = std::env::var("XDG_SESSION_TYPE")
            .or_else(|_| std::env::var("WAYLAND_DISPLAY"))
            .unwrap_or_else(|_| "x11".to_string());

        let window_system_type = if session_type.to_lowercase().contains("wayland") {
            WindowSystemType::Wayland
        } else {
            WindowSystemType::X11
        };

        let wm = detect_wayland_wm().unwrap_or_else(|| "unknown".into());

        Ok(Self {
            window_system_type,
            window_system_name: wm,
            bar_height: 0,
        })
    }

    pub fn get_active_window_geometry_x11(&self) -> Option<(i32, i32, i32, i32)> {
        // Try xdotool for X11 or XWayland
        let output = std::process::Command::new("xdotool")
            .args(["getactivewindow", "getwindowgeometry", "--shell"])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let content = String::from_utf8_lossy(&output.stdout);
        let mut x = 0;
        let mut y = 0;
        let mut w = 0;
        let mut h = 0;

        for line in content.lines() {
            if let Some(val) = line.strip_prefix("X=") {
                x = val.parse().unwrap_or(0);
            } else if let Some(val) = line.strip_prefix("Y=") {
                y = val.parse().unwrap_or(0);
            } else if let Some(val) = line.strip_prefix("WIDTH=") {
                w = val.parse().unwrap_or(0);
            } else if let Some(val) = line.strip_prefix("HEIGHT=") {
                h = val.parse().unwrap_or(0);
            }
        }

        if w > 0 && h > 0 {
            Some((x, y, w, h))
        } else {
            None
        }
    }

    pub fn get_active_window_geometry_wayland(&self) -> Option<(i32, i32, i32, i32)> {
        if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
            return self.get_hyprland_active_window();
        } else if std::env::var("SWAYSOCK").is_ok() {
            return self.get_sway_active_window();
        }
        None
    }

    fn get_hyprland_active_window(&self) -> Option<(i32, i32, i32, i32)> {
        let output = std::process::Command::new("hyprctl")
            .args(["activewindow", "-j"])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
        let at = json.get("at")?.as_array()?;
        let size = json.get("size")?.as_array()?;

        let x = at.get(0)?.as_i64()? as i32;
        let y = at.get(1)?.as_i64()? as i32;
        let w = size.get(0)?.as_i64()? as i32;
        let h = size.get(1)?.as_i64()? as i32;

        Some((x, y, w, h))
    }

    fn get_sway_active_window(&self) -> Option<(i32, i32, i32, i32)> {
        let output = std::process::Command::new("swaymsg")
            .args(["-t", "get_tree"])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;

        // Recursive search for focused node
        fn find_focused(node: &serde_json::Value) -> Option<(i32, i32, i32, i32)> {
            if node.get("focused").and_then(|v| v.as_bool()) == Some(true) {
                if let Some(rect) = node.get("rect") {
                    let x = rect.get("x")?.as_i64()? as i32;
                    let y = rect.get("y")?.as_i64()? as i32;
                    let w = rect.get("width")?.as_i64()? as i32;
                    let h = rect.get("height")?.as_i64()? as i32;
                    return Some((x, y, w, h));
                }
            }

            if let Some(nodes) = node.get("nodes").and_then(|v| v.as_array()) {
                for child in nodes {
                    if let Some(res) = find_focused(child) {
                        return Some(res);
                    }
                }
            }

            // Also check floating_nodes
            if let Some(nodes) = node.get("floating_nodes").and_then(|v| v.as_array()) {
                for child in nodes {
                    if let Some(res) = find_focused(child) {
                        return Some(res);
                    }
                }
            }

            None
        }

        find_focused(&json)
    }
}

fn detect_wayland_wm() -> Option<String> {
    let pattern = Regex::new(r"(?i)^(sway|hyprland|plasmashell|kwin_wayland|wayfire)$").ok()?;
    let output = Command::new("ps")
        .args(["-e", "-o", "comm"])
        .output()
        .ok()?;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let token = line.trim();
        if pattern.is_match(token) {
            return Some(token.to_lowercase());
        }
    }
    None
}
