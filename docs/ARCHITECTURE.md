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
| [`crates/lane`](crates/lane) | `bowlingbox-lane`: the lane types, the physics trait and the throw state machine. |

Libraries live in `crates/` and hold the logic. Apps live in `apps/` and only wire a library to an interface: a command line or a window. An app depends on a library, never the other way round, so each library can be tested without an interface.

## The score API

`bowlingbox_score` has no dependencies. It exports:

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

- `bowlingbox-score` has no dependencies, so it builds and tests without a window or a GPU.
- `bowlingbox` (the app) uses `bowlingbox-score` for the rules and `eframe` for the window. `eframe` runs
  with the wgpu renderer and default features off; only `wgpu`, `default_fonts`, `wayland`, `x11` and
  `accesskit` are on.
- `buildkit` is a build dependency of the app. It supplies the version string that `--version` and the
  About window show.
- `lane` will depend on `score`. `render` and `audio` will depend on neither `lane` nor each other.

## Decisions

- **Rust port of the 3D game only.** The C++ console and display apps, and the `Core` library, stay in the
  C++ repository. Rust's `Result` replaces `Core`.
- **Score as its own crate.** The rules need no window, GPU or audio device, so they are tested without one.
- **eframe with the wgpu backend.** We write our own renderer instead of using Bevy. Bevy would save
  rendering code but would impose its architecture, slow the builds and break about every three months.
- **Rapier for physics and kira for audio.** Both arrive in later milestones. The throw is retuned, because
  Rapier does not reproduce the C++ numbers.
- **Separate repository, Beta flow.** `Bowlingbox_R` is public from its first commit. Features merge into
  `Beta`, which releases `v2.0.0-beta.N`. Promoting `Beta` to `main` releases `v2.0.0`.
