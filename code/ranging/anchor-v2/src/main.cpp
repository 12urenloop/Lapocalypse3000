#include <Arduino.h>
#include <SPI.h>
#include <WiFi.h>
#include <WiFiClient.h>

#include "dw3000_registers.h"
#include "dw3000_api.h"

// SPI Setup
#define RST_PIN 27
#define CHIP_SELECT_PIN 4

#define HSPI 2  // 2 for S2 and S3, 1 for S1
#define VSPI 3

// Set to 1 for Anchor 1, 2 for Anchor 2
#define ANCHOR_ID 1
#define RESPONSE_TIMEOUT_MS 10 // Maximum time to wait for a response
unsigned long last_ranging_time = 0;
#define MAX_RETRIES 3
int retry_count = 0;

// WiFi Configuration
#include "wificonfig.h"
#define USEWIFI false

const int port = 7007;             // Choose a port number
WiFiClient client;
bool wifiConnected = false;

#define BOARD_ESP32_WROOM_DEVBOARD
// #define BOARD_ESP32_WT32_ETH01

#ifdef BOARD_ESP32_WROOM_DEVBOARD
//ESP32 WROOM
const int HSPI_MISO = 19;
const int HSPI_MOSI = 23;
const int HSPI_SCLK = 18;
const int HSPI_SS = 4;

const int VSPI_MISO = 19;
const int VSPI_MOSI = 23;
const int VSPI_SCLK = 18;
const int VSPI_SS = 4;
#define CHIP_SELECT_PIN 4 // ESP32 WROOM
#endif

#ifdef BOARD_ESP32_WT32_ETH01
//WT32-ETH01
const int HSPI_MISO = 15;
const int HSPI_MOSI = 12;
const int HSPI_SCLK = 14;
const int HSPI_SS = 5;

const int VSPI_MISO = 15;
const int VSPI_MOSI = 12;
const int VSPI_SCLK = 14;
const int VSPI_SS = 5;
#define CHIP_SELECT_PIN 5 // WT32-ETH01
#endif



static int rx_status;
static int tx_status;

static int curr_stage = 0;

static int t_roundB = 0;
static int t_replyB = 0;

static long long rx = 0;
static long long tx = 0;




#define DEBUG_OUTPUT 0 // Turn to 1 to get all reads, writes, etc. as info in the console
static int ANTENNA_DELAY = 16350;

int led_status = 0;

int destination = 0x0; // Default Values for Destination and Sender IDs
int sender = 0x0;

SPIClass vspi = SPIClass(VSPI);

// WiFi Functions
void connectToWiFi()
{
    Serial.println("Connecting to WiFi...");
    WiFi.begin(ssid, password);

    int attempts = 0;
    while (WiFi.status() != WL_CONNECTED && attempts < 20)
    {
        delay(500);
        Serial.print(".");
        attempts++;
    }

    if (WiFi.status() == WL_CONNECTED)
    {
        wifiConnected = true;
        Serial.println("\nWiFi connected");
        Serial.print("IP address: ");
        Serial.println(WiFi.localIP());
    }
    else
    {
        Serial.println("\nFailed to connect to WiFi");
    }
}

// extern DWM3000Class DWM3000;
DWM3000Class::Config config = {
    vspi,               // Use VSPI
    CHIP_SELECT_PIN,   // CS Pin
    RST_PIN,           // RST Pin
    VSPI_MOSI,         // MOSI Pin
    VSPI_MISO,         // MISO Pin
    VSPI_SCLK,         // SCK Pin
    ANTENNA_DELAY,     // Antenna Delay
    CHANNEL_5,         // Channel
    PREAMBLE_4096,     // Preamble Length
    9,                 // Preamble Code (Same for RX and TX!)
    PAC8,              // PAC
    DATARATE_850KB,    // Datarate
    PHR_MODE_STANDARD, // PHR Mode
    PHR_RATE_850KB     // PHR Rate
};

DWM3000Class DWM3000(config);

// Initial Radio Configuration
// int DWM3000Class::config = {
//     CHANNEL_5,         // Channel
//     PREAMBLE_4096,      // Preamble Length
//     9,                 // Preamble Code (Same for RX and TX!)
//     PAC8,              // PAC
//     DATARATE_850KB,    // Datarate
//     PHR_MODE_STANDARD, // PHR Mode
//     PHR_RATE_850KB     // PHR Rate
// };




