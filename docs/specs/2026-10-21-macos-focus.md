# macOS Focus

```text
Goal:
  On macOS, Focus suspends parked processes and resumes them when the
  session ends. Boost has no provider on macOS yet.

In:
  ps listing (pid and command name), SIGSTOP / SIGCONT, SIGTERM for soft-stop.
  The controller takes ProcessOps so tests record the pid without signals.

Out:
  A macOS Boost provider, App Nap toggles, service killing, overlays.

Behavior:
  parse_ps reads `ps -ax -o pid=,ucomm=` lines. A non-numeric first field is skipped.
  suspend and resume go through ProcessOps.
  The real backend calls kill only under cfg(target_os = "macos").
  CLI and tray keep an empty booster list on macOS. Ending Boost is a no-op.

Constraints:
  nix is the same 0.29 signal dependency the Linux crate already uses,
  and only on cfg(target_os = "macos").
  ucomm is the short command name (macOS truncates it).

Done when:
  cargo test -p gamefocus-macos is green on Linux.
  The kill calls compile only under cfg(target_os = "macos").
```
