#pragma once

#define UPESY 1
#define WEMOSUNO 2
#define NRF_FEATHER 3
#define DWM3001C 4

#ifndef BOARD
#define BOARD UPESY
#endif

// BOARD=4 (DWM3001CDK): use J9 (the J-Link OB virtual COM port) for the
// console. It bridges UART0 on P0.15/P0.19. The separate J20 connector is
// native nRF USB CDC, but is intentionally not the console in this build.
// Remap all sketch-side Serial usage to Serial1 (UART0).
// Must stay a local macro (not a global -DSerial=Serial1): TinyUSB's CDC
// header would expand its `extern Adafruit_USBD_CDC SerialTinyUSB;` to
// `extern Adafruit_USBD_CDC Serial1;` and clash with the Uart object.
#if BOARD==DWM3001C
#define Serial Serial1
#endif

