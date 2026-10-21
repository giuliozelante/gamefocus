# Windows CPU affinity Boost

```text
Goal:
  After Focus, pin suspended processes to CPU 0 (mask 1). Rollback restores
  each previous mask from the handle payload.

In:
  CpuAffinityBoost + AffinityOps.
  GetProcessAffinityMask / SetProcessAffinityMask under cfg(windows).
  Payload format: pid:mask comma-separated hex masks.
  windows-gaming.toml adds cpu_affinity.
  CLI and tray register the provider.

Out:
  Per-profile affinity masks. Touching the game process.

Done when:
  cargo test -p gamefocus-windows --lib is green on Linux.
```
