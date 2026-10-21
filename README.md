# gamefocus

Cross-platform **game focus** tool: free RAM/CPU for a session without vendor lock-in.

Inspired by the *useful* part of Razer Cortex — not a clone.

## Modes (independent toggles)

| Switch | What it does |
|--------|----------------|
| **Focus** | Suspend (preferred) or soft-stop processes listed in the profile `park` list; restore on session end |
| **Boost** | Reversible OS tweaks (Linux: GameMode; Windows: High Performance power plan, working-set trim, and CPU affinity on parked pids). macOS has no Boost provider yet. An empty provider list applies none |

Use Focus only, Boost only, or both. Gaming preset defaults: both on.

## Non-goals (v1)

FPS overlay, rewards, dehumidifier, driver “optimizer” packs, telemetry, Cortex assets/code.

## Workflow

`develop` is the integration branch. Cut `feature/<issue>-slug` or `bugfix/<issue>-slug` from `develop` and open the pull request into `develop`. `main` stays the stable line.

Spec: [`docs/specs/2026-10-20-gamefocus.md`](docs/specs/2026-10-20-gamefocus.md).
Changelog: [`CHANGELOG.md`](CHANGELOG.md). Releases: [v0.3.1](https://github.com/giuliozelante/gamefocus/releases/tag/v0.3.1).

Story and failing test first for Rust changes (`.cursor/rules/agile-tdd.mdc`). SonarQube Cloud scans on `develop` and `main` after the `SONAR_TOKEN` secret exists. Tag `v*` builds the CLI archives plus unsigned tray installers (`.deb` / NSIS `.exe` / `.dmg`).

## Status

Phase 0–2 + tray scaffold:

- Rust workspace + CLI `gamefocus`
- Linux Focus (`SIGSTOP` / `SIGCONT`) + GameMode soft-fail boost
- Bundled profiles `profiles/bundled/{linux,windows,macos}-gaming.toml`
- Competitive notes: [`docs/competitive/cortex-inventory.md`](docs/competitive/cortex-inventory.md) (VM inventory done)
- Tauri tray UI: [`crates/gamefocus-ui`](crates/gamefocus-ui) (needs `webkit2gtk-4.1` on Linux)
- Windows Focus suspend/resume, power-plan Boost (`power_plan`), and working-set Boost (`working_set`)
- macOS Focus suspend/resume (`SIGSTOP` / `SIGCONT`). No Boost provider yet

## Quickstart (casual)

**Tray (easiest):** download the installer for your OS from [Releases](https://github.com/giuliozelante/gamefocus/releases) (`.deb` / Windows setup `.exe` / `.dmg`), install, then click **Start gaming**. Installers are unsigned — macOS/Windows may ask you to allow the app once.

**CLI:** download a release archive, extract, then:

```bash
./gamefocus start          # uses embedded OS default; no profile path needed
./gamefocus status
./gamefocus end
```

Customize later: see [`docs/PROFILES.md`](docs/PROFILES.md).

## Build

```bash
# If rustup proxies mis-detect the binary name under Cursor:
export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
# or pin the installed toolchain:
export PATH="$HOME/.rustup/toolchains/1.83.0-x86_64-unknown-linux-gnu/bin:$PATH"

cargo build -p gamefocus-cli
cargo test
cargo run -p gamefocus-cli -- start --dry-run
```

## CLI

```bash
# Zero-config (embedded default, or ~/.config/gamefocus/default.toml if present)
cargo run -p gamefocus-cli -- start --dry-run
cargo run -p gamefocus-cli -- start

# Optional explicit profile
cargo run -p gamefocus-cli -- start --profile bundled/linux-gaming.toml --dry-run

cargo run -p gamefocus-cli -- profiles
cargo run -p gamefocus-cli -- status
cargo run -p gamefocus-cli -- end
```

**Safety:** Focus only acts on the explicit `[[park]]` list. Empty `park` = no suspensions. Shell/compositor names are protected when `protect_shell = true`. Empty Boost `providers` applies none.

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
