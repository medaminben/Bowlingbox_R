---
title: Troubleshooting
order: 4
summary: Problems you may meet and what to do about them.
---

## The program does not start

Check that you installed the version for your system. On Linux, the installer package lists the libraries the program needs. Install them and start the program again.

## The window opens but stays empty

Update your graphics driver and start the program again.

## The picture has no shadows, reflection or glow

Bowlingbox uses a simpler picture when your graphics card cannot draw the full one. Start it from a terminal: one line that starts with `render:` says why. Updating your graphics driver may help.

To check whether the simpler picture fixes a problem, start the program with `BOWLINGBOX_RENDER=basic bowlingbox`. `BOWLINGBOX_RENDER=full`, or no value, uses the full picture where your graphics card supports it.

## The game runs slowly

Start the program with `BOWLINGBOX_RENDER_STATS=1 bowlingbox` in a terminal. About once a second it prints how many milliseconds the graphics card spends on each part of the picture. Add that output when you report the problem. If your graphics card cannot measure this, the program says so once.

## Reporting a problem

Send the version number and the installer you used, together with the steps that led to the problem.

Go back to [Playing](usage.md) or to the [Welcome](index.md) page.
