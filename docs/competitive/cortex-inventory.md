# Razer Cortex — competitive inventory (Phase 0.5)

**Status:** complete for v1 planning (Tiny10 lab install + host-side PE strings/imports/exports). Deep decompiler REA blocked (`GHIDRA_INSTALL_DIR` unset; Hopper/IDA not configured). Checklist is enough for clean-room Focus/Boost design.

**Rules:** documentation only. No decompiled code, assets, or string tables copied into product sources. Extracted binaries stay under `~/VMs/gamefocus-windows/shared/extracted/` — **not** in git.

## Artifact analyzed (installer bootstrapper)

| Item | Value |
|------|--------|
| Path | `/home/gzelante/Downloads/RazerCortexInstaller.exe` |
| Format | PE32 GUI installer (x86) |
| SHA-256 | `e4eabf1a0cf030b773a1a14ed22f4cb337c5f85638863260215b6315d83eb570` |
| Nested payload | PE resource `RCDATA` @ file offset `224752` (`~13.8 MiB`) → itself a **.NET** PE (`CortexSetupPayload` / Razer Installer) with an embedded ZIP of installer DLLs |

### What the download actually is

The “Cortex installer” on disk is an **online bootstrapper**, not the Game Booster engine:

- Discovery: `discovery3.razerapi.com`
- Manifest: `manifest3.razerapi.com`
- Bundled binaries are installer/UI/analytics (`RazerInstaller.exe`, `Razer.Installer.*.dll`, `Razer.Analytics.dll`, AWS Kinesis SDK, NLog, PInvoke.*).

**Implication:** REA of this EXE cannot inventory Game Booster behavior. That requires installing inside a Windows VM (so the real product bits are downloaded), then analyzing those binaries.

## Public / marketing-level feature map (informational)

Mapped to **gamefocus** Focus (A) / Boost (B) / **won’t copy**:

| Cortex-area capability | Bucket | gamefocus stance |
|------------------------|--------|------------------|
| Close / suspend background apps while gaming | **Focus (A)** | Implement (suspend preferred; soft-stop fallback) |
| Whitelist (e.g. Discord, chat) | **Focus (A)** | Implement via profile TOML |
| Auto detect games / launchers | **Focus (A)** | Implement via matchers (Steam/Proton/cmdline) |
| High-performance power plan | **Boost (B)** | Windows `power_plan` provider |
| Memory “cleanup” / working-set trim | **Boost (B)** | Windows `working_set` on Focus-suspended pids only; never scrub game + anti-cheat |
| GPU / driver “optimizer” packs | **won’t copy** | Out of scope |
| FPS / in-game overlay | **won’t copy** | Out of scope (v1 non-goal) |
| Rewards / account / store hooks | **won’t copy** | Out of scope |
| Dehumidifier / ambient toys | **won’t copy** | Out of scope |
| Telemetry / analytics (installer already ships `Razer.Analytics.dll` + Kinesis) | **won’t copy** | No telemetry by default |

## Static installer observations

- Nested payload magic: `MZ` (.NET assembly).
- Installer configuration points at Razer discovery/manifest CDN — product bits arrive after network install.
- Interesting DLL names for trust narrative (do **not** ship equivalents): `Razer.Analytics.dll`, `AWSSDK.Kinesis.dll`.
- No embedded Game Booster process list or suspend logic in the analyzed ZIP entries.

## Safety notes for gamefocus (from this inventory)

- Prefer **Focus before launch** for competitive titles; avoid Boost that touches the protected game process.
- Document privilege needs (user vs admin/polkit) when Boost providers need elevation.
- Treat Cortex RE notes as a checklist only — clean-room Rust remains the implementation source.

## Lab VM

Throwaway Tiny10 via `dockur/windows` at `~/VMs/gamefocus-windows` (`ADAPTER=e1000`).

- Viewer: http://127.0.0.1:8006
- RDP: 127.0.0.1:3389 (`Docker` / `admin`)
- Installer on guest Shared (`Z:\RazerCortexInstaller.exe`)
- Host extracts: `~/VMs/gamefocus-windows/shared/extracted/`
- Start again: `cd ~/VMs/gamefocus-windows && docker compose up -d`

## Post-install observations (2026-10-20)

Installed product path:

