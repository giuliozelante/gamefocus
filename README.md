# gamefocus

Cross-platform **game focus** tool: free RAM/CPU for a session without vendor lock-in.

Inspired by the *useful* part of Razer Cortex — not a clone.

## Modes (independent toggles)

| Switch | What it does |
|--------|----------------|
| **Focus** | Suspend (preferred) or soft-stop processes listed in the profile `park` list; restore on session end |
| **Boost** | Apply reversible OS tweaks for the session (Linux: Feral GameMode when available) |

Use Focus only, Boost only, or both. Gaming preset defaults: both on.

## Non-goals (v1)

FPS overlay, rewards, dehumidifier, driver “optimizer” packs, telemetry, Cortex assets/code.

## Status

Phase 0–2 + tray scaffold:

- Rust workspace + CLI `gamefocus`
- Linux Focus (`SIGSTOP` / `SIGCONT`) + GameMode soft-fail boost
- Bundled profile `profiles/bundled/linux-gaming.toml`
- Competitive notes: [`docs/competitive/cortex-inventory.md`](docs/competitive/cortex-inventory.md) (VM inventory done)
- Tauri tray UI: [`crates/gamefocus-ui`](crates/gamefocus-ui) (needs `webkit2gtk-4.1` on Linux)
- Windows / macOS Focus: planned

## Build

```bash
# If rustup proxies mis-detect the binary name under Cursor:
export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
# or pin the installed toolchain:
export PATH="$HOME/.rustup/toolchains/1.83.0-x86_64-unknown-linux-gnu/bin:$PATH"

cargo build -p gamefocus-cli
cargo test
```

## CLI

```bash
# Preview (no suspend)
cargo run -p gamefocus-cli -- start \
  --profile profiles/bundled/linux-gaming.toml \
  --dry-run

# Start Focus+Boost (writes gamefocus-session.json)
cargo run -p gamefocus-cli -- start \
  --profile profiles/bundled/linux-gaming.toml \
  --focus on --boost on

# Focus only / Boost only
cargo run -p gamefocus-cli -- start --profile profiles/bundled/linux-gaming.toml --focus on --boost off
cargo run -p gamefocus-cli -- start --profile profiles/bundled/linux-gaming.toml --focus off --boost on

cargo run -p gamefocus-cli -- status
cargo run -p gamefocus-cli -- end
```

**Safety:** Focus only acts on the explicit `[[park]]` list. Empty `park` = no suspensions. Shell/compositor names are protected when `protect_shell = true`.

## Tray UI (Tauri)

```bash
# Linux deps once: sudo pacman -S webkit2gtk-4.1
export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
cargo tauri dev --manifest-path crates/gamefocus-ui/src-tauri/Cargo.toml
```

See [`crates/gamefocus-ui/README.md`](crates/gamefocus-ui/README.md).

## Anti-cheat note

Prefer Focus **before** launching titles with aggressive anti-cheat. Avoid Boost that touches the protected game process. You are responsible for how you use system tools.

## Related projects

- [Feral GameMode](https://github.com/FeralInteractive/gamemode) — Linux OS boosts (we optionally call into it)
- [guapow](https://github.com/vinifmor/guapow) / [falcond](https://github.com/PikaOS-Linux/falcond) — Linux-only optimizers
- [OpenGameBoost](https://github.com/bensheed/OpenGameBoost) — Windows-only suspender

**gamefocus** aims at shared profiles + Focus/Boost toggles across Linux, Windows, and macOS.

## License

MIT
