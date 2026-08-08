# LuxCamp Badge 2026 — Game Firmware (Rust, bare-metal)

A plan / single source of truth for writing a **Rust game firmware** for the
LuxCamp Badge 2026, targeted at the **M5Stack Atom S3 Lite**.

The firmware is a **slim, custom immediate-mode 2D engine** — *inspired by*
raylib's API shape, but **tailored to this badge**. It is **not** a port of
raylib, and it **never touches C, ESP-IDF, FreeRTOS, or PlatformIO**.

---

## 1. Goal

Write games (snake, pong, breakout, …) that run on the badge's round 240×240
display and use its 8 buttons, 4 LEDs and buzzer, with the shortest possible
path from `draw something` to `it shows on the panel`.

The engine exposes a familiar raylib-style immediate-mode API so games read like
desktop raylib, but only the surface we actually need — no windows, no GL, no 3D,
no audio DAC, no file I/O.

---

## 2. Target hardware

### Microcontroller: M5Stack Atom S3 Lite

- SoC: **ESP32-S3FN8**, dual-core Xtensa LX7 @ 240 MHz
- Flash: **8 MB**
- SRAM: ~**320 KB usable** (512 KB total) — **NO PSRAM**
- Native USB for flashing + serial (`/dev/cu.usbmodemXXXX`)

> The **non-Lite** AtomS3 carries PSRAM; the **Lite does not**. raylib's fat
> renderer needs PSRAM, which is one reason we go slim and 2D-only.

### Badge peripherals (documented in the repo README)

