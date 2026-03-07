#pragma once

#include <Dw3000/src/dw3000.h>
#include <boards.hpp>

#define N_TAGS 2
#define ANCHORBROADCAST 0xFF // broadcast address


// connection pins

#if BOARD==UPESY
const uint8_t PIN_RST = 27; // reset pin
const uint8_t PIN_IRQ = 14; // irq pin
const uint8_t PIN_SS = 4;   // spi select 
#elif BOARD==WEMOSUNO
const uint8_t PIN_RST = 14; // reset pin
const uint8_t PIN_IRQ = 12; // irq pin
const uint8_t PIN_SS = 5; // spi select pin
#else
const uint8_t PIN_RST = 27; // reset pin
const uint8_t PIN_IRQ = 14; // irq pin
const uint8_t PIN_SS = 4;   // spi select 
#endif


/* Inter-ranging delay period, in milliseconds. */
#define RNG_DELAY_MS 30

/* Default antenna delay values for 64 MHz PRF. See NOTE 2 below. */
#ifndef TX_ANT_DLY
#define TX_ANT_DLY 16415
#endif

#ifndef RX_ANT_DLY
#define RX_ANT_DLY 16415
#endif

/* Length of the common part of the message (up to and including the function code, see NOTE 3 below). */
// #define ALL_MSG_COMMON_LEN 10
#define ALL_MSG_COMMON_LEN 3
/* Indexes to access some of the fields in the frames defined above. */
#define ALL_MSG_SN_IDX 7
#define RES_MSG_DELAY_IDX 3
#define RESP_MSG_TS_LEN 4
#define POLL_TX_TO_RESP_RX_DLY_UUS 240

/* Frame sequence number, incremented after each transmission. */
uint8_t frame_seq_nb = 0;

#define RESP_RX_TIMEOUT_UUS 400

/* Hold copies of computed time of flight and distance here for reference so that it can be examined at a debug breakpoint. */
double tof;
double distance;

/* Values for the PG_DELAY and TX_POWER registers reflect the bandwidth and power of the spectrum at the current
 * temperature. These values can be calibrated prior to taking reference measurements. See NOTE 2 below. */
extern dwt_txconfig_t txconfig_options;

/* Buffer to store received response message.
 * Its size is adjusted to longest frame that this example code is supposed to handle. */
#define RX_BUF_LEN 12
uint8_t rx_buffer[RX_BUF_LEN];

/* Default communication configuration. We use default non-STS DW mode. */
const dwt_config_t standard_dwconfig = {
        5,               /* Channel number. */
        DWT_PLEN_1024,    /* Preamble length. Used in TX only. */
        DWT_PAC16,        /* Preamble acquisition chunk size. Used in RX only. */
        9,               /* TX preamble code. Used in TX only. */
        9,               /* RX preamble code. Used in RX only. */
        2,               /* 0 to use standard 8 symbol SFD, 1 to use non-standard 8 symbol, 2 for non-standard 16 symbol SFD and 3 for 4z 8 symbol SDF type */
        DWT_BR_850K,      /* Data rate. */
        DWT_PHRMODE_STD, /* PHY header mode. */
        DWT_PHRRATE_STD, /* PHY header rate. */
        (1025 + 16 - 16),   /* SFD timeout (preamble length + 1 + SFD length - PAC size). Used in RX only. */
        DWT_STS_MODE_OFF, /* STS disabled */
        DWT_STS_LEN_64,/* STS length see allowed values in Enum dwt_sts_lengths_e */
        DWT_PDOA_M0      /* PDOA mode off */
};



class UWB_Common{
    public:
    
    struct Config{
        dwt_config_t dwconfig;
        byte address;
        bool enable_serialreport = false;
    };

    Config config = {standard_dwconfig, 0x0};

    void setup(){
         /* Configure SPI rate, DW3000 supports up to 38 MHz */
        /* Reset DW IC */
        spiBegin(PIN_IRQ, PIN_RST);
        spiSelect(PIN_SS);

        delay(2); // Time needed for DW3000 to start up (transition from INIT_RC to IDLE_RC, or could wait for SPIRDY event)

        while (!dwt_checkidlerc()) // Need to make sure DW IC is in IDLE_RC before proceeding
        {
            UART_puts("IDLE FAILED\r\n");
            while (1)
                ;
        }

        if (dwt_initialise(DWT_DW_INIT) == DWT_ERROR)
        {
            UART_puts("INIT FAILED\r\n");
            while (1)
                ;
        }

        // Enabling LEDs here for debug so that for each TX the D1 LED will flash on DW3000 red eval-shield boards.
        dwt_setleds(DWT_LEDS_ENABLE | DWT_LEDS_INIT_BLINK);

        /* Configure DW IC. See NOTE 6 below. */
        if (dwt_configure(&config.dwconfig)) // if the dwt_configure returns DWT_ERROR either the PLL or RX calibration has failed the host should reset the device
        {
            UART_puts("CONFIG FAILED\r\n");
            while (1)
                ;
        }

        /* Configure the TX spectrum parameters (power, PG delay and PG count) */
        dwt_configuretxrf(&txconfig_options);

        /* Apply default antenna delay value. See NOTE 2 below. */
        dwt_setrxantennadelay(RX_ANT_DLY);
        dwt_settxantennadelay(TX_ANT_DLY);

        /* Next can enable TX/RX states output on GPIOs 5 and 6 to help debug, and also TX/RX LEDs
        * Note, in real low power applications the LEDs should not be used. */
        dwt_setlnapamode(DWT_LNA_ENABLE | DWT_PA_ENABLE);
        

        dwt_write32bitreg(TX_POWER_ID, 0xFFFFFFEF);
        dwt_setleds(0b11);
        dwt_write32bitreg(LED_CTRL_ID, 0x0101); // set shortest led blink time

    }

    void LEDBlinkBlocking(){
        for (int i = 0; i < 200; i++)
        {
            dwt_write32bitreg(LED_CTRL_ID + 2, 0b1111);
            delay(2);
        }
        delay(500);
        for (int i = 0; i < 200; i++)
        {
            dwt_write32bitreg(LED_CTRL_ID + 2, 0b0000);
            delay(2);
        }
        delay(500);
        for (int i = 0; i < 200; i++)
        {
            dwt_write32bitreg(LED_CTRL_ID + 2, 0b1111);
            delay(2);
        }
        delay(500);
        for (int i = 0; i < 200; i++)
        {
            dwt_write32bitreg(LED_CTRL_ID + 2, 0b0000);
            delay(2);
        }
    }

};