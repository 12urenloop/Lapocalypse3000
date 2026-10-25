# DWM3001CDK bring-up (nRF52833 + DW3110)

How this port works, what is different from the nRF52832 Feather tags, and how
to flash/monitor it.

## What was set up

- **Custom board definition** `boards/dwm3001cdk.json`
  - Adafruit nRF52 BSP for Arduino, nRF52833, SoftDevice **S140 7.3.0**
  - application is linked at `0x27000` (`nrf52833_s140_v7.ld`)
- **Custom variant** `variants/dwm3001cdk/` (Arduino pin map: digital pin *N* =
  P0.N, digital pin *32+N* = P1.N). Pin mapping verified against Qorvo's own
  DWM3001CDK firmware (`custom_board.h`, Decawave Applications) and Zephyr's
  mainline board file; **differs from the first draft mapping** (see
  `dwm3001C_mapping.csv`, all rows now carry a Source column):
  - DW3110 SPI runs on **SPIM3**: SCK `P0.03`, MOSI `P0.08`, MISO `P0.29`
  - SPI CS: `P1.06` (digital 38), DW IRQ input: `P1.02` (digital 34),
    DW reset: `P0.25` (digital 25)
- **Encoder `src/boards.hpp`**: `BOARD=4` (`DWM3001C`), remaps `Serial` to
  `Serial1` (hardware UART0).
- **Flash helper** `uploader/dwm3001cdk_flash.py` (see below).

## Serial console

The CDK has two micro-USB connectors. This build uses **J9**, the **J-Link OB**
connector, which bridges **UART0** (`P0.15` RX / `P0.19` TX) as a virtual COM
port. The separate **J20** connector is the nRF52833's native USB CDC interface
and is deliberately not used for this console. Monitor:

```
pio device monitor -e nrftag1dwm3001c          # picks the J-Link VCOM at 115200
```

## First bring-up on a factory board

Factory firmware (Qorvo/Zephyr, no SoftDevice) must be replaced once. The J-Link
must be the one connected to the J-Link USB connector.

1. Build the firmware:
   ```
   pio run -e nrftag1dwm3001c
   ```
2. Mass-erase, program MBR + **SoftDevice S140 7.3.0** (from
   `tools/softdevice/`, downloaded from the official Adafruit bootloader repo,
   license file included) and, if already built, also the app:
   ```
   pio run -e nrftag1dwm3001c -t flash_softdevice
   ```
3. Afterwards, normal flashing of just the app works like on the feathers:
   ```
   pio run -e nrftag1dwm3001c -t upload
   ```

The app is linked at 0x27000; `upload_protocol = jlink` was replaced by a
custom runner (`loadfile`) that flashes the IntelHex at its linked addresses,
so no offset juggling is needed.

## XTAL trim

`-DXTALTRIM=40` from the Feather tags is **not** carried over; the DW3110 in
the DWM3001C module is a different crystal, code default stays `26`. Calibrate
per device like on the feathers and add e.g. `-DXTALTRIM=xx` to this env.

## Environment variables

`[env:nrftag1dwm3001c]` (and alias `[env:nrftag3001c]`) in `platformio.ini`:

- `TAG_ID=1`, `INFOSTRING="DWM3001C1"` (matches the feather tags' pattern)
- `Serial` → `Serial1` remap is local to `src/boards.hpp`; do not make it a
  global `-DSerial=Serial1`, which would collide with TinyUSB's CDC declaration.
- Aggressive optimization flags enabled (`-Ofast`, loop unrolling, aggressive inlining, frame-pointer omission, Cortex-M4 tuning, and function/loop alignment) for minimal latency in delayed UWB TX timing.