void resetRadio()
{
  Serial.println("[INFO] Performing radio reset...");
  DWM3000.softReset();
  delay(100);
  DWM3000.clearSystemStatus();
  DWM3000.configureAsTX();
  DWM3000.standardRX();
}

void setup()
{
  Serial.begin(115200);
  DWM3000.begin();
  DWM3000.hardReset();
  delay(200);

  if (!DWM3000.checkSPI())
  {
    Serial.println("[ERROR] Could not establish SPI Connection to DWM3000!");
    while (1)
      ;
  }

  while (!DWM3000.checkForIDLE())
  {
    Serial.println("[ERROR] IDLE1 FAILED\r");
    delay(1000);
  }

  DWM3000.softReset();
  delay(200);

  if (!DWM3000.checkForIDLE())
  {
    Serial.println("[ERROR] IDLE2 FAILED\r");
    while (1)
      ;
  }

  DWM3000.init();
  DWM3000.setupGPIO();

  // Set antenna delay - calibrate this for your hardware!
  DWM3000.setTXAntennaDelay(16350);

  // Set anchor ID
  // DWM3000.setSenderID(ANCHOR_ID);
  sender = ANCHOR_ID;

  Serial.print("> ANCHOR ");
  Serial.print(ANCHOR_ID);
  Serial.println(" - Ready for ranging <");
  Serial.print("Antenna delay set to: ");
  Serial.println(DWM3000.getTXAntennaDelay());
  Serial.println("[INFO] Setup finished.");

  DWM3000.configureAsTX();
  DWM3000.clearSystemStatus();
  DWM3000.standardRX();
}

void handleCommand(const String& cmd) {
    // Tokenize
    int firstSpace  = cmd.indexOf(' ');
    int secondSpace = cmd.indexOf(' ', firstSpace + 1);
    int thirdSpace = cmd.indexOf(' ', secondSpace + 1);

    String action = cmd.substring(0, firstSpace);

    if (action == "get") {
        if (firstSpace < 0 || secondSpace < 0) {
            client.println("ERR Invalid format. Use: get <reg> <offset>");
            return;
        }

        int reg    = cmd.substring(firstSpace + 1, secondSpace).toInt();
        int offset = cmd.substring(secondSpace + 1).toInt();

        // readRegisterBytes(reg, offset, buffer, numBytes);
        uint32_t value = DWM3000.read(reg, offset);

        // Send bytes back
        client.write((uint8_t*)&value, sizeof(value));
    }else if(action == "set"){
        if (firstSpace < 0 || secondSpace < 0 || thirdSpace < 0) {
            client.println("ERR Invalid format. Use: set <reg> <offset>");
            return;
        }

        int reg    = cmd.substring(firstSpace + 1, secondSpace).toInt();
        int offset = cmd.substring(secondSpace + 1, thirdSpace).toInt();

        uint32_t data = cmd.substring(thirdSpace + 1).toInt();

        DWM3000.write(reg, offset, data);
        client.write("set OK");
    }else if(action == "otp"){
        if (firstSpace < 0) {
            client.println("ERR Invalid format. Use: otp <reg>");
            return;
        }

        int addr    = cmd.substring(firstSpace + 1).toInt();

        // readRegisterBytes(reg, offset, buffer, numBytes);
        uint32_t value = DWM3000.readOTP(addr);

        // Send bytes back
        client.write((uint8_t*)&value, sizeof(value));
    }
    else {
        client.println("ERR Unknown command");
    }
}

