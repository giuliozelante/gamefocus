# Keep tray icon alive (#62)

Goal:
  Persist the Tauri TrayIcon for the app lifetime so KDE/Linux SNI shows it.

In / out:
  In: app.manage(tray) after TrayIconBuilder::build.
  Out: new icon assets; StatusNotifier rewrite.

Done when:
  gamefocus-ui shows a tray entry on KDE Wayland after launch.
