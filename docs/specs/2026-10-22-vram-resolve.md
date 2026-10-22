# VRAM PID → supervisor resolve (issue #50)

Goal:
  Preflight WARN names Docker/Podman/systemd-user targets when resolvable from cgroup.

In / out:
  In: cgroup parse + optional inspect; wire into VRAM warn; tests.
  Out: auto-stop, prompt, hooks, Windows/macOS.

Done when:
  Mock cgroup unit tests; soft-fail keeps PID warn; CI green.
