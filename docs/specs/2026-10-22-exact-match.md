# Exact basename park matching (issue #42)

Goal:
  `process_name` matches exact basename (case-insensitive); optional `*` globs.

In / out:
  In: matcher + tests + PROFILES/CHANGELOG note.
  Out: VRAM probe, madvise, hooks, exe_path/cmdline policy change.

Behavior:
  - `chrome` ≠ `chrome_crashpad`
  - `chrome` = `Chrome` (case-insensitive)
  - `code-*` matches `code-helper`
  - whitelist `process_name` uses the same rules
  - `exe_path` / `cmdline` stay substring contains

Done when:
  Unit tests green; `cargo test -p gamefocus-core`.
