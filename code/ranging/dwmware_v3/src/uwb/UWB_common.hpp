#pragma once

#include <Dw3000/src/dw3000.h>
// #include <Dw3000/src/dw3000.h>
#include <boards.hpp>
#include <Arduino.h>


#define MS_TO_DWT_TIME 249601 //249 600.639

#define MS_TO_DWT_TIME 249601 //249 600.639

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
#elif BOARD==NRF_FEATHER
const uint8_t PIN_RST = A2; // reset pin
const uint8_t PIN_IRQ = 16; // irq pin
const uint8_t PIN_SS = 15; // spi select pin
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
#define POLL_SYSTS_IDX 5
#define RESP_SYSTS_IDX 9

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
#define DWT_FRAME_CRC_LEN 2
#define RX_BUF_LEN 32
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

struct __attribute__((packed)) TagPacket{
    uint8_t sender;
    uint8_t receiver;
    uint8_t msgtype;
    uint32_t resp_delay;
    uint16_t seq;
    uint64_t synctime;
};

struct __attribute__((packed)) AnchorPacket{
    uint8_t sender;
    uint8_t receiver;
    uint8_t msgtype;
    uint16_t seq;
    uint64_t synctime;
};

enum MsgType{
    AnchorRange = 0,
    TagRangeResp = 1,
};

class UWB_Common{
    public:

    // void write_tx_frame(const void* payload, uint16_t payload_len){
    //     uint8_t frame[RX_BUF_LEN] = {};
    //     memcpy(frame, payload, payload_len);
    //     const uint16_t frame_len = payload_len + DWT_FRAME_CRC_LEN;
    //     dwt_writetxdata(frame_len, frame, 0);
    //     dwt_writetxfctrl(frame_len, 0, 1);
    // }
    
    struct Config{
        dwt_config_t dwconfig;
        byte address;
        bool enable_serialreport = false;
    };

    Config config = {standard_dwconfig, 0x0};
    uint32_t last_systime = 0;
    // uint32_t last_systime = UINT32_MAX;
    uint64_t ts_sync_base; // mesh time sync offset to add to SYS_TS (lower 32 bits), supports around 8 years of runtime
    

    void setup(){
         /* Configure SPI rate, DW3000 supports up to 38 MHz */
        /* Reset DW IC */
        // SPI.begin();
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

        // Serial.print("XTRIM OTP: ");
        // uint32_t otpread[1] = {0xABABABAB};
        // dwt_otpread(0x1E, otpread, 1);
        // Serial.println(otpread[0], HEX);

        // uint32_t otpadr = 0x0;
        // while(otpadr <= 0x7F){
        //     Serial.print(otpadr, HEX);
        //     Serial.print(" = ");
        //     dwt_otpread(otpadr, otpread, 1);
        //     Serial.println(otpread[0], HEX);

        //     otpadr += 1;
        // }

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

    void common_loop(){
        // keep track of UWB module system time rollovers
        uint32_t systime = dwt_readsystimestamphi32();
        dwt_write32bitreg(SYS_TIME_ID, 0);
        // uint32_t synctime = systime + uwb_sync_offset;
        if(last_systime > systime){
            ts_sync_base += 1 << 32; // right?
            Serial.print("ROLLOVER: ");
            Serial.print(systime);
            Serial.print(" < ");
            Serial.print(last_systime);
            Serial.print(", now: ");
            Serial.print(static_cast<unsigned long>((ts_sync_base + systime) / MS_TO_DWT_TIME));
            Serial.print("\n");
        }
        last_systime = systime;
    }

    void sync_meshtime(uint64_t fullts_rx, uint32_t systs_at_rx){
        if(last_systime > systs_at_rx){
            ts_sync_base += 1 << 32; // right?
            Serial.print("ROLLOVER: ");
            Serial.print(systs_at_rx);
            Serial.print(" < ");
            Serial.print(last_systime);
            Serial.print(", now: ");
            Serial.print(static_cast<unsigned long>((ts_sync_base + systs_at_rx) / MS_TO_DWT_TIME));
            Serial.print("\n");
        }
        uint64_t my_fullts = ts_sync_base + systs_at_rx;
        last_systime = systs_at_rx;

        if(fullts_rx > my_fullts){
            ts_sync_base = fullts_rx - systs_at_rx;
            Serial.print("SYNC HEX: ");
            // Serial.print(static_cast<unsigned long>(fullts_rx >> 32));
            Serial.print(static_cast<unsigned long>(fullts_rx / MS_TO_DWT_TIME));
            // Serial.print(static_cast<unsigned long>(fullts_rx / MS_TO_DWT_TIME));
            Serial.print(" > ");
            Serial.print(static_cast<unsigned long>(my_fullts / MS_TO_DWT_TIME));
            Serial.print("\n");
        }
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