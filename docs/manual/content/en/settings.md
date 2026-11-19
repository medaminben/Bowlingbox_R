---
title: Settings
order: 4
summary: What the game remembers and where it keeps it.
---

## What is remembered

The game remembers three sound settings for the next start: mute, volume and the background loop. Change
them with the keys M, + and -, and B, or in the **Sound** menu. The theme is not remembered; the game
starts in Neon Hall unless you use `--theme`. The window size is not remembered either.

## Starting options

- `bowlingbox --theme=classic` starts in a theme (`neon`, `classic` or `outdoor`, in any letter case). An
  unknown name prints a warning in the terminal and starts Neon Hall.
- `bowlingbox --mute` starts without sound. That mutes only this session and does not change the saved mute
  setting, unless you switch mute or the volume yourself during the session.
- `bowlingbox --debug-view` shows the lane from above instead of the 3D view.

## Where it is saved

The settings are saved about every 30 seconds and when you close the window, in the file `app.ron` in
this folder:

| System | Folder |
| --- | --- |
| Linux | `~/.local/share/bowlingbox` (or `$XDG_DATA_HOME/bowlingbox` when that is set) |
| Windows | `%APPDATA%\Bowlingbox\data` |
| macOS | `~/Library/Application Support/Bowlingbox` |

Delete the file to go back to the defaults (volume 80 %, not muted, background loop on). A value in the file
that the game cannot read is replaced by its default, and a volume outside 0 to 100 % is cut to that range.

## Graphics

`BOWLINGBOX_RENDER=basic bowlingbox` forces the simpler picture without shadows, reflection and glow.
`BOWLINGBOX_RENDER=full`, or no value, uses the full picture where your graphics card supports it. See
[Troubleshooting](troubleshooting.md) for when to use it.

Go back to [Playing](usage.md) or to the [Welcome](index.md) page.
