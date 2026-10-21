# CLI profiles and path resolve

```text
Goal:
  List bundled profiles and resolve relative profile names. Default
  session snapshot uses the core XDG/runtime path.

In:
  gamefocus profiles
  start/end/status use resolve_profile_path and default_snapshot_path
  when the user omits --snapshot or passes a relative profile name.

Out:
  New profile formats. Tray changes.

Done when:
  cargo test -p gamefocus-core stays green.
  cargo run -p gamefocus-cli -- profiles prints the three bundled files.
```
