module;
#include <Arduino.h>
#include <WiFi.h>
#include <esp_now.h>
#include <env/anchorconfig.hpp>

export module firmware;

import serial_commands;
import sstwr_initiator_uwbsync;
import espnow_reporter;

#define USEWIFI true
#define UWB_DEBUG true

// #include <connectivity/mqtt_reporter.hpp>
// #include <connectivity/debugserver.hpp>

#define APP_NAME "SS TWR INIT v1.0"


SSTWR_Initiator::Config anchorconfig = {
        24, 24 / N_ANCHORS, true
    };

SSTWR_Initiator UWBInitiator = 
    SSTWR_Initiator(0x0 + ANCHOR_ID, anchorconfig, ANCHOR_ID - 1);


export void anchor_setup()
{
#if BOARD==DWM3001C
    Serial.setPins(15, 19);
#endif
    Serial.begin(115200);
    Serial.println("STARTUP");
    Serial.println("LAPOCALYPSE3000 ANCHOR");
    
    Serial.print("ANCHOR ID: ");
    Serial.println(ANCHOR_ID);
    Serial.print("BOARD ID: ");
    Serial.println(BOARD);
    delay(1000);

    UWBInitiator.setup();

    setup_espnow();
    esp_now_register_recv_cb(anchorOnReceive);

}

// String serialBuffer = "";
bool workingReceive = false; // received a valid packet yet?

uint64_t lastReceive = 0;

unsigned long lastreport = 0;

bool uwbOn = true;
static String serialBuffer = "";

export void anchor_loop()
{

    if(uwbOn){
        UWBInitiator.slotted_loop();
    }else{
        delay(1000);
        Serial.println("send RESET to turn on UWB");
    }

    while (Serial.available()) {
        char c = (char)Serial.read();
        serialBuffer += c;
        if (serialBuffer.length() > 64) serialBuffer = serialBuffer.substring(serialBuffer.length() - 64);
        // Serial.println(serialBuffer);
        
        
        if(c != '\n') continue;
        bool commanded = true;
        if(serialBuffer.indexOf("AT+GETINFO") != -1){
            Serial.println("");
            Serial.print("INFO=");
            Serial.println(INFOSTRING);
        }else{
            commanded = false;
        }
        
        if(commanded){
            // serialBuffer.clear();
        }
    }


    unsigned long ms = millis();
    if (ms - lastreport >= 175)
    {
        delay(ANCHOR_ID);
        sendData(UWBInitiator.tagIDs, UWBInitiator.distances);
        Serial.print("loopms = ");
        Serial.println(millis() - lastreport);
        lastreport = millis();
    }

    /* Execute a delay between ranging exchanges. */
    // Sleep(1);
}


export void firmware_setup()
{
    anchor_setup();
}

export void firmware_loop()
{
    anchor_loop();
}