void loop()
{
  if (USEWIFI && !wifiConnected)
  {
      connectToWiFi();
      if (!wifiConnected)
          return;
  }

  if (USEWIFI && !client.connected() && USEWIFI) {
      Serial.println("Disconnected. Reconnecting...");
      while (!client.connect(host, port)) {
          delay(500);
      }
      Serial.println("connected!");
  }

  if (USEWIFI && client.available()) {
      String command = client.readStringUntil('\n');
      command.trim();

      if (command.length() > 0) {
          Serial.println("Received command: " + command);
          handleCommand(command);
      }
  }



  if (DWM3000.receivedFrameSucc() == 1 && DWM3000.ds_getStage() == 1 && DWM3000.getDestinationID() == ANCHOR_ID)
  {
    // Reset session if new ranging request arrives
    if (curr_stage != 0)
    {
      Serial.println("[INFO] New request - resetting session");
      curr_stage = 0;
      t_roundB = 0;
      t_replyB = 0;
    }
  }
  switch (curr_stage)
  {
  case 0: // Await ranging
    t_roundB = 0;
    t_replyB = 0;
    last_ranging_time = millis(); // Reset timeout timer

    if (rx_status = DWM3000.receivedFrameSucc())
    {
      DWM3000.clearSystemStatus();
      if (rx_status == 1)
      { // If frame reception was successful
        // Only respond if frame is addressed to us
        if (DWM3000.getDestinationID() == ANCHOR_ID)
        {
          if (DWM3000.ds_isErrorFrame())
          {
            Serial.println("[WARNING] Received error frame!");
            curr_stage = 0;
            DWM3000.standardRX();
          }
          else if (DWM3000.ds_getStage() != 1)
          {
            Serial.print("[WARNING] Unexpected stage: ");
            Serial.println(DWM3000.ds_getStage());
            // DWM3000.ds_sendErrorFrame(); // turned this off experimentally
            DWM3000.clearSystemStatus();
            curr_stage = 0;
            DWM3000.standardRX();
          }
          else
          {
            curr_stage = 1;
          }
        }
        else
        {
          // Not for us, go back to RX
          DWM3000.standardRX();
        }
      }
      else
      {
        Serial.println("[ERROR] Receiver Error occurred!");
        DWM3000.clearSystemStatus();
        DWM3000.standardRX();
        curr_stage = 0;
      }
    }
    else if (millis() - last_ranging_time > RESPONSE_TIMEOUT_MS)
    {
      Serial.println("[WARNING] Timeout waiting for ranging request");
      if (++retry_count > MAX_RETRIES)
      {
        Serial.println("[ERROR] Max retries reached, resetting radio");
        resetRadio();
        retry_count = 0;
      }
      DWM3000.standardRX(); // Reset to listening mode
    }
    break;

  case 1: // Ranging received. Sending response
    DWM3000.ds_sendFrame(2, sender, destination);

    rx = DWM3000.readRXTimestamp();
    tx = DWM3000.readTXTimestamp();

    t_replyB = tx - rx;
    curr_stage = 2;
    last_ranging_time = millis(); // Reset timeout timer
    break;

  case 2: // Awaiting response
    if (rx_status = DWM3000.receivedFrameSucc())
    {
      retry_count = 0; // Reset on successful response
      DWM3000.clearSystemStatus();
      if (rx_status == 1)
      { // If frame reception was successful
        if (DWM3000.ds_isErrorFrame())
        {
          Serial.println("[WARNING] Received error frame!");
          curr_stage = 0;
          DWM3000.standardRX();
        }
        else if (DWM3000.ds_getStage() != 3)
        {
          Serial.print("[WARNING] Unexpected stage: ");
          Serial.println(DWM3000.ds_getStage());
          // DWM3000.ds_sendErrorFrame(); // turned this off experimentally
          DWM3000.clearSystemStatus();
          curr_stage = 0;
          DWM3000.standardRX();
        }
        else
        {
          curr_stage = 3;
        }
      }
      else
      {
        Serial.println("[ERROR] Receiver Error occurred!");
        DWM3000.clearSystemStatus();
        curr_stage = 0;
        DWM3000.standardRX();
      }
    }
    else if (millis() - last_ranging_time > RESPONSE_TIMEOUT_MS)
    {
      Serial.println("[WARNING] Timeout waiting for second response");
      if (++retry_count > MAX_RETRIES)
      {
        Serial.println("[ERROR] Max retries reached, resetting radio");
        resetRadio();
        retry_count = 0;
      }
      curr_stage = 0;
      DWM3000.standardRX();
    }
    break;

  case 3: // Second response received. Sending information frame
    rx = DWM3000.readRXTimestamp();
    t_roundB = rx - tx;
    DWM3000.ds_sendRTInfo(t_roundB, t_replyB, sender, destination);

    curr_stage = 0;
    DWM3000.standardRX();
    break;

  default:
    Serial.print("[ERROR] Entered unknown stage (");
    Serial.print(curr_stage);
    Serial.println("). Reverting back to stage 0");

    curr_stage = 0;
    DWM3000.standardRX();
    break;
  }
}