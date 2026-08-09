# AGENTS.md

## IMPORTNAT: CODE STYLE

DO NOT VIOLATE THIS

### Style guide

We require the official Rust formatter and clippy linter. In addition to that, please also consider the following best-effort aspects:

  - Avoid magic numbers and strings. Instead, add them as module constants.
  - Avoid abbreviated variable and function names. Always provide meaningful and readable symbols.
  - NEVER EVER write in-line code comments except absolutely necessary. Instead write doc comments. Break up functions to factor out code if you find yourself writing slop inline comments!
  - Don’t write macros and don’t use third party macros for things that can easily be expressed in few lines of code or outlined into functions.
  - Avoid import aliasing. Please use the parent or fully qualified path for conflicting symbols.
  - Any inline comments must provide additional semantic meaning, explain counter-intuitive behavior or highlight non-obvious design decisions. In other words, try to make the code expressive enough to a degree it doesn’t need comments expressing the same thing again in the English language. Delete such comments if your AI assistant generated them.
  - Public items must have a meaningful doc comment.
  - Provide meaningful panic messages to .expect() or just use .unwrap().

## Flashing

### C++ (PlatformIO)

The command below targets the **M5Stack Atom S3 Lite** (env: `atoms3_hello_badge`)
and is for the C++/PlatformIO example firmware:

```sh
cd Software/CPP/Mixed-Examples
pio run -e atoms3_hello_badge -t upload -t monitor
```

### Rust (the primary firmware)

There is a short and very usef Makefile that teaches how to do stuff.

TL;DR:

Install the toolchain once: set up Rust via rustup, then install `espflash` and
`espup` with cargo. `espup install` downloads the Xtensa nightly overlays and
creates `~/export-esp.sh`; source that script in every new shell so the
Xtensa toolchain is on the PATH.

```sh
source "$HOME/export-esp.sh"   # configures the Xtensa toolchain for this shell
cd firmware
cargo build --release
cd ..
espflash flash target/xtensa-esp32s3-none-elf/release/firmware --monitor
```

`espflash` auto-detects the connected board; pass `--port` (e.g. `/dev/ttyUSB0`,
adapting to your platform) only if more than one serial device needs
disambiguation.
