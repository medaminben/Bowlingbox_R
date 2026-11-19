---
title: Scoring rules
order: 3
summary: How a game of ten-pin bowling is scored.
---

## Scoring rules

Bowlingbox scores standard ten-pin bowling:

- A game has 10 frames. In each frame you get up to two balls to knock down 10 pins.
- **Strike** (`X`): all 10 pins with the first ball. The frame scores 10 plus the next two balls.
- **Spare** (`/`): all 10 pins with two balls. The frame scores 10 plus the next ball.
- **Open frame**: the frame scores the number of pins knocked down.
- **Miss** (`-`): a ball that knocks down no pins.
- **Tenth frame**: a strike earns two bonus balls and a spare one, so the frame has up to three
  balls. The rack is set up again whenever it is cleared; otherwise the next ball faces the pins
  still standing.
- A ball can never knock down more pins than are standing. The calculator refuses such input.
- A perfect game is 12 strikes = 300.

On the scoreboard, the top row shows each ball and the bottom row shows the running total. A
frame's total appears once its bonus balls have been played.

Go back to [Playing](usage.md) or to the [Welcome](index.md) page.
