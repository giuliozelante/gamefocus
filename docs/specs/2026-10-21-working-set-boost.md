# Windows working-set Boost after Focus

```text
Goal:
  Focus runs before Boost. On Windows, working_set trims the working set
  of processes Focus just suspended. Rollback is a no-op.

In:
  BoostContext with suspended process list.
  BoostProvider::apply takes &BoostContext.
  WorkingSetBoost + WorkingSetOps (EmptyWorkingSet under cfg(windows)).
  windows-gaming.toml lists power_plan then working_set.
  CLI and tray register both Windows boosters.

Out:
  Trimming the game process, service killing, non-Windows EmptyWorkingSet.

Behavior:
  start_session: Focus, then Boost.
  working_set soft-fails per pid and still returns a handle.
  payload is a comma-separated list of trimmed pids (or empty).

Done when:
  cargo test -p gamefocus-core -p gamefocus-windows is green on Linux.
  EmptyWorkingSet compiles only under cfg(windows).
```