`C:\Program Files (x86)\Razer\Razer Cortex\`

Cortex is downloaded by the Razer Gaming Software hub (~426 MB), not embedded in `RazerCortexInstaller.exe`.

### Key binaries (host copy under `shared/extracted/bin/` — **not** committed)

| File | Role (from names + strings) |
|------|-----------------------------|
| `RazerCortex.exe` / `.Shell.exe` | Main UI shell |
| `RazerCortexBoostHelper.exe` | Boost helper process |
| `Boost.dll` | Native Game Booster engine (PE32+ x64) |
| `RazerCortex.GameBoost.dll` | Managed Game Boost settings / item lists |
| `RazerCortex.SystemOptimize.dll` / `SystemClean.dll` / `SystemSetting.dll` | System tweak modules |
| `RazerCortex.GameOverlay.dll` / `GameClip.dll` / `FPSRunner.exe` | Overlay / clips / FPS — **won't copy** |
| AWS Kinesis SDK (in install dir) | Telemetry — **won't copy** |

### `Boost.dll` identity

| Field | Value |
|-------|--------|
| SHA-256 | `ac11cd0c8ab7bb1a5a0b874dbb697856821e1affd085f5b42a9faa87c229dcd7` |
| Size | 460488 bytes |
| Format | PE32+ DLL x86-64 |
| PDB path (string) | `...\razer.gameboost\src\BoostDLL\Razer.BoostDll\...` |

### `CBoostManage::StartBoost` / `Restore` module map

| Module | Evidence | Bucket for gamefocus |
|--------|----------|----------------------|
| Process Boost / Restore | `CProcessBoost::*`; log `kill process %s success/failed`; restore relaunches via `ShellExecute(Ex)` | **Focus (A)** — but Cortex **terminates + relaunches**; gamefocus prefers **suspend/resume** |
| BackgroundApps Boost | `CBackgroundAppsBoost::*` registry modify/restore | **Focus (A)** / optional |
| Service Boost | `ControlService` / `EnumDependentServicesW`; `__GetServicesDetail` | **out of scope** by default |
| Power Boost | power-plan name/count strings; `GUID_DEVICE_POWER_POLICY_VIDEO_BRIGHTNESS` | **Boost (B)** — power plan |
| Ram Boost | import `K32EmptyWorkingSet` | **Boost (B)** — background only |
| CPU unpark | `CCpuUnarkBoost::ModifyCoreUnparkReg` | **Boost (B)** (Linux: GameMode-ish) |
| CPU affinity | `SetProcessAffinityMask` / whitelist file APIs | **Boost (B)** optional |
| Clipboard / Desktop / Cortana / Telemetry / ReportProblems | dedicated `*Boost` classes; Cortana string: `Cortana.exe is suspended` | **won't copy** / optional later |
| Key / WinKey disabler | StartBoost readiness logs | **won't copy** |

### PE imports that matter (checklist, not a recipe)

`KERNEL32`: `OpenProcess`, `TerminateProcess`, `CreateToolhelp32Snapshot`, `Process32FirstW`/`NextW`, `K32EmptyWorkingSet`, `SetProcessAffinityMask` / `GetProcessAffinityMask`.

`ADVAPI32`: registry R/W (`RegOpenKeyExW` … `RegSetValueExW`), `ControlService`, `OpenProcessToken`.

`SHELL32`: `ShellExecuteW` / `ShellExecuteExW` (process restore path).

**Not imported** on this `Boost.dll`: `NtSuspendProcess` / `SuspendThread`. Process “boost” here is kill+relaunch; only some specialty modules (e.g. Cortana) mention suspend in strings.

### Public exports (API surface names only)

`__InitModule`, `__ReleaseModule`, `__StartBoost`, `__StartSafeBoost`, `__Restore`, `__Stop`, `__IsBoosted`, `__UpdateConfiguartion`, `__GetProcessesDetail`, `__GetServicesDetail`, `__SetGamePid`, `__SetGamePaths`, CPU affinity/unpark getters/setters/whitelist helpers, `__ExitGameDesk`.

### Managed layer (`RazerCortex.GameBoost.dll`)

Type/method name fragments only: process / Windows-service / third-party-service / special boost *items*, hotkey start, Chroma hooks. Confirms UI configures per-item toggles — maps to gamefocus profile TOML sections, not a shared binary.

### Full string dump (local only)

`~/VMs/gamefocus-windows/shared/extracted/strings-inventory.md`

## Design takeaway for gamefocus

1. **Focus ≠ Cortex Process Boost.** Prefer `SIGSTOP`/`SIGCONT` (Linux) and Windows suspend APIs over terminate+relaunch.
2. **Whitelist** belongs on Focus park lists (and optionally Boost affinity filters) — profile-driven.
3. **Boost** = reversible power / GameMode / working-set trim on already-parked backgrounds — never the protected game.
4. Overlay, rewards, telemetry, defrag, key remaps stay non-goals.
