# gamefocus run wrapper

Goal:
  `gamefocus run [--reclaim-vram] [--] <cmd>...` starts a session, execs the
  child, ends the session when the child exits.

In / out:
  In: CLI subcommand; reuse start/end paths; forward exit code.
  Out: dry-run for run; tray equivalent; Windows service quirks.

Done when:
  Short-command test green; no leftover snapshot; cargo test green.
