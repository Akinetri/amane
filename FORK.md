# What this fork changes

A fork of [MystiaFin/amane](https://github.com/MystiaFin/amane). Everything below is on top of upstream `main` (`544cbfc`). It's the library behind [Akinetri/suzuha](https://github.com/Akinetri/suzuha).

## Added

- **Text paste** in `TextInput` with Ctrl+V, Ctrl+Shift+V and Shift+Insert. It reads the Wayland clipboard and inserts the text at the cursor in one go, so `on_change` fires once. The V key is matched by its key code, so it works in any keyboard layout. Line breaks become spaces, other control characters are dropped, and a clipboard owner that doesn't answer in 500 ms is skipped.
- **`Method::sender()`**: who called a D-Bus method. A tray icon often registers with only its object path, so the tray has to know the caller.
- **`Apps::icon_path(name)`**: the file for an icon name from the user's icon theme, like `"firefox"`. The icon folders are read once and cached.
- **Audio devices**: `Audio::outputs()`, `inputs()`, `set_output()`, `set_input()`, and `AudioDevice` with `name()`, `description()`, `icon()`, `default()`
- **`Bus::emit_to`**: a D-Bus signal sent to one program instead of everyone

## Changed

- **Notifications**: the action and close signals go only to the app that sent the notification. Ids start over with every shell, so an app could still hold an id from an earlier run and take another app's click for its own.

## Fixed

- **Audio**: the PulseAudio context is dropped before its mainloop, which used to abort the process on exit

The docs (`docs/src/audio.md`, `dbus.md`, `text_input.md`, `cheatsheet.md`) cover the new calls.
