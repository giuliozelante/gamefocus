# Windows Focus and power-plan Boost

```text
Goal:
  On Windows, Focus suspends parked processes and Boost switches to the
  High Performance power plan. Ending the session resumes processes and
  restores the previous scheme.

In:
  Scheme parsing, PowerPlanBoost, tasklist CSV parsing, NtSuspendProcess /
  NtResumeProcess, taskkill without /F for soft-stop.
  CLI registers power_plan on Windows.

Out:
  Service killing, overlays, working-set scrub of the game process.

Behavior:
  apply stores the active scheme GUID and activates
  8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c unless it is already active.
  rollback restores the stored GUID.
  suspend and resume go through ProcessOps so tests can record the pid.

Done when:
  cargo test -p gamefocus-windows is green on Linux.
  The Win32 calls compile only under cfg(windows).
```
