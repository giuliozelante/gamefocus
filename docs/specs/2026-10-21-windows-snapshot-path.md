# Windows LocalAppData session path

```text
Goal:
  Prefer LOCALAPPDATA for the default session snapshot when XDG is unset.

In:
  resolve_snapshot_path helper; default_snapshot_path uses it.
  Unit test with a fake env lookup.

Out:
  Changing the XDG or HOME paths.

Done when:
  cargo test -p gamefocus-core --lib is green.
```
