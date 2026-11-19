---
title: Playing
order: 2
summary: Start the game.
---

## Starting the game

Start Bowlingbox from its entry in your applications menu, or run `bowlingbox` in a terminal.

## Where the game stands

The window shows the lane in 3D from behind the foul line, with a cyan guide for your aim. The camera
follows the ball down the lane and returns when the throw is over. Above the lane are the scoreboard with
ten frames and a power bar. The bottom line shows the phase of the throw, your score, the pins of the last
throw and the theme. The sound arrives in a later version.

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
| Esc, Ctrl+Q | Quit |

M, B, + and - are reserved for the sound and do nothing yet.

## Menus

- **Game, New Game** starts over.
- **Game, Quit** closes the window. So do Esc and Ctrl+Q. While the About window is open, either key
  closes it first. While a menu is open, Esc closes the menu and Ctrl+Q does nothing.
- **View, Theme** picks the theme; the current one is marked.
- **Help, About** shows the version.

For how a game is scored, see [Scoring rules](scoring.md). If something does not work, see
[Troubleshooting](troubleshooting.md).
