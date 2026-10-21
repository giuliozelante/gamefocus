# Bundled Windows and macOS profiles

```text
Goal:
  Ship a gaming profile for Windows and one for macOS. An empty Boost
  provider list applies nothing.

In:
  profiles/bundled/windows-gaming.toml (park common browsers and Telegram,
  whitelist Steam and Discord, provider power_plan).
  profiles/bundled/macos-gaming.toml (same park idea, providers empty).
  start_session skips every booster when providers is empty.

Out:
  Personal home-directory paths. New Boost providers. Editing linux-gaming.toml.

Behavior:
  protect_shell stays on. Focus action is suspend.
  Discovery lists both new files next to linux-gaming.toml.
  A named provider still runs when it is listed. An empty list runs none.

Done when:
  cargo test -p gamefocus-core is green.
```
