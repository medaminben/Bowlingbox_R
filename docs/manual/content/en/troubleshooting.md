---
title: Troubleshooting
order: 5
summary: Problems you may meet and what to do about them.
---

## The program does not start

Check that you installed the version for your system. On Linux, the installer package lists the libraries the program needs. Install them and start the program again.

## The window opens but stays empty

Update your graphics driver and start the program again.

## The picture has no shadows, reflection or glow

Bowlingbox uses a simpler picture when your graphics card cannot draw the full one. Start it from a terminal: one line that starts with `render:` says why. Updating your graphics driver may help.

To check whether the simpler picture fixes a problem, start the program with `BOWLINGBOX_RENDER=basic bowlingbox`. `BOWLINGBOX_RENDER=full`, or no value, uses the full picture where your graphics card supports it.

## There is no sound

Start the program from a terminal. If one line starts with `sound disabled:`, the game could not open a sound device and runs without sound. Check that your system has a working audio output (on Linux, a PulseAudio or PipeWire server, or ALSA) and start the program again. Check also that the sound is not muted (the bottom line shows `sound muted`; press M). The game remembers a muted state; see [Playing](usage.md) for where it is saved. On Linux, building from source needs the package `libasound2-dev`.

## The settings are not remembered

The game saves mute, volume and the background loop when you close the window and about every 30 seconds.
If the game is stopped by force, the last change may be lost. If the folder cannot be written, nothing is
saved. See [Settings](settings.md) for the folder. After `--mute` the saved mute setting stays as it was.

## The Controls or About window is open and the keys do nothing

That is how it works: while one of the two windows is open, the game keys do not reach the lane. Press Esc
or the Close button. A second Esc quits the game.

## The game runs slowly

Start the program with `BOWLINGBOX_RENDER_STATS=1 bowlingbox` in a terminal. About once a second it prints how many milliseconds the graphics card spends on each part of the picture. Add that output when you report the problem. The aim is 60 frames per second at 1920x1080 on an integrated graphics card; if you get less, say which card you have. If your graphics card cannot measure this, the program says so once.

## Reporting a problem

Send the version number and the installer you used, together with the steps that led to the problem.

Go back to [Playing](usage.md) or to the [Welcome](index.md) page.
