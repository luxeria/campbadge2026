# games-firmware

Rust firmware and game logic for the LuxCamp badge (M5Stack Atom S3 Lite).

## Layout

- `firmware` — one embedded binary per game, selected at compile time by a
  cargo feature. Drives the display, I/O expander, buttons and buzzer.
- `games` — platform-independent game logic. No I/O, so it runs on the host and
  is unit tested.
- `raylib_camp` — the small drawing engine (canvas, colors, text, input).

## Games

The `firmware` crate compiles exactly one game per build, chosen by a feature:

| feature | game       | module      |
|---------|------------|-------------|
| `dice`  | Farkle     | `apps::dice`  |
| `snake` | Snake      | `apps::snake`  |
| `seven` | Sevens     | `apps::seven`  |
| `demo`  | demo scene | `apps::demo`  |

`dice` is the default. Each module lives under `firmware/src/apps/` behind its
feature gate and defines one entry point (`apps::dice::main`, ...).
`firmware/src/board.rs` initialises the shared hardware (SPI display, I2C
expander, framebuffer) once and hands a `Board` to the chosen game. Only the
enabled game is compiled, so the buzzer/expander/LED wiring (used by Farkle)
stays out of the other builds.

The `games` crate contains:

- `dice`: playable two-player Farkle variant. The firmware runs this by default.
- `snake`: small example game
- `sevens`: WIP — the Sevens (Fan Tan / Parliament) card game, unfinished

## Build / flash

Use the `Makefile` from this directory. The `GAME` variable selects the game
(one of `dice`, `snake`, `seven`, `demo`); `dice` is the default.

```sh
make build              # default: dice
make flash              # build + flash dice

export GAME=snake       # switch game
make flash              # build + flash snake
```

`make install-toolchain` sets up the Xtensa toolchain once. `espflash`
auto-detects the board; pass `--port` only if several serial devices make it
ambiguous.

## Tests

```sh
cargo test -p games -p raylib_camp
```
