# VRAM preflight warn (issue #44)

Goal:
  Warn before start/dry-run when a non-whitelisted process holds >500 MiB GPU memory.

In / out:
  In: parse/probe helpers in core; CLI logs WARNs; docs/CHANGELOG.
  Out: hard block; AMD/Intel; tray modal; auto docker stop.

Behavior:
  - Probe via `nvidia-smi --query-compute-apps=pid,process_name,used_gpu_memory --format=csv,noheader,nounits` (soft-fail if missing).
  - Threshold 500 MiB; skip profile whitelist process_name exact/glob matches (basename of process_name path).
  - Session still starts after warnings.

Done when:
  Unit tests for CSV parse + filter; live dry-run does not crash without NVIDIA.
