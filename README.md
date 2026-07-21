# LuxCamp Badge 2026

Badge for the LuxCamp 2026. It is a bring-your-own-microcontroller badge
with the following components:


  - [GC9A01](Datasheets/GC9A01A.pdf) 1.28 inch 240×240px TFT display
  - [UMW-TCA9539PWR](Datasheets/UMW-TCA9539PWR.pdf) I/O expander for reading buttons and controlling four LEDs
  - [XHXDZ SMD8530-32Ω](Datasheets/SMD8530-32Ohm.pdf) passive (externally driven) buzzer
  - [TPS61023](Datasheets/TPS61023.pdf)-based booster for battery powered operations


## Getting Started

The examples in this repository have been written with the
[M5Stack Atom Lite](https://shop.m5stack.com/products/atom-lite-esp32-development-kit)
or [M5Stack Atom S3 Lite](https://shop.m5stack.com/products/atoms3-lite-esp32s3-dev-kit)
microcontroller in mind. Check out the following examples in the `Software`
folder to get familiarized with the badge:

 - [Getting Started with MicroPython](Software/MicroPython/)
 - [Getting Started with Arduino/PlatformIO](Software/CPP/)

## Render

![PCB Render Front](Hardware/Design/Render_Front.png)

## License

Unless where stated otherwise, all files in this repository are licensed under
the [MIT License](LICENSE).
