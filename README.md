# rust-hintsx

Rust rewrite of **hints** (keyboard-driven GUI mouse control) targeting Wayland (Hyprland/Sway/Plasma) and X11. It shows overlaid hint labels for accessible UI elements and clicks them via a uinput mouse daemon.

## Binaries
- `hintsd` — mouse daemon exposing a Unix socket at `/tmp/hints.socket`.
- `hintsx` — main UI; gathers elements via AT-SPI (default) or OpenCV + grim fallback, renders GTK4 overlay, and sends click requests to `hintsd`.

## Features

- **Fast screenshot capture** using PPM format with stdout piping
- **Parallel AT-SPI traversal** for efficient accessibility tree scanning
- **Multiple backends**: AT-SPI (accessibility) and OpenCV (computer vision)
- **Full keyboard control** with modifier support:
  - Click once: `<hint>` (e.g., `jk`)
  - Click multiple times: `<number><hint>` (e.g., `2jk` for double-click)
  - Right click: `Shift + <hint>`
  - Drag: `Alt + <hint>` (may not work on all Wayland compositors)
  - Hover: `Ctrl + <hint>`
  - Move mouse: `h` (left), `j` (down), `k` (up), `l` (right)
  - Scroll: `Shift + h/j/k/l`
  - Exit: `Esc`

## Build
```bash
cargo build --release
```

## Run
1. Start the mouse daemon (needs uinput access):
   ```bash
   ./target/release/hintsd &
   ```
2. Launch the overlay:
   ```bash
   ./target/release/hintsx
   ```

## Config
Configuration is read from `~/.config/hints/config.json` if present; otherwise built-in defaults are used (alphabet, keybindings, colors, OpenCV thresholds).

## Notes
- Wayland overlays use `gtk4-layer-shell`; make sure the compositor allows overlay surfaces (Hyprland/Sway/Plasma work; GNOME blocks layer-shell overlays by design).
- OpenCV fallback requires `grim` for screenshots.
- AT-SPI backend needs accessibility enabled (same prerequisites as the original Python project).
