module;

#include <Arduino.h>
// #include "boards.hpp"
#include <uwb/SSTWR_responder_uwbsync.hpp>
// #include <uwb/SSTWR_responder.hpp>

#define USEWIFI false
// #include <connectivity/debugserver.hpp>

#define APP_NAME "SS TWR RESP v1.0"
export module tagfirmware;

import serial_commands;

SSTWR_Responder uwb_responder = SSTWR_Responder({standard_dwconfig, 0xA0 + TAG_ID});

#ifndef INFOSTRING
#define INFOSTRING "NODEVINFOSET"
#endif



void arduino_setup()
{
#if defined(ARDUINO_ARCH_NRF52)
    NRF_NVMC->ICACHECNF = NVMC_ICACHECNF_CACHEEN_Enabled;
    NRF_POWER->DCDCEN = 1;
#endif
// #if BOARD==DWM3001C
//     Serial.setPins(15, 19);
// #endif
    Serial.begin(115200);
    Serial.println("LAPOCALYPSE3000 TAG");

    Serial.print("TAG ID: ");
    Serial.println(TAG_ID);
    Serial.print("BOARD ID: ");
    Serial.println(BOARD);

    uwb_responder.setup();

    Serial.println("entering loop");
}

static String serial_buffer;


void arduino_loop()
{
    // debugserver_loop();
    
    serial_commands::process(Serial, serial_buffer, INFOSTRING);
    
    uwb_responder.loop();
}


export void firmware_setup()
{
    arduino_setup();
}

export void firmware_loop()
{
    arduino_loop();
}
