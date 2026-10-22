# Tray VRAM reclaim prompt (follow-up to #52)

Goal:
  Wire soft-stop/restore into tray Start/Stop gaming with a native Yes/No dialog.

In / out:
  In: run_start reclaim + dialog; run_end restore; warn_heavy_vram_users.
  Out: remember checkbox; hooks; custom webview modal.

Done when:
  Manual tray path works; cargo test core green; UI builds.
