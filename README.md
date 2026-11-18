# Bowlingbox

Bowlingbox is a 3D ten-pin bowling game rebuilt in Rust. It follows up
[Bowlingbox 1.5.5](https://github.com/medaminben/Bowlingbox), the C++ version. The work is in progress
until v2.0.0. Today the window opens with a menu, an empty central panel and a score line; the lane
arrives in milestone 3.

## Build and run

```
cargo build
cargo test
cargo run -p bowlingbox
```

`cargo run -p bowlingbox -- --version` prints the version and exits. In the window, Esc and Ctrl+Q quit.

`cargo bk version` prints the product version. `cargo bk` lists the other commands.

## Layout

<!-- bk:layout -->
| Path | Purpose |
| --- | --- |
| [`crates/score`](crates/score) | `bowlingbox-score`: the ten-pin scoring rules. |
| [`apps/bowlingbox`](apps/bowlingbox) | `bowlingbox`: the game. |

`cargo bk add-crate <name>` and `cargo bk add-app <name> --console|--gui` add a row to this table and to the one in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Your own text in either file is never rewritten.

## Documentation

- [Architecture](docs/ARCHITECTURE.md): how the crates and apps fit together.
- [User manual](docs/manual/content/en/index.md): the pages that ship with the app.
