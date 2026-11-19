# Changelog

## 2.0.0

Bowlingbox 2.0.0 is the Rust port of the 3D ten-pin bowling game. It succeeds
[Bowlingbox 1.x](https://github.com/medaminben/Bowlingbox), the C++ version.

### For players

- Three themes: Neon Hall (the default), Classic Alley and Outdoor. Press T or use View, Theme.
  `--theme=classic` starts in one.
- Sound: rolling ball, pin knocks, strike and spare cues, and an ambience per theme. M mutes, + and -
  change the volume, B switches the ambience. The Sound menu does the same, and `--mute` starts silent.
- Mute, volume and ambience are saved in `app.ron` in the game's data folder
  (`~/.local/share/bowlingbox` on Linux, `%APPDATA%\Bowlingbox\data` on Windows). The theme and window
  size are not saved.
- Two render paths. The full path has shadows, lane reflection and bloom. The basic path drops them and
  is used when the graphics card cannot do the full one. `BOWLINGBOX_RENDER=basic` forces the basic path.
- The full path measured, at 1920x1080 on an Intel UHD (Comet Lake GT2) on Linux, a median of 71 frames
  per second in Neon Hall, 72 in Classic Alley and 90 in Outdoor. Neon Hall's slowest second was 63.
- Menu bar (Game, View, Sound, Help) with Controls and About windows.

### For contributors

- `bowlingbox --screen=<name>` opens a named screen: `game`, `controls`, `about`, `sound-menu`,
  `theme-classic` or `theme-outdoor`. `--capture=<path>` writes a PNG of it and exits.
- `cargo bk manual capture` runs those screens and writes the manual's screenshots to
  `docs/manual/assets/shots/en/`. Manual pages reference them as `![caption](shot:name)`.
- On Linux the build needs `libasound2-dev`.

### Known limitations

- Not yet checked on real hardware: how the sound actually sounds, 60 fps on Windows, the whole
  checklist on Windows, Esc and Ctrl+Q in a real window, and settings surviving a restart. See
  [docs/ACCEPTANCE.md](docs/ACCEPTANCE.md).
- The aim guide blends slightly differently from 1.x: its cyan is up to about 16 of 255 levels less
  bright.
