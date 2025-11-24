# Repository Guidelines

## Project Structure & Module Organization
- `hints/`: Python package entry (`hints/hints.py`), overlays in `hints/huds/`, accessibility backends in `hints/backends/`, window adapters in `hints/window_systems/`, mouse logic in `mouse.py` + `mouse_service.py`, shared config in `utils.py` and `constants.py`, compositor helpers in `hints/scripts/kwin/`.
- `rust-hintsx/`: Rust rewrite. Binaries: `hintsx` (GTK4 overlay/UI) and `hintsd` (uinput mouse daemon over `/tmp/hints.socket`). Build outputs land in `rust-hintsx/target/`.
- Top-level docs: `README.md` and `rust-hintsx/README.md`.

## Build, Test, and Development Commands
- Python: `python3 -m venv venv && source venv/bin/activate` then `pip install -e .` to register `hints`/`hintsd` entrypoints and install the user systemd unit.
- Python smoke: `python -m compileall hints` or `python -m hints --help` to catch import/runtime issues.
- Rust: run inside `rust-hintsx/` — `cargo build --release` for optimized binaries; `cargo run --bin hintsd` and `cargo run --bin hintsx` for iterative dev.
- Format/lint: `cargo fmt` then `cargo clippy --all-targets --all-features -D warnings` before pushing Rust changes.

## Coding Style & Naming Conventions
- Python: PEP 8, 4-space indents, snake_case for functions/vars, CamelCase for classes, ALL_CAPS for constants; docstrings on public functions; prefer `logging` over prints and keep GTK/AT-SPI imports local to the modules that need them.
- Rust: rustfmt defaults; modules snake_case; keep binaries thin and gate compositor-specific logic with clear flags/guards. Avoid adding new runtime dependencies without discussion; document Linux-only assumptions in comments.

## Testing Guidelines
- No formal suite yet. Manual smoke: start `hintsd`, run `hints` (Python) or `cargo run --bin hintsx` (Rust), and exercise click/drag/scroll on at least one X11 and one Wayland compositor (Hyprland/Sway/Plasma preferred; GNOME Wayland unsupported).
- When touching daemon install paths, reinstall in a throwaway venv to validate `HINTS_EXPECTED_BIN_DIR`. For Rust socket interactions, run `cargo run --bin hintsd` alongside `cargo run --bin hintsx`.

## Commit & Pull Request Guidelines
- Commit messages: short imperative summaries under ~72 chars (e.g., "Fix scroll freeze", "Add Plasma overlay guard"); mention platform/scope when helpful.
- PRs: describe motivation, approach, risk; link issues (`Fixes #NN`); list manual tests (compositor, window system, keyboard layout); include screenshots/gifs for visible UI changes; call out follow-ups or config changes needed. Keep changes small and single-purpose.

## Security & Configuration Tips
- `hintsd` needs uinput access; ensure appropriate permissions before running. Wayland overlays rely on `gtk4-layer-shell`; Hyprland/Sway/Plasma allow overlays, GNOME blocks them by design. OpenCV fallback requires `grim` for screenshots; enable accessibility for AT-SPI usage.
