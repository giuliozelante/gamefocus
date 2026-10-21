# gamefocus

```text
Goal:
  Cross-platform Focus (park/restore processes) and Boost (reversible OS tweaks)
  as independent toggles. Clean-room Rust. Cortex is a checklist, not a source tree.

In:
  Rust workspace, Linux Focus + GameMode soft-fail, bundled profile,
  CLI, Git Flow (main stable, develop integration),
  GitHub Actions on Linux, Windows, and macOS.
  The Tauri tray is a separate issue; its CI job is added with that crate.

Out:
  FPS overlay, rewards, driver packs, telemetry, Cortex binaries in git,
  terminate-and-relaunch process boost, service killing by default.

Behavior:
  Focus acts only on the profile park list. Empty park suspends nothing.
  Shell and compositor names stay protected when protect_shell is true.
  Boost soft-fails per provider. Focus and Boost can each be on or off.

Constraints:
  One issue, one branch from develop: feature/<issue>-slug or bugfix/<issue>-slug.
  Pull requests target develop. main is the stable line.
  Spec updates live in docs/specs/. Proprietary Cortex bits stay outside the repo.

Done when:
  cargo test, cargo clippy -D warnings, and cargo build pass for the
  workspace members that CI builds, and the matching GitHub Actions jobs are green.
  Tag pushes v* publish CLI archives for Linux x64, Windows x64, and macOS arm64.
  SonarQube Cloud scans develop and main once SONAR_TOKEN is set.
```

## Layout

| Path | Role |
|------|------|
| `crates/gamefocus-core` | Profiles, session, Focus/Boost traits |
| `crates/gamefocus-linux` | SIGSTOP/SIGCONT + GameMode |
| `crates/gamefocus-windows` | Windows Focus and power-plan Boost |
| `crates/gamefocus-macos` | macOS Focus (`SIGSTOP` / `SIGCONT`). No Boost provider yet |
| `crates/gamefocus-cli` | `gamefocus` CLI |
| `crates/gamefocus-ui` | Tauri tray (its own issue) |
| `profiles/bundled/` | Shipped TOML profiles |
| `docs/competitive/` | Cortex inventory, no copied code |
| `docs/specs/` | Specs for work that outlives a chat |

## Commands

```bash
export PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
cargo test --workspace --exclude gamefocus-ui
cargo clippy --workspace --exclude gamefocus-ui --all-targets -- -D warnings
cargo build --workspace --exclude gamefocus-ui
```

Linux UI CI needs `webkit2gtk-4.1` (`.github/workflows/ui.yml`).

## Quality and release

- Agile and TDD for this repo: `.cursor/rules/agile-tdd.mdc`
- Sonar: `.cursor/rules/sonar.mdc`, `sonar-project.properties`, secret `SONAR_TOKEN`
- Release: push tag `v*` (`.github/workflows/release.yml`)

