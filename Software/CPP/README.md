# Arduino / PlatformIO on the LuxCamp Badge 2026

C++ projects for the badge, built with [PlatformIO](https://platformio.org/)
against the Arduino framework for ESP32.

| Project | What it is |
|---------|------------|
| [`Mixed-Examples/`](Mixed-Examples/) | Seven small examples, one per component, plus the `Badge` driver library they all build on. **Start here.** |

A second, game-focused example is planned and will land next to this one.

For the hardware itself — component list, pinout, I/O expander port map and the
errata you should read before debugging anything — see the
[main README](../../README.md).

## Getting PlatformIO

Either the [VS Code extension](https://platformio.org/install/ide?install=vscode)
or the command line tool:

```sh
pip install platformio
```

Then plug the badge in and build a project from its own folder:

```sh
cd Mixed-Examples
pio run -e atom_i2c_scan -t upload -t monitor
```

## Supported boards

The badge is bring-your-own-microcontroller. Two M5Stack boards are supported
out of the box; they share a footprint, so the same badge header positions land
on different GPIO numbers:

| Badge signal | AtomS3 Lite (ESP32-S3) | Atom Lite (ESP32-PICO-D4) |
|--------------|------------------------|---------------------------|
| `BZR`        | G2                     | G26                       |
| `TFT_SCL`    | G5                     | G22                       |
| `TFT_SDA`    | G6                     | G19                       |
| `TFT_DC`     | G7                     | G23                       |
| `TFT_CS`     | G8                     | G33                       |
| `I2C_SDA`    | G38                    | G25                       |
| `I2C_SCL`    | G39                    | G21                       |

> The `TFT_CS` and `TFT_DC` labels on the 5 pin header are swapped on the PCB.
> The table above is correct — it follows the schematic and the 7 pin header.

Projects here follow two conventions so the boards stay interchangeable:

- **Environments are named `atom_*` and `atoms3_*`**, one pair per example, so
  you pick a board by picking an environment.
- **The mapping lives in one file**,
  [`Mixed-Examples/lib/Badge/BadgePins.h`](Mixed-Examples/lib/Badge/BadgePins.h).
  Supporting a third board means adding one `#elif` block there and a pair of
  environments to `platformio.ini` — no driver or example code changes.

**On the AtomS3 Lite, `Serial` needs `-DARDUINO_USB_CDC_ON_BOOT=1`.** The stock
`m5stack-atoms3` board definition leaves `Serial` on UART0, which is not
connected to the USB-C port, so without the flag nothing you print ever shows
up. It is already set for every `atoms3_*` environment. The Atom Lite does not
need it — it has a real USB-to-serial chip on UART0.

## The driver library

[`Mixed-Examples/lib/Badge/`](Mixed-Examples/lib/Badge/) is a self-contained
PlatformIO library covering the display, the I/O expander (buttons, LEDs,
display reset) and the buzzer. Copy the folder into your own project's `lib/`
and add its dependencies to `lib_deps`; the
[Mixed-Examples README](Mixed-Examples/README.md#the-driver-library) documents
the API.
