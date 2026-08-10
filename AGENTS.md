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


### Rust (the primary firmware)

Read and use the `games-firmware/Makefile`!

```sh
source "$HOME/export-esp.sh"
cd games-firmware
GAME=dice make flash
GAME=dice make flash
```

### C++ (PlatformIO)

The command below targets the **M5Stack Atom S3 Lite** (env: `atoms3_hello_badge`)
and is for the C++/PlatformIO example firmware:

```sh
cd Software/CPP/Mixed-Examples
pio run -e atoms3_hello_badge -t upload -t monitor
```
