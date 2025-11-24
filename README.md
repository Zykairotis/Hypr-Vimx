# Hypr-Vimx

Starter home for a Hyprland + Neovim workflow: tiling Wayland desktop, modal editing, and a handful of helper scripts to glue them together. This repo is currently a skeleton so it can be cloned, tracked, and iterated on.

## Goals (early draft)
- Keep Hyprland config lean and readable (per-feature include files, clear defaults).
- Ship a Neovim setup focused on window/session control that mirrors Hyprland binds where sensible.
- Provide small helpers (scripts/services) to bridge compositor state (workspaces, scratchpads) with the editor.

## Getting Started
```bash
git clone https://github.com/Zykairotis/Hypr-Vimx
cd Hypr-Vimx
# add your configs under ./hypr/ and ./nvim/ before installing
```

Recommended layout once files are added:
- `hypr/` → Hyprland config fragments (e.g., `hypr.conf`, `binds.conf`, `windows.conf`).
- `nvim/` → Neovim config (Lua) with keymaps that parallel Hyprland binds.
- `scripts/` → helper scripts (workspace toggles, screenshot wrappers, etc.).

Install (example using stow once files exist):
```bash
stow -t ~/.config hypr
stow -t ~/.config nvim
```

## Contributing / Next Steps
- Fill in the initial Hyprland and Neovim configs that match your current setup.
- Add a short `scripts/README.md` describing each helper as it lands.
- Use concise, imperative commits (e.g., "Add basic Hyprland layout"; "Mirror Hypr binds in Neovim").

If you open issues/PRs, please include your compositor version, Neovim version, and any Wayland quirks hit during testing.
