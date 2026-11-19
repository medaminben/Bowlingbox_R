# Bowlingbox architecture

Bowlingbox keeps its rules in small crates that need no window, GPU or audio device, and a thin app that
wires them to an egui window (eframe with the wgpu backend).

Crates: `score` (the scoring rules), `lane` (the throw and the physics seam) and `render` (the
GPU-free themes, decor, meshes, lighting maths and camera; the wgpu renderer follows) exist now. `audio`
(the sound catalogue and the output seam) comes in a later milestone. `lane` uses `score`; `render` and `audio` will use neither `lane` nor each other.
Only the app converts between their types.

## Layout

<!-- bk:layout -->
| Path | Purpose |
| --- | --- |
| [`crates/score`](crates/score) | `bowlingbox-score`: the ten-pin scoring rules. |
| [`apps/bowlingbox`](apps/bowlingbox) | `bowlingbox`: the game. |
| [`crates/lane`](crates/lane) | `bowlingbox-lane`: the lane types, the physics trait and the throw state machine. |
| [`crates/render`](crates/render) | `bowlingbox-render`: GPU-free render data and maths: themes, decor layout, meshes, lighting maths, camera and scene description. |

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

## The lane API

`bowlingbox_lane` depends on `bowlingbox-score` and `rapier3d`. Rapier types stay inside the crate.
It exports:

- The types: `Phase` (`Aiming`, `Charging`, `Rolling`, `Settling`, `FrameDone`, `GameOver`), `Pose`,
  `Snapshot` (what the lane shows right now), `Body` and `Impact`, and the dimensions `LANE`
  (`LaneDimensions`), `MAX_AIM_OFFSET`, `MAX_AIM_ANGLE_DEG` and `BALL_START_Z`.
- `PhysicsWorld`: the rigid-body world as a trait. It knows no bowling rules and speaks arrays, so
  `Lane` is tested with a scripted fake.
- `Lane<W, G>`: the throw state machine, ported from C++ `Lane_impl.cpp`. It drives a `PhysicsWorld`
  and a `bowlingbox_score::Game`. The commands (`move_aim`, `rotate_aim`, `start_charge`, `release`,
  `new_game`) return `LaneError::WrongPhase` and change nothing when the phase does not allow them.
  `step(dt)` advances the timers and the world; `snapshot`, `score_sheet` and `drain_impacts` read it.
- `RapierWorld`: the `PhysicsWorld` on Rapier. It runs fixed sub-steps of 1/120 s, at most four per
  `step` call. `Lane::default()` is the game: a `RapierWorld` and a `ScoreCalculator`.
- The tuned throw: `POCKET_OFFSET`, `POCKET_ANGLE_DEG` and `POCKET_POWER` describe a throw that usually
  strikes. The tests and the manual use them as the one source.
- `PIN_MESH_OFFSET_Y`: what the renderer adds to a pin pose, along the pin's own up axis, to place
  the C++ pin mesh. The pin collider ends 2 cm above the mesh base, and the pins spawn resting on the
  deck.

Impacts are reported once per pair of bodies, on their first touch. A pair that stays in contact does
not report again until the bodies separate. Contacts below 0.3 N·s are dropped, and so are pin contacts
before the ball nears the rack. The app must drain them every frame: the buffer holds 256.

### Conventions

- Units are metres, seconds and kilograms. Angles are in degrees where a name ends in `_deg`.
- The origin is on the foul line in the middle of the lane, with the deck surface at `y = 0`. +z runs
  down the lane from the foul line towards the pins, +y is up and +x is the bowler's right. Right and D
  move and turn the aim towards +x.
- +x is drawn on the right of the screen, so Right moves the ball right. The debug view does this, and
  the 3D camera must too. A right-handed look-at down +z, as in the C++ game, puts +x on the left. The
  `render` camera is left-handed (glam's `lh` look-at and perspective), so +x projects right of the
  screen centre with no conversion; a test covers it. Meshes keep the C++ winding, which appears
  clockwise on screen: use `FrontFace::Cw`.
- Rotations are unit quaternions in the order (x, y, z, w).
- `Pose.position` is the rigid-body origin. For the ball that is its centre. For a pin it is the point
  on its axis `LANE.pin_origin_height` (0.12 m) above its nominal base. That point is not the centre of
  mass, which Rapier computes from the collider shape.

## Dependencies

- `bowlingbox-score` has no dependencies, so it builds and tests without a window or a GPU.
- `bowlingbox` (the app) uses `bowlingbox-lane` for the throw and `bowlingbox-score` for the sheet, and
  `eframe` for the window. `eframe` runs with the wgpu renderer and default features off; only `wgpu`,
  `default_fonts`, `wayland`, `x11` and `accesskit` are on. The app also names `winit` to switch on
  `wayland-csd-adwaita`, so GNOME on Wayland gets a title bar.
- `buildkit` is a build dependency of the app. It supplies the version string that `--version` and the
  About window show.
- `bowlingbox-lane` depends on `bowlingbox-score` and `rapier3d`. `render` and `audio` will depend on
  neither `lane` nor each other.
- The dev profile builds `rapier3d` and `parry3d` with `opt-level = 3`, so a debug build simulates in
  real time.

## Decisions

- **Rust port of the 3D game only.** The C++ console and display apps, and the `Core` library, stay in the
  C++ repository. Rust's `Result` replaces `Core`.
- **Score as its own crate.** The rules need no window, GPU or audio device, so they are tested without one.
- **eframe with the wgpu backend.** We write our own renderer instead of using Bevy. Bevy would save
  rendering code but would impose its architecture, slow the builds and break about every three months.
- **Rapier for physics and kira for audio.** Rapier came with the lane; kira comes with the sound. The
  throw is retuned, because Rapier does not reproduce the C++ numbers.
- **Separate repository, Beta flow.** `Bowlingbox_R` is public from its first commit. Features merge into
  `Beta`, which releases `v2.0.0-beta.N`. Promoting `Beta` to `main` releases `v2.0.0`.
