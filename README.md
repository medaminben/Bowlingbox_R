# Bowlingbox

Bowlingbox is a 3D ten-pin bowling game rebuilt in Rust. It follows up
[Bowlingbox 1.5.5](https://github.com/medaminben/Bowlingbox), the C++ version. The work is in progress
until v2.0.0. Today the window shows the lane from above, a scoreboard and a power bar, so you can play a full game
with the keyboard. The 3D view and the sound arrive in later milestones.

## Build and run

```
cargo build
cargo test
cargo run -p bowlingbox
```

`cargo run -p bowlingbox -- --version` prints the version and exits. In the window, Esc and Ctrl+Q quit. With the About window open, either key closes it first. With a menu open,
Esc closes the menu and Ctrl+Q does nothing.

Controls: Left and Right move the ball, A and D turn the aim, hold Space to charge the throw and release it to
throw, N starts a new game. With the mouse, move it over the lane to shift the ball, drag with the right button
to turn the aim, and hold the left button to charge and release to throw. T, M, B, + and - are reserved for the
theme and the sound and do nothing yet.

`cargo bk version` prints the product version. `cargo bk` lists the other commands.

## Layout

<!-- bk:layout -->
| Path | Purpose |
| --- | --- |
| [`crates/score`](crates/score) | `bowlingbox-score`: the ten-pin scoring rules. |
| [`apps/bowlingbox`](apps/bowlingbox) | `bowlingbox`: the game. |
| [`crates/lane`](crates/lane) | `bowlingbox-lane`: the lane types, the physics trait and the throw state machine. |
| [`crates/render`](crates/render) | `bowlingbox-render`: themes, decor layout, meshes, lighting maths, camera, scene description and the wgpu scene renderer. |

`cargo bk add-crate <name>` and `cargo bk add-app <name> --console|--gui` add a row to this table and to the one in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Your own text in either file is never rewritten.

## Documentation

- [Architecture](docs/ARCHITECTURE.md): how the crates and apps fit together.
- [User manual](docs/manual/content/en/index.md): the pages that ship with the app.
