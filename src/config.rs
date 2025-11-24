use crate::consts::{DEFAULT_ALPHABET, default_config_path};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub alphabet: String,
    /// Where to draw hints: only the focused window ("window") or the whole screen ("screen").
    pub overlay_target: OverlayTarget,
    pub overlay_x_offset: i32,
    pub overlay_y_offset: i32,
    pub window_system: String,
    pub backends: BackendsConfig,
    pub hints: HintsStyle,
    pub mouse: MouseConfig,
    pub overlay: OverlayConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BackendsConfig {
    pub enable: Vec<String>,
    pub atspi: AtspiConfig,
    pub opencv: OpencvConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AtspiConfig {
    pub states: Vec<String>,
    pub roles: Vec<String>,
    pub scale_factor: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OpencvConfig {
    pub kernel_size: i32,
    pub canny_min_val: f64,
    pub canny_max_val: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OverlayConfig {
    /// Whether to clear the background to transparent before drawing
    pub clear_background: bool,
    /// Background color for the overlay window (RGBA)
    pub background_color: (f64, f64, f64, f64),
    /// Whether to remove the default GTK background CSS class
    pub remove_background_class: bool,
    /// Whether to use layer-shell on Wayland
    pub use_layer_shell: bool,
    /// Layer-shell namespace
    pub layer_shell_namespace: String,
    /// Whether to set exclusive zone (-1 for transparency)
    pub layer_shell_exclusive_zone: i32,
    /// Debug overlay settings
    pub debug_overlay_enabled: bool,
    pub debug_overlay_color: (f64, f64, f64, f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HintsStyle {
    pub hint_height: i32,
    pub hint_width_padding: i32,
    pub hint_font_size: i32,
    pub hint_font_face: String,
    pub hint_font_color: (f64, f64, f64, f64),
    pub hint_pressed_font_color: (f64, f64, f64, f64),
    pub hint_background_color: (f64, f64, f64, f64),
    pub hint_uppercase: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MouseConfig {
    pub move_left: String,
    pub move_right: String,
    pub move_up: String,
    pub move_down: String,
    pub scroll_left: String,
    pub scroll_right: String,
    pub scroll_up: String,
    pub scroll_down: String,
    pub move_pixel_sensitivity: i32,
    pub move_rampup_time: f32,
    pub scroll_pixel_sensitivity: i32,
    pub scroll_rampup_time: f32,
    pub exit_key: u32,
    pub hover_modifier: u32,
    pub grab_modifier: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OverlayTarget {
    Window,
    Screen,
}

impl Default for OverlayTarget {
    fn default() -> Self {
        OverlayTarget::Window
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            alphabet: DEFAULT_ALPHABET.to_string(),
            overlay_target: OverlayTarget::Window,
            overlay_x_offset: 0,
            overlay_y_offset: 0,
            window_system: "".into(),
            backends: BackendsConfig::default(),
            hints: HintsStyle::default(),
            mouse: MouseConfig::default(),
            overlay: OverlayConfig::default(),
        }
    }
}

impl Default for BackendsConfig {
    fn default() -> Self {
        Self {
            enable: vec!["atspi".into(), "opencv".into()],
            atspi: AtspiConfig::default(),
            opencv: OpencvConfig::default(),
        }
    }
}

impl Default for AtspiConfig {
    fn default() -> Self {
        Self {
            // state/role names match atspi::StateType/Role stringified variants
            states: vec!["Sensitive".into(), "Showing".into(), "Visible".into()],
            roles: vec![
                "PushButton".into(),
                "CheckBox".into(),
                "RadioButton".into(),
                "ToggleButton".into(),
                "MenuItem".into(),
                "ListItem".into(),
                "Text".into(),
                "Entry".into(),
            ],
            scale_factor: 1.0,
        }
    }
}

impl Default for OpencvConfig {
    fn default() -> Self {
        Self {
            kernel_size: 6,
            canny_min_val: 100.0,
            canny_max_val: 200.0,
        }
    }
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            clear_background: true,
            background_color: (0.0, 0.0, 0.0, 0.0),
            remove_background_class: true,
            use_layer_shell: true,
            layer_shell_namespace: "hints".into(),
            layer_shell_exclusive_zone: -1,
            debug_overlay_enabled: false,
            debug_overlay_color: (1.0, 0.0, 1.0, 0.2),
        }
    }
}

impl Default for HintsStyle {
    fn default() -> Self {
        Self {
            hint_height: 30,
            hint_width_padding: 10,
            hint_font_size: 15,
            hint_font_face: "Sans".into(),
            hint_font_color: (0.0, 0.0, 0.0, 1.0),
            hint_pressed_font_color: (0.7, 0.7, 0.4, 1.0),
            hint_background_color: (1.0, 1.0, 0.5, 0.8),
            hint_uppercase: true,
        }
    }
}

impl Default for MouseConfig {
    fn default() -> Self {
        Self {
            move_left: "h".into(),
            move_right: "l".into(),
            move_up: "k".into(),
            move_down: "j".into(),
            scroll_left: "h".into(),
            scroll_right: "l".into(),
            scroll_up: "k".into(),
            scroll_down: "j".into(),
            move_pixel_sensitivity: 10,
            move_rampup_time: 0.5,
            scroll_pixel_sensitivity: 5,
            scroll_rampup_time: 0.5,
            exit_key: 65307,        // GDK_KEY_Escape
            hover_modifier: 1 << 2, // Control
            grab_modifier: 1 << 3,  // Alt/Mod1
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let path = default_config_path();
        if let Ok(contents) = fs::read_to_string(&path) {
            serde_json::from_str::<Config>(&contents).unwrap_or_default()
        } else {
            Config::default()
        }
    }
}
