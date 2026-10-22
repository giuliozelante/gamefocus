# Profiles

A profile is a TOML file that lists which processes Focus may park, what to whitelist, and which Boost providers to run.

Casual users can ignore this file. The CLI and tray load an **embedded OS default** when you do not pass `--profile` and have no user default.

### `process_name` matching

- Default: **exact** executable basename, case-insensitive (`chrome` matches `Chrome`, not `chrome_crashpad`).
- Optional simple globs with `*` (e.g. `code-*` matches `code-helper`).
- `exe_path` and `cmdline` still use substring contains.

## Resolve order

1. `--profile <path>` (or a tray selection)
2. User default: `$XDG_CONFIG_HOME/gamefocus/default.toml`, else `~/.config/gamefocus/default.toml`, else `%APPDATA%\gamefocus\default.toml` on Windows
3. Embedded compile-time default for the host OS (`profiles/embedded/`)

## Customize

```bash
mkdir -p ~/.config/gamefocus
cp examples/linux.toml ~/.config/gamefocus/default.toml   # from a release archive
# or:
cp profiles/embedded/linux.toml ~/.config/gamefocus/default.toml
```

Edit the park list, then run `gamefocus start` with no flags.

## Bundled examples

| Path | Role |
|------|------|
| `profiles/embedded/*.toml` | Safe defaults compiled into the binary |
| `profiles/bundled/*-gaming.toml` | Richer examples (may include personal paths; treat as templates) |

Release archives ship both under `examples/`.
