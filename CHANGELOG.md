# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
