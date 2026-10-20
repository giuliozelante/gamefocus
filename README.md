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

## Workflow

`develop` is the integration branch. Cut `feature/<issue>-slug` or `bugfix/<issue>-slug` from `develop` and open the pull request into `develop`. `main` stays the stable line.

Spec: [`docs/specs/2026-10-20-gamefocus.md`](docs/specs/2026-10-20-gamefocus.md).

## Status

Early scaffold (Phase 0–1):

- Rust workspace + CLI `gamefocus`
- Linux Focus (`SIGSTOP` / `SIGCONT`) + GameMode soft-fail boost
- Bundled profile `profiles/bundled/linux-gaming.toml`
- Competitive notes: [`docs/competitive/cortex-inventory.md`](docs/competitive/cortex-inventory.md)
- Windows / macOS / Tauri tray: planned

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

## Anti-cheat note

Prefer Focus **before** launching titles with aggressive anti-cheat. Avoid Boost that touches the protected game process. You are responsible for how you use system tools.

## Related projects

- [Feral GameMode](https://github.com/FeralInteractive/gamemode) — Linux OS boosts (we optionally call into it)
- [guapow](https://github.com/vinifmor/guapow) / [falcond](https://github.com/PikaOS-Linux/falcond) — Linux-only optimizers
- [OpenGameBoost](https://github.com/bensheed/OpenGameBoost) — Windows-only suspender

**gamefocus** aims at shared profiles + Focus/Boost toggles across Linux, Windows, and macOS.

## License

MIT
