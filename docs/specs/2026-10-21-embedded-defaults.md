# Embedded default profiles (zero-config start)

```text
Goal:
  First-time users can `gamefocus start` or tray Start without a profile path.

In:
  Resolve order: --profile → user default.toml → embedded OS default.
  profiles/embedded/{linux,windows,macos}.toml (safe park lists).
  include_str! loaders in gamefocus-core.
  CLI --profile optional. Tray uses resolved default.
  Release archives include examples/ copies of bundled + embedded TOMLs.
  README quickstart; docs/PROFILES.md for customization.

Out:
  Setup wizard. Native installers. Aggressive park presets.

Done when:
  cargo test covers explicit / user-config / embedded resolve.
  `gamefocus start` with no --profile works on a clean machine.
```