| Component | Interface | Pins (AtomS3 Lite) | Notes |
|-----------|-----------|--------------------|-------|
| GC9A01A TFT (1.28", round 240×240) | SPI | SCL=G5, SDA=G6, DC=G7, CS=G8 | No MISO; **reset driven by I/O expander P1.0** |
| UMW-TCA9539PWR I/O expander | I2C | SDA=G38, SCL=G39 | TCA9535-compatible register layout |
| 8 buttons | via expander | P0.0–P0.7 | read low while pressed |
| 4 LEDs | via expander | P1.1–P1.4 | green/blue/red/yellow |
| SMD8530 passive buzzer | PWM (LEDC) | G2 | externally driven, no I2C |

---

## 3. The model: one round canvas, painter's order

The entire engine is one **240×240 RGB565 framebuffer** in SRAM:

- Every draw call rasterizes into the framebuffer in **call order**.
- **Draw order = layering** (painter's algorithm). No depth buffer.
- `present()`/`draw_end()` flushes the framebuffer to the GC9A01A over SPI.
- The canvas knows it is **round**: center `(120, 120)`, radius `120`.
  Anything outside the circle is hidden by the bezel.

### Memory budget

- 240×240 × 2 bytes RGB565 = **112 KB** framebuffer — fits in SRAM.
- A float depth buffer (230 KB) would **not** fit — deliberately not used.
- Keeps the rest of SRAM (~200 KB) for game state, fonts, sprite data.

---

## 4. Why "slim raylib-like" and not real raylib

Real raylib on ESP32 is an **ESP-IDF + FreeRTOS + C** component using a CPU
software renderer (rlgl OpenGL 1.1 software backend, raylib PR #4832 —
e.g. the `georgik/raylib` component). It requires:

- ESP-IDF, a C cross-compile, FreeRTOS — a large non-Rust dependency stack
- PSRAM — which the **Atom S3 Lite does not have**

Both conflict with our constraints (below), so we build our own.

### What an OS would normally provide — mapped to bare metal

| raylib needs | Bare-metal replacement on `esp-hal` |
|--------------|--------------------------------------|
| Time | `esp-hal` systimer / millis |
| Input | Our own read of the TCA9539 buttons |
| Memory | `embedded_alloc` (bump/heap pool) or fixed buffers |
| Threads (audio) | **Dropped** — no DAC, only a PWM buzzer |
| File I/O | **Dropped** — assets via `include_bytes!` or procedural |

---

## 5. Dependency policy (hard constraint)

**No dependencies except the Rust HAL and display/expander drivers.**

Allowed external crates:

- `esp-hal` (the Rust HAL)
- `gc9a01-rs` (+ its transitive `embedded-graphics`, `display-interface`)
- an allocator if we want `Vec`/`String` (e.g. `embedded_alloc`)

Everything else (the engine, math, font, input, LEDs, buzzer) is **our own
code**. No C, no `raylib-sys`, no ESP-IDF, no FreeRTOS, no PlatformIO.

---

## 6. Proposed API surface (badge-tailored)

### Lifecycle
- `badge.init()`
- `draw_begin()` / `draw_end()`  (the latter flushes to the panel)
- `clear(color)`

### Shapes (into the framebuffer)
- `circle(cx, cy, r, color)` / `circle_filled(...)`
- `rect(x, y, w, h, color)` / `rect_filled(...)`
- `triangle(a, b, c, color)`
- `line(x0, y0, x1, y1, thick, color)`
- `pixel(x, y, color)`

### Text / fonts
- `text(str, x, y, scale, color)`
- `measure_text(str, scale)`
- Crisp small built-in bitmap font (no assets required)

### Sprites / blits
- `sprite(data, x, y, w, h)` — blit raw RGB565 data (embedded or procedural art)

### Input — 8 buttons as a gamepad
- `pressed(BTN)`, `just_pressed(BTN)`, `just_released(BTN)`, `hold_time(BTN)`, `any_pressed()`
- Named `BTN1..BTN8`, plus role aliases (`A/B/UP/DOWN/LEFT/RIGHT`) per game

### Outputs
- `led_on(LED)` / `led_off(LED)` / `led_toggle(LED)` — wraps the TCA9539
  "off = configure as input" quirk
- `beep(freq)` / `play_melody(...)` — non-blocking PWM on G2

### Game loop helpers
- `frame_time()` / `fps()` (systimer)
- `rand_u32()` / `randf()`
- A small frame/state accumulator

---

## 7. Proposed architecture

Just an idea. Needs further review and cross-checking with the actual raylib.h implementation.

```
firmware/          # esp-hal no_std binary crate: entry, hw init, main loop
  hw_badge.rs      # GC9A01A (SPI), TCA9539 (I2C: btns/leds/rst), buzzer (PWM)
raylib_camp/       # our badge-tailored immediate-mode "engine"
  canvas.rs        # RGB565 fb + rasterization, round clipping
  shapes.rs        # DrawCircle / Rect / Tri / Line
  text.rs          # bitmap font + DrawText / MeasureText
  sprite.rs        # blit RGB565 pixel data
  input.rs         # TCA9539 buttons → gamepad abstraction
  led.rs           # LED control (off = input quirk)
  buzzer.rs        # non-blocking PWM tones
  math.rs          # Vec2 / rand / helpers
games/             # game crates: snake, pong, breakout, …
```

`firmware` depends on `raylib_camp`; `games` depend on `raylib_camp`.

---

## 8. Hardware gotchas to respect (from the repo README, errata)

Needs further review and cross-checking.

- **`TFT_CS` and `TFT_DC` labels are swapped** on the 5-pin header. The correct
  mapping follows the schematic / 7-pin header (`DC=G7`, `CS=G8`). Display black
  ⇒ check this.
- **The display's reset line is on the I/O expander (P1.0)**, not a GPIO.
  ⇒ The TCA9539 must be initialised **before** the display.
- **LEDs turn OFF by configuring the pin as INPUT** (they are driven at 5 V;
  the expander only pulls up to VCC). Do *not* implement "off" as driving high.
  The POWER LED cannot be turned off.
- **Some badges have two blue LEDs** — `LED_RED` may glow blue.
- **Buttons read LOW while pressed**; idle port byte is `0xFF`.
- **I2C address is dynamic** (probed): usually `0x77`, in range `0x74–0x77`
  (TCA9539) or `0x20–0x27` (TCA9535). Probe rather than hard-code.
- **Buzzer is passive** — it needs an externally driven square wave (PWM),
  it cannot be "turned on" with a DC level.
- **No MISO** on the display; **no filesystem** — embed assets.

---

## 9. Flashing / build commands (recorded here for convenience)

Target board env: `atoms3_hello_badge` (with the Rust firmware, this becomes
`cargo build --release` + a flash step via `espflash`/`esptool`).

Reference flashing command (from the C++/PlatformIO setup, for the S3 Lite):

```sh
cd Software/CPP/Mixed-Examples
pio run -e atoms3_hello_badge -t upload -t monitor
```

> The Rust workflow will replace PlatformIO with cargo + `espflash`; the board
> mapping in `BadgePins.h` (G5/G6/G7/G8/G38/G39/G2) is the source of truth for
> the `hw_badge.rs` pin definitions.

---

## 10. Suggested milestones

1. **Canvas + clear + flush** — framebuffer, `clear(color)`, `present()` to the
   real round panel (verify it physically shows colour). *The foundation.*
2. **Shapes** — `rect`, `circle`, `line`, `triangle` rasterization.
3. **Text** — bitmap font, `text()` and `measure_text()`.
4. **Sprites** — blit RGB565 data (`draw_texture` equivalent).
5. **Input** — TCA9539 buttons → `just_pressed`/`just_released`, debounce.
6. **Output** — `led_*` and `beep`/`play_melody`.
7. **Game loop** — `frame_time`, `fps`, `rand`, frame accumulator.
8. **First game** — snake, pong or breakout on the round screen.

---

## 11. Open questions (to lock before scaffolding)

1. **Coordinate origin**
   - Top-left (`0..240`, raylib-style) — familiar, consistent with the panel.
   - Center-origin (`-120..120`) — feels natural for round games.
   - *Recommendation: top-left for drawing, plus a small center-based helper
     for symmetric/round layouts.*
2. **Allocator**
   - `core`-only (fixed buffers, no `Vec`) — simplest, safest on ~320 KB.
   - `embedded_alloc` (permit `Vec`/`String`) — easier game state.
   - *Recommendation: start `core`-only; opt into an allocator if a game needs
     dynamic collections.*

---

## 12. Non-goals (explicitly out of scope)

- 3D / depth buffering (won't fit, not needed for 2D games)
- Full rlgl software rasterizer
- Audio beyond PWM beeps/melodies (no DAC on the badge)
- File system / loading external assets
- Real raylib C via ESP-IDF / FreeRTOS

## 13. IMPORTNAT: CODE STYLE

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


