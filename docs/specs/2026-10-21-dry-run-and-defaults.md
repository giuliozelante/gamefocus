# Dry-run park list and safe Boost default

```text
Goal:
  Dry-run prints park matches. BoostPolicy::default().providers is empty.

In:
  park_targets helper shared by start_session and the CLI dry-run.
  BoostPolicy Default change.
  README Modes row for Windows Boost.

Out:
  New Boost providers.

Done when:
  cargo test -p gamefocus-core --lib is green.
```
