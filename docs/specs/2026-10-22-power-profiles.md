# Linux power-profiles-daemon Boost (#58)

Goal:
  Soft-fail Boost provider `power_profiles` that sets `performance` via
  `powerprofilesctl` and restores the previous profile on rollback.

In / out:
  In: provider + mockable ctl; wire into CLI/tray; profile providers list.
  Out: tuned/power-saver auto heuristics; GNOME-only paths.

Done when:
  Unit tests with mock ctl; cargo test -p gamefocus-linux green.
