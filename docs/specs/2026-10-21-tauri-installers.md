# Tauri tray installers (issue #36)

Goal:
  Attach unsigned tray installers to `v*` GitHub Releases: Linux `.deb`, Windows NSIS `.exe`, macOS `.dmg`.

In / out:
  In: release.yml bundle-ui matrix; docs; CHANGELOG.
  Out: code signing / notarization; AppImage; changing CLI archives.

Behavior:
  On tag push, build CLI archives (existing) and UI bundles in parallel, then publish all assets.
  NSIS instead of MSI (no WiX dependency on runners). Unsigned; README notes the gap.

Constraints:
  Static frontend (`frontendDist`); `cargo tauri build --bundles …` via pinned `tauri-cli`.
  Skip red-green: workflow/docs only.

Done when:
  actionlint clean; PR into develop; release body lists installer artifacts.
