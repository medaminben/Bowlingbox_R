# Bowlingbox architecture

Bowlingbox keeps its rules in small crates that need no window, GPU or audio device, and a thin app that
wires them to an egui window (eframe with the wgpu backend).

Planned crates: `score` exists now. `lane` (the throw and the physics seam), `render` (themes, meshes
and the wgpu renderer) and `audio` (the sound catalogue and the output seam) come in later milestones.
`lane` will use `score`; `render` and `audio` use neither `lane` nor each other. Only the app converts
between their types.

## Layout

<!-- bk:layout -->
| Path | Purpose |
| --- | --- |
| [`crates/score`](crates/score) | `bowlingbox-score`: the ten-pin scoring rules. |
| [`apps/bowlingbox`](apps/bowlingbox) | `bowlingbox`: the game. |

Libraries live in `crates/` and hold the logic. Apps live in `apps/` and only wire a library to an interface: a command line or a window. An app depends on a library, never the other way round, so each library can be tested without an interface.

## The score API

`bowlingbox_score` has no dependencies. Its public items:

- `Slot`: what one roll position shows (`NotPlayed`, `Empty`, `Strike`, `Spare`, `Pins(n)`).
  `notation` and `total_notation` turn slots and totals into scoreboard text.
- `ScoreSheet`: read access to a game (`rolls`, `frame_totals`, `score`, `frame_score`, `pins_left`,
  `is_over`).
- `Game`: a `ScoreSheet` that accepts input (`roll`, `reset`). `roll` returns `RollError::GameOver`
  or `RollError::TooManyPins` instead of accepting a bad roll.
- `ScoreCalculator`: the implementation of both traits.
- `frames` and `frames_of`: a ten-frame view (`FrameView`) for drawing a scoreboard.

The constants `FRAMES` (10), `PINS` (10) and `ROLL_SLOTS` (21) describe the sheet.

## Dependencies

Write down which crate depends on which, and why, when a dependency is not obvious from the table.

## Decisions

Record a decision here when you make one: what you chose, what you rejected, and what would make you revisit it.
