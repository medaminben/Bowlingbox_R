# Bowlingbox

Bowlingbox is a 3D ten-pin bowling game rebuilt in Rust. It follows up
[Bowlingbox 1.5.5](https://github.com/medaminben/Bowlingbox), the C++ version. The work is in progress
until v2.0.0. Today the window shows the lane in 3D with a following camera, with the scoreboard on top and a power bar, a hint and the throw result below it, so you
can play a full game with the keyboard. Three themes are built in, and the game plays its sounds.

## Build and run

```
cargo build
cargo test
cargo run -p bowlingbox
```

`cargo run -p bowlingbox -- --version` prints the version and exits. `--theme=neon|classic|outdoor` picks
the look at start (any letter case; an unknown name prints a warning and gives Neon Hall). `--debug-view`
draws the top-down physics view instead of the 3D view. `--mute` starts without sound (this session only). Sound settings are remembered between runs.
`--screen=<name>` opens a named screen for screenshots (see the manual).
The 3D view uses the full render path (shadows, lane reflection,
bloom) where the GPU supports it and the basic path otherwise; stderr then gets one `render: ...` line with the
reason. `BOWLINGBOX_RENDER=basic` forces the basic path, `full` or no value picks the full path where supported,
and any other value prints a warning. `BOWLINGBOX_RENDER_STATS=1` prints the GPU time of each pass about once a
second. In the window, Esc and Ctrl+Q quit. With the Controls or About window open, Esc closes it first and the game keys do
nothing; Ctrl+Q still quits. With a menu open, Esc closes the menu and Ctrl+Q does nothing.

Controls: Left and Right move the ball, A and D turn the aim, hold Space to charge the throw and release it to
throw, N starts a new game. With the mouse, move it over the lane to shift the ball, drag with the right button
to turn the aim, and hold the left button to charge and release to throw. T changes the theme. M mutes, + and - change the volume
(the keypad keys too), and B switches the background ambience; the Sound menu does the same. On Linux the build needs the ALSA headers (`libasound2-dev`). If no sound device opens, stderr gets one `sound disabled: ...` line and the game runs silent.

The menu and the HUD use the Rubik font (SIL Open Font License); its licence is in
[`packaging/NOTICES-extra.txt`](packaging/NOTICES-extra.txt).

`cargo bk version` prints the product version. `cargo bk` lists the other commands.

## Layout

<!-- bk:layout -->
| Path | Purpose |
| --- | --- |
| [`crates/score`](crates/score) | `bowlingbox-score`: the ten-pin scoring rules. |
| [`apps/bowlingbox`](apps/bowlingbox) | `bowlingbox`: the game. |
| [`crates/lane`](crates/lane) | `bowlingbox-lane`: the lane types, the physics trait and the throw state machine. |
| [`crates/render`](crates/render) | `bowlingbox-render`: themes, decor layout, meshes, lighting maths, camera, scene description and the wgpu scene renderer. |
| [`crates/audio`](crates/audio) | `bowlingbox-audio`: the 13 embedded sounds, the audio output trait, a kira output and a silent fallback. |

`cargo bk add-crate <name>` and `cargo bk add-app <name> --console|--gui` add a row to this table and to the one in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Your own text in either file is never rewritten.

## Documentation

- [Architecture](docs/ARCHITECTURE.md): how the crates and apps fit together.
- [User manual](docs/manual/content/en/index.md): the pages that ship with the app.
