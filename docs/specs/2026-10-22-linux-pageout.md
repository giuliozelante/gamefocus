# Linux process_madvise pageout after Focus (issue #46)

Goal:
  After SIGSTOP, soft-fail `process_madvise(MADV_PAGEOUT)` on anonymous RW VMAs so physical RAM can free.

In / out:
  In: gamefocus-linux pageout helper; call from suspend.
  Out: hard failures; Windows/macOS; cgroup freeze.

Done when:
  Maps parser unit test; cargo test -p gamefocus-linux; unsupported kernels soft-fail.
