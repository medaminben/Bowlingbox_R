---
title: Playing
order: 2
summary: Start the game.
---

## Starting the game

Start Bowlingbox from its entry in your applications menu, or run `bowlingbox` in a terminal.

## Where the game stands

The window shows the lane in 3D from behind the foul line, with a cyan guide for your aim. The camera
follows the ball down the lane and returns when the throw is over. Above the lane is the scoreboard with
ten frames and the total. Below it are the power bar, a hint for the current phase and the result of the
last throw (`STRIKE!`, `SPARE!`, `Gutter` or the number of pins). The bottom line shows the phase of the
throw, your score, the pins standing, the theme and the sound (`volume 80 %` or `sound muted`). All of it
scales with the window.

## Themes

Three looks are built in: Neon Hall (the default), Classic Alley and Outdoor. Press T to switch to the
next one, or pick one under **View, Theme**. The theme changes only the look, never the game. To start in
a theme, run `bowlingbox --theme=classic` (`neon`, `classic` or `outdoor`, in any letter case). An unknown
name prints a warning in the terminal and starts Neon Hall. `bowlingbox --debug-view` shows the lane from
above instead, which helps to check the physics.

## Throwing

1. Move the ball with Left and Right and turn the aim with A and D.
2. Hold Space. The power bar swings between empty and full.
3. Release Space to throw.
4. Wait for the pins to settle. The next throw starts on its own, and the game ends after the tenth frame.

For a strike, press Right three or four times from the centre, leave the aim straight and release at
about 80 % power. Each press moves the ball 2 cm. One press of A or D usually costs the strike. The
ball keeps its place and aim for the next throw; a new game puts it back in the centre.

With the mouse, move it over the lane to shift the ball, drag with the right button to turn the aim,
and hold the left button to charge and release it to throw.

| Key | Action |
| --- | --- |
| Left, Right | Move the ball |
| A, D | Turn the aim |
| Space | Hold to charge, release to throw |
| N | New game |
| T | Next theme |
| M | Mute or unmute |
| + or =, - | Louder, quieter (the keypad keys too) |
| B | Background ambience on or off |
| Esc, Ctrl+Q | Quit |

## Sound

The game plays the rolling ball, the pins, a tone while you charge, a sound for strikes and spares, and a
background loop for each theme. M mutes. + and - change the volume in steps of 10 %, and changing the volume
also unmutes. B switches the background loop on or off; the other sounds stay on. The same choices are in the
**Sound** menu. The game remembers
mute, volume and the background loop for the next start; the theme is not remembered. To start without sound,
run `bowlingbox --mute`. That mutes only this session and does not change the saved mute setting, unless you
switch mute or the volume yourself during the session.

The settings are saved about every 30 seconds and when you close the window, in the file `app.ron` in
this folder:

| System | Folder |
| --- | --- |
| Linux | `~/.local/share/bowlingbox` (or `$XDG_DATA_HOME/bowlingbox` when that is set) |
| Windows | `%APPDATA%\Bowlingbox\data` |
| macOS | `~/Library/Application Support/Bowlingbox` |

Delete the file to go back to the defaults (volume 80 %, not muted, background loop on). A value in the file
that the game cannot read is replaced by its default, and a volume outside 0 to 100 % is cut to that range.

If no sound device can be opened, the game runs without sound and prints one line that starts with
`sound disabled:` in the terminal.

## Menus

- **Game, New Game** (Ctrl+N) starts over.
- **Game, Quit** (Ctrl+Q) closes the window. So does Esc. While the Controls or About window is open,
  Esc closes it first and Ctrl+Q still quits. While a menu is open, Esc closes the menu and Ctrl+Q does
  nothing.
- **View, Theme** picks the theme; the current one is marked.
- **Sound** has Mute, Volume up, Volume down and Background ambience; the first and last are checked while on.
  Volume up is greyed out at 100 % and Volume down at 0 %.
- **Help, Controls** lists every key and mouse action. **Help, About** shows the version.

While the Controls or About window is open, the lane keeps running but the game keys (move, aim, throw,
new game, theme and sound) do nothing until you close it.

## For contributors

`bowlingbox --screen=<name>` opens the window in a named state without any input, so a screenshot tool can
capture it: `game` (the default), `controls`, `about`, `sound-menu` (the Sound menu shown open),
`theme-classic` and `theme-outdoor`. It changes nothing else. An unknown name prints
`unknown screen '<name>'; use game, controls, about, sound-menu, theme-classic or theme-outdoor` and the game
starts normally. A theme screen wins over `--theme`.

For how a game is scored, see [Scoring rules](scoring.md). If something does not work, see
[Troubleshooting](troubleshooting.md).
