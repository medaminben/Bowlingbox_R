# Bowlingbox

Bowlingbox is a Rust workspace made with buildkit-rust. Describe what Bowlingbox does in one or two sentences here.

## Build and run

```
cargo build
cargo test
cargo run -p bowlingbox
```

`cargo bk version` prints the product version. `cargo bk` lists the other commands.

## Layout

<!-- bk:layout -->
| Path | Purpose |
| --- | --- |
| [`crates/score`](crates/score) | `bowlingbox-score`: describe what this crate is for. |
| [`apps/bowlingbox`](apps/bowlingbox) | `bowlingbox`: the desktop app. |

`cargo bk add-crate <name>` and `cargo bk add-app <name> --console|--gui` add a row to this table and to the one in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Your own text in either file is never rewritten.

## Documentation

- [Architecture](docs/ARCHITECTURE.md): how the crates and apps fit together.
- [User manual](docs/manual/content/en/index.md): the pages that ship with the app.
