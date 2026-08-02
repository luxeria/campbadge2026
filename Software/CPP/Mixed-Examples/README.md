# Mixed examples

Seven small Arduino/PlatformIO examples for the LuxCamp Badge 2026, one per
component, plus the `Badge` driver library they build on.

They are meant to be worked through in order: each one checks a layer the next
one depends on, so when something does not work you know which layer to blame.

- Hardware, pinout, I/O expander port map and errata: [main README](../../../README.md)
- PlatformIO setup and supported boards: [`Software/CPP/README.md`](../README.md)

## Quick start

```sh
pio run -e atom_i2c_scan -t upload -t monitor    # start here
pio run -t upload -t monitor                     # the combined demo
```

There is no single `src/main.cpp`. Each example is its own folder under
`examples/`, with one environment per board (`atom_` = Atom Lite, `atoms3_` =
AtomS3 Lite), so you pick one when you build.

## The examples

| Example | Environments | What it does |
|---------|--------------|--------------|
| [`00_pin_probe`](examples/00_pin_probe/) | `atom_pin_probe` / `atoms3_pin_probe` | Tries every combination of the board's exposed pins as an I2C bus and prints the pair the I/O expander answers on. Only needed for a board that is not in `BadgePins.h` or for hand-wiring. |
| [`01_i2c_scan`](examples/01_i2c_scan/) | `atom_i2c_scan` / `atoms3_i2c_scan` | Scans the bus and reports the expander's address. **Run this first** — buttons, LEDs and the display's reset line all hang off that one chip. Expect exactly one device in `0x74`–`0x77` or `0x20`–`0x27`. |
| [`02_leds`](examples/02_leds/) | `atom_leds` / `atoms3_leds` | Cycles the four LEDs, chases them, blinks them together. Shows why "off" means *configure as input* on this badge. |
| [`03_buttons`](examples/03_buttons/) | `atom_buttons` / `atoms3_buttons` | Prints debounced press/release events, plus the raw port byte — `0xFF` with nothing held. All eight buttons come from a single I2C read. |
| [`04_buzzer`](examples/04_buzzer/) | `atom_buzzer` / `atoms3_buzzer` | Beep, frequency sweep and a short melody, driven with LEDC. The only example that needs no I2C. |
| [`05_display`](examples/05_display/) | `atom_display` / `atoms3_display` | Colour fills, circles, centred text and an orbiting dot on the round panel. |
| [`06_hello_badge`](examples/06_hello_badge/) | `atom_hello_badge` / `atoms3_hello_badge` | Everything together: BTN1–BTN4 switch screens, light the matching LED and click; BTN5 plays a melody. The default environment. |

Each `main.cpp` starts with a comment explaining what the example demonstrates
and which badge-specific detail it exists to teach — that is where the real
documentation lives.

## The driver library

`lib/Badge/` is a normal PlatformIO library, so you can copy the folder into
your own project.

| File | Component |
|------|-----------|
| `BadgePins.h`        | Pin numbers per board, register addresses, button and LED names |
| `BadgeIOExpander.*`  | TCA9539 — buttons, LEDs and the display's reset line |
| `BadgeButtons.*`     | Debouncing and press/release events on top of the expander |
| `BadgeBuzzer.*`      | LEDC square-wave driver, tones and non-blocking melodies |
| `BadgeDisplay.*`     | GC9A01A bring-up, subclasses `Adafruit_GC9A01A` |
| `Badge.h/.cpp`       | The `badge` object that ties the four together |

Examples `00`–`05` use the individual drivers directly; `06_hello_badge` uses
the `badge` object, which is the shortest path to using all of it at once:

```cpp
#include <Badge.h>

void setup() {
  Serial.begin(115200);
  badge.begin();                                 // I2C, expander, buttons, buzzer, display
  badge.display.centerText("Hello", BADGE_TFT_CX, BADGE_TFT_CY, GC9A01A_WHITE);
}

void loop() {
  badge.update();                                // debounce + buzzer timing
  if (badge.buttons.wasPressed(BADGE_BTN1)) {
    badge.io.ledOn(BADGE_LED_GREEN);
    badge.buzzer.beep();
  }
  if (badge.buttons.wasReleased(BADGE_BTN1)) {
    badge.io.ledOff(BADGE_LED_GREEN);
  }
}
```

`badge.begin()` returns `false` if the I/O expander did not answer; the buzzer
still works in that case, since it is wired straight to a GPIO. Pass
`begin(false)` to skip display init and save about a fifth of a second on boot.

`BadgeDisplay` derives from `Adafruit_GC9A01A`, so every Adafruit_GFX call
works unchanged — `fillScreen()`, `drawCircle()`, `setFont()` and so on.
`centerText()` is the one addition, because centring on a round screen is what
you almost always want.

## Things that will surprise you

**The display's reset line is on the I/O expander (P1.0), not on a GPIO.** So
the expander has to be initialised before the display can be — which is why
`BadgeDisplay::begin()` takes a `BadgeIOExpander &`, and why the display
examples bail out if the I2C bus is dead.

**The I2C address is detected at runtime.** Depending on how the expander's
address pins are strapped it can sit anywhere in `0x74`–`0x77` or `0x20`–`0x27`.
The badges measured so far answer at **0x77**, but `BadgeIOExpander::begin()`
probes both ranges anyway rather than hard-coding it. `io.address()` tells you
what it found; pass it explicitly with `io.begin(Wire, 0x77)` to skip the probe.

**LEDs are switched off by configuring the pin as an input** — a hardware
quirk, explained in the [errata](../../../README.md#errata-and-other-important-information).
`BadgeIOExpander::ledOff()` handles it; just do not "fix" it into a `digitalWrite`.

**Nothing in the drivers blocks.** `playTone()`, `beep()` and `playMelody()`
return immediately and are advanced by `badge.update()` (or `buzzer.update()`),
so sound and display updates can happen at the same time. If you call
`playMelody()` and never call `update()`, you will hear one endless note. The
note array is not copied either — it has to outlive the call, so make it
`static const`.

**`Wire.end()` does not undo the ESP32's GPIO matrix routing.** Only relevant if
you write something like the pin probe: a pin used as SCL stays attached and
keeps clocking the bus, so every later pin combination looks like a match. The
probe detaches each pin by hand for exactly this reason.

## Troubleshooting

| Symptom | Likely cause |
|---------|--------------|
| Serial monitor stays empty | On an AtomS3 Lite, missing `-DARDUINO_USB_CDC_ON_BOOT=1`; on either board, the monitor was opened before the board finished resetting |
| `i2c_scan` finds nothing | SDA/SCL swapped, or the badge is not powered. Run `pin_probe`. |
| LEDs glow faintly instead of going off | Something is driving the pin high rather than switching it to an input |
| Display stays black | `TFT_CS`/`TFT_DC` swapped on the 5 pin header (see the errata), or the expander was not found so the panel never came out of reset |
| Pixels look corrupted | Lower `BADGE_TFT_SPI_HZ` in `BadgePins.h`; long jumper wires do not like 40 MHz |
| Buttons read as always pressed | Check the raw port byte printed by `buttons` — with nothing held it should be `0xFF` |
| A melody sounds wrong | Build with `build_flags = -DBADGE_BUZZER_DEBUG` to print every note and its timing. Usually it is the note data: rhythm comes from varying `durationMs`, and a rest shorter than about 100 ms is hard to hear over the gap the player already puts between notes. |
