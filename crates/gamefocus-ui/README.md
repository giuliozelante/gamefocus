# gamefocus UI (Tauri)

System tray + small control window for Focus/Boost sessions.

## Prerequisites (Linux)

```bash
# Arch / CachyOS
sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl \
  appmenu-gtk-module libappindicator-gtk3 librsvg
```

Install the Tauri CLI once:

```bash
export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
cargo install tauri-cli --version "^2.0.0" --locked
```

## Run

From the repo root:

```bash
export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
cargo tauri dev --manifest-path crates/gamefocus-ui/src-tauri/Cargo.toml
```

Or:

```bash
cd crates/gamefocus-ui
cargo tauri dev
```

## Bundle (local)

```bash
cargo tauri build --bundles deb --manifest-path crates/gamefocus-ui/src-tauri/Cargo.toml
# Windows: --bundles nsis
# macOS:   --bundles dmg
```

Tagged GitHub Releases publish unsigned installers (`.deb` / NSIS setup `.exe` / `.dmg`). Code signing is not configured yet.

## Controls

- Window: pick profile, toggle Focus/Boost, Start / End
- Tray: same toggles + Start / End / Quit (closing the window hides it)
