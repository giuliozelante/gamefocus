# Razer Cortex — competitive inventory (Phase 0.5)

**Status:** partial (static + public product knowledge). Full post-install `Cortex.exe` REA is **blocked** on this machine (no Windows VM / qemu / VirtualBox).

**Rules:** documentation only. No decompiled code, assets, or string tables copied into product sources.

## Artifact analyzed

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

**Implication:** REA of this EXE cannot inventory Game Booster behavior. That requires installing inside a Windows VM (so the real `Cortex.exe` / helpers are downloaded), then analyzing those binaries.

## Follow-up when a Windows VM is available

1. Snapshot a clean Win10/11 VM.
2. Install from the same installer; note install path, services, scheduled tasks.
3. REA: `open_binary` on `Cortex.exe` and helpers; strings / imports for process suspend, power plan, memory trim, overlay.
4. Optional: one Game Booster session + Process Explorer to list suspended processes.
5. Append findings to this doc under “Post-install observations”.

## Public / marketing-level feature map (informational)

Mapped to **gamefocus** Focus (A) / Boost (B) / **won’t copy**:

| Cortex-area capability | Bucket | gamefocus stance |
|------------------------|--------|------------------|
| Close / suspend background apps while gaming | **Focus (A)** | Implement (suspend preferred; soft-stop fallback) |
| Whitelist (e.g. Discord, chat) | **Focus (A)** | Implement via profile TOML |
| Auto detect games / launchers | **Focus (A)** | Implement via matchers (Steam/Proton/cmdline) |
| High-performance power plan | **Boost (B)** | Windows provider (Phase 4) |
| Memory “cleanup” / working-set trim | **Boost (B)** | Only on already-suspended *background* processes; never scrub game + anti-cheat |
| GPU / driver “optimizer” packs | **won’t copy** | Out of scope |
| FPS / in-game overlay | **won’t copy** | Out of scope (v1 non-goal) |
| Rewards / account / store hooks | **won’t copy** | Out of scope |
| Dehumidifier / ambient toys | **won’t copy** | Out of scope |
| Telemetry / analytics (installer already ships `Razer.Analytics.dll` + Kinesis) | **won’t copy** | No telemetry by default |

## Static installer observations (this session)

- Nested payload magic: `MZ` (.NET assembly).
- Installer configuration points at Razer discovery/manifest CDN — product bits arrive after network install.
- Interesting DLL names for trust narrative (do **not** ship equivalents): `Razer.Analytics.dll`, `AWSSDK.Kinesis.dll`.
- No embedded Game Booster process list or suspend logic in the analyzed ZIP entries.

## Safety notes for gamefocus (from this inventory)

- Prefer **Focus before launch** for competitive titles; avoid Boost that touches the protected game process.
- Document privilege needs (user vs admin/polkit) when Boost providers need elevation.
- Treat any future Cortex RE notes as a checklist only — clean-room Rust remains the implementation source.

## Lab VM (added)

Throwaway Tiny10 via `dockur/windows` at `~/VMs/gamefocus-windows`.
- Viewer: http://127.0.0.1:8006
- RDP: 127.0.0.1:3389 (`Docker` / `admin`)
- Installer on guest Shared (`Z:\RazerCortexInstaller.exe`)
- After install, copy `Cortex.exe` helpers into `~/VMs/gamefocus-windows/shared/extracted/` for REA on the host.

## Post-install observations (2026-10-20)

Installed via `dockur/windows` Tiny10 lab (`~/VMs/gamefocus-windows`, `ADAPTER=e1000`). Product bits land at:

`C:\Program Files (x86)\Razer\Razer Cortex\`

### Key binaries (host copy under `~/VMs/gamefocus-windows/shared/extracted/bin/` — **not** committed to git)

| File | Role (from names + strings) |
|------|-----------------------------|
| `RazerCortex.exe` / `.Shell.exe` | Main UI shell |
| `RazerCortexBoostHelper.exe` | Boost helper process |
| `Boost.dll` | Core Game Booster engine |
| `RazerCortex.GameBoost.dll` | Managed Game Boost layer |
| `RazerCortex.SystemOptimize.dll` / `SystemClean.dll` / `SystemSetting.dll` | System tweak modules |
| `RazerCortex.GameOverlay.dll` / `GameClip.dll` / `FPSRunner.exe` | Overlay / clips / FPS — **won't copy** |
| AWS Kinesis SDK (in install dir) | Telemetry — **won't copy** |

### `Boost.dll` behavioral checklist (`CBoostManage::StartBoost` / `Restore`)

| Module | Bucket for gamefocus |
|--------|----------------------|
| Process Boost / Restore | **Focus (A)** — process park/restore |
| BackgroundApps Boost | **Focus (A)** |
| Service Boost | **out of scope** by default |
| Power Boost | **Boost (B)** — power plan |
| Ram Boost (`EmptyWorkingSet`) | **Boost (B)** — background only |
| Clipboard / Desktop / Cortana / Telemetry disable | **won't copy** / optional later |
| Key / WinKey disabler | **won't copy** |
| CPU unpark / affinity | **Boost (B)** (Linux: GameMode-ish) |

Whitelist language appears in `Boost.dll` (`WhiteList` / `Whitelist`). Full string dump: `~/VMs/gamefocus-windows/shared/extracted/strings-inventory.md`.

### Installer note

Cortex is downloaded by the Razer Gaming Software hub (~426 MB), not embedded in `RazerCortexInstaller.exe`.
