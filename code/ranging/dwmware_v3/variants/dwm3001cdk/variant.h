/* Qorvo DWM3001CDK board variant (DWM3001C module / nRF52833 + DW3110)
 *
 * Pin mapping follows Qorvo's own sources, verified (2026-10):
 *  - Qorvo DWM3001CDK official firmware Src/Boards/custom_board.h (Decawave Applications)
 *  - Zephyr mainline board dts: boards/qorvo/decawave_dwm3001cdk
 *
 * Arduino digital pin N maps to Nordic GPIO (P0.N / P1.(N-32)):
 *   digital 0..31  -> P0.00..P0.31
 *   digital 32..47 -> P1.00..P1.15
 * So e.g. DW3000 IRQ is on P1.02 = digital 34, SPIM3 SCK P0.03 = digital 3.
 */

#ifndef _VARIANT_DWM3001CDK_
#define _VARIANT_DWM3001CDK_

/** Master clock frequency */
#define VARIANT_MCK       (64000000ul)

#define USE_LFXO      // Board uses 32khz crystal for LF
// define USE_LFRC    // Board uses RC for LF

/*----------------------------------------------------------------------------
 *        Headers
 *----------------------------------------------------------------------------*/

#include "WVariant.h"

#ifdef __cplusplus
extern "C"
{
#endif // __cplusplus

// Number of pins defined in PinDescription array
#define PINS_COUNT           (48)
#define NUM_DIGITAL_PINS     (48)
#define NUM_ANALOG_INPUTS    (8)
#define NUM_ANALOG_OUTPUTS   (0)

/*
 * LEDs (active low)
 */
#define PIN_LED1             (4)   // P0.04  D9  green
#define PIN_LED2             (5)   // P0.05  D10 blue
#define PIN_LED3             (22)  // P0.22  D11 red
#define PIN_LED4             (14)  // P0.14  D12 red

#define LED_BUILTIN          PIN_LED1

#define LED1_PIN             PIN_LED1
#define LED2_PIN             PIN_LED2
#define LED3_PIN             PIN_LED3
#define LED4_PIN             PIN_LED4

#define LED_STATE_ON         0         // State when LED is lit

/*
 * Buttons: SW1 is connected to P0.18 which is nRESET, use SW2 instead
 * (uicr gpio-as-nreset set per Qorvo board config)
 */
#define PIN_BUTTON1          (2)

/*
 * Analog pins (SAADC capable inputs)
 */
#define PIN_A0               (2)    // P0.02 (also SW2)
#define PIN_A1               (3)    // P0.03
#define PIN_A2               (4)    // P0.04 (also LED1)
#define PIN_A3               (5)    // P0.05 (also LED2)
#define PIN_A4               (28)   // P0.28
#define PIN_A5               (29)   // P0.29
#define PIN_A6               (30)   // P0.30
#define PIN_A7               (31)   // P0.31

static const uint8_t A0  = PIN_A0 ;
static const uint8_t A1  = PIN_A1 ;
static const uint8_t A2  = PIN_A2 ;
static const uint8_t A3  = PIN_A3 ;
static const uint8_t A4  = PIN_A4 ;
static const uint8_t A5  = PIN_A5 ;
static const uint8_t A6  = PIN_A6 ;
static const uint8_t A7  = PIN_A7 ;
#define ADC_RESOLUTION    14

// Other pins
#define PIN_AREF           (30)
#define PIN_VBAT           (31)

static const uint8_t AREF = PIN_AREF;

/*
 * Serial interfaces
 */
// UART0 - connected to the on-board J-Link OB virtual COM port
#define PIN_SERIAL1_RX       (15)   // P0.15
#define PIN_SERIAL1_TX       (19)   // P0.19

/*
 * SPI Interfaces
 */
#define SPI_INTERFACES_COUNT 1

#define PIN_SPI_MISO         (29)   // P0.29
#define PIN_SPI_MOSI         (8)    // P0.08
#define PIN_SPI_SCK          (3)    // P0.03

static const uint8_t SS   = (38);    // P1.06 DW3110 SPI chip select
static const uint8_t MOSI = PIN_SPI_MOSI ;
static const uint8_t MISO = PIN_SPI_MISO ;
static const uint8_t SCK  = PIN_SPI_SCK ;

/*
 * Wire Interfaces
 */
#define WIRE_INTERFACES_COUNT 1

#define PIN_WIRE_SDA         (24)   // P0.24 (LIS2DH12 accelerometer)
#define PIN_WIRE_SCL         (36)   // P1.04

#ifdef __cplusplus
}
#endif

/*----------------------------------------------------------------------------
 *        Arduino objects - C++ only
 *----------------------------------------------------------------------------*/

#endif
