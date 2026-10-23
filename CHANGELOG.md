# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `gamefocus stop` as an alias for `end`
- Linux embedded/gaming profiles park Cursor, Java/Gradle, and `llama-server`
- Optional `*` globs on `process_name` (e.g. `code-*`)
- VRAM preflight: WARN when a non-whitelisted process holds ≥500 MiB GPU memory (`nvidia-smi`)
- VRAM preflight resolves Docker/Podman/systemd-user names from cgroup when possible
- VRAM reclaim lifecycle: `[gpu.vram_reclaim]` policy (`warn`/`prompt`/`auto`), soft-stop matching Docker/Podman/systemd-user targets, restore on `end`; CLI `--reclaim-vram`
- Tray Start gaming prompts to soft-stop reclaimable VRAM targets; Stop gaming restores them
- `gamefocus run [--reclaim-vram] -- <cmd>...` starts a session, runs the command, ends on exit (Steam launch options)
- Linux Boost: `power_profiles` provider (`powerprofilesctl` → `performance`, restore on end; soft-fail)
- Linux Focus: after SIGSTOP, soft-fail `process_madvise(MADV_PAGEOUT)` on anonymous RSS
- Session impact summary after `start` (park count + MemAvailable delta when available)

### Fixed

- Tray icon stayed invisible because `TrayIcon` was dropped right after setup

### Changed

- Embedded profiles no longer park Discord (whitelisted); gamers keep voice chat during Focus
- Resume of already-exited parked PIDs (ESRCH) logs at debug instead of warn
- `process_name` park/whitelist matching is exact basename (case-insensitive), not substring

## [0.3.1] - 2026-10-21

### Fixed

- Linux GameMode Boost used the wrong D-Bus interface (`.Client`); RegisterGame now targets `com.feralinteractive.GameMode`

## [0.3.0] - 2026-10-21

### Added

- Unsigned tray installers on `v*` releases: Linux `.deb`, Windows NSIS setup `.exe`, macOS `.dmg`

## [0.2.0] - 2026-10-21

### Added

- Embedded OS default profiles so `gamefocus start` works with no `--profile`
- Profile resolve order: `--profile` → user `default.toml` → embedded
- Release archives include an `examples/` folder of TOML profiles
- Tray **Start gaming** / **Stop gaming**; Boost disabled on macOS

### Changed

- README quickstart for casual users; profile docs moved to `docs/PROFILES.md`
- Windows release packing uses a relative `dist/` path so zip creation works on GitHub Actions

### Fixed

- Windows release zip failed with `Cannot connect to D:` when tar saw absolute drive paths

## [0.1.0] - 2026-10-21

### Added

- Linux Focus (`SIGSTOP` / `SIGCONT`) and GameMode Boost soft-fail
- Windows Focus (`NtSuspendProcess` / `NtResumeProcess`) and Boost providers:
  `power_plan`, `working_set`, `cpu_affinity`
- macOS Focus (`SIGSTOP` / `SIGCONT`); no Boost provider yet
- Bundled profiles: `linux-gaming`, `windows-gaming`, `macos-gaming`
- Tauri tray UI for Focus/Boost toggles, profile select, start/end
- CLI: `profiles`, `start` / `end` / `status`, `--dry-run` park listing
- Default session snapshot under XDG runtime, Windows `LOCALAPPDATA`, or `~/.local/state`
- Git Flow workflow, CI matrix (Linux/Windows/macOS), UI job, Sonar gate, `v*` release builds
- Cortex competitive inventory from the Tiny10 lab (docs only)

### Changed

- Focus always runs before Boost so Boost can see parked pids
- End session rolls Boost back before resuming Focus
- Empty Boost `providers` applies no boosters (safe default)

### Security

- Focus only acts on the explicit park deny-list; shell protect stays on by default
- Working-set and CPU affinity touch only Focus-suspended background pids

[Unreleased]: https://github.com/giuliozelante/gamefocus/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/giuliozelante/gamefocus/releases/tag/v0.3.1
[0.3.0]: https://github.com/giuliozelante/gamefocus/releases/tag/v0.3.0
[0.2.0]: https://github.com/giuliozelante/gamefocus/releases/tag/v0.2.0
[0.1.0]: https://github.com/giuliozelante/gamefocus/releases/tag/v0.1.0
