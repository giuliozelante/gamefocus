# VRAM reclaim soft-stop/restore (issue #52)

Goal:
  Soft-stop resolvable Docker/Podman/systemd-user VRAM hogs for a session
  and restore them on end, behind warn/prompt/auto policy.

In / out:
  In: profile [gpu.vram_reclaim]; SupervisorOps; snapshot reclaimed[];
     CLI TTY prompt + --reclaim-vram; restore on end.
  Out: tray modal; proactive [hooks]; Windows/macOS; shell exec hooks.

Done when:
  Unit tests for pattern match, consent, mock stop/start; cargo test green.
