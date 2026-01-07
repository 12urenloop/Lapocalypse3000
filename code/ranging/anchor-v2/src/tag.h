#include <Arduino.h>
#include <SPI.h>
#include <WiFi.h>
#include <WiFiClient.h>

#include "dw3000_registers.h"
#include "regids_dw3000_api.h"

#define HSPI 2  // 2 for S2 and S3, 1 for S1
#define VSPI 3

// replace with the pins you want to use

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

//WT32-ETH01
// const int HSPI_MISO = 15;
// const int HSPI_MOSI = 12;
// const int HSPI_SCLK = 14;
// const int HSPI_SS = 5;

// const int VSPI_MISO = 15;
// const int VSPI_MOSI = 12;
// const int VSPI_SCLK = 14;
// const int VSPI_SS = 5;
// #define CHIP_SELECT_PIN 5 // WT32-ETH01

// WiFi Configuration
#include "wificonfig.h"
#define USEWIFI false

const int port = 7007;             // Choose a port number
WiFiClient client;
bool wifiConnected = false;

// SPI Setup
#define RST_PIN 17

// Scalable Anchor Configuration
#define NUM_ANCHORS 1 // Change this to scale the system
#define TAG_ID 10
#define FIRST_ANCHOR_ID 1 // Starting ID for anchors (1, 2, 3, ...)

// Ranging Configuration
#define FILTER_SIZE 30 // For median filter
#define MIN_DISTANCE 0
#define MAX_DISTANCE 5000.0 // 50 meters

// UWB Configuration
#define LEN_RX_CAL_CONF 4
#define LEN_TX_FCTRL_CONF 6
#define LEN_AON_DIG_CFG_CONF 3
#define PMSC_STATE_IDLE 0x3
#define FCS_LEN 2
#define STDRD_SYS_CONFIG 0x188
#define DTUNE0_CONFIG 0x0F
#define SYS_STATUS_FRAME_RX_SUCC 0x2000
#define SYS_STATUS_RX_ERR 0x4279000
#define SYS_STATUS_FRAME_TX_SUCC 0x80
#define PREAMBLE_32 4
#define PREAMBLE_64 8
#define PREAMBLE_128 5
#define PREAMBLE_256 9
#define PREAMBLE_512 11
#define PREAMBLE_1024 2
#define PREAMBLE_2048 10
#define PREAMBLE_4096 3
#define PREAMBLE_1536 6
#define CHANNEL_5 0x0
#define CHANNEL_9 0x1
#define PAC4 0x03
#define PAC8 0x00
#define PAC16 0x01
#define PAC32 0x02
#define DATARATE_6_8MB 0x1
#define DATARATE_850KB 0x0
#define PHR_MODE_STANDARD 0x0
#define PHR_MODE_LONG 0x1
#define PHR_RATE_6_8MB 0x1
#define PHR_RATE_850KB 0x0
#define SPIRDY_MASK 0x80
#define RCINIT_MASK 0x100
#define BIAS_CTRL_BIAS_MASK 0x1F
#define GEN_CFG_AES_LOW_REG 0x00
#define GEN_CFG_AES_HIGH_REG 0x01
#define STS_CFG_REG 0x2
#define RX_TUNE_REG 0x3
#define EXT_SYNC_REG 0x4
#define GPIO_CTRL_REG 0x5
#define DRX_REG 0x6
#define RF_CONF_REG 0x7
#define RF_CAL_REG 0x8
#define FS_CTRL_REG 0x9
#define AON_REG 0xA
#define OTP_IF_REG 0xB
#define CIA_REG1 0xC
#define CIA_REG2 0xD
#define CIA_REG3 0xE
#define DIG_DIAG_REG 0xF
#define PMSC_REG 0x11
#define RX_BUFFER_0_REG 0x12
#define RX_BUFFER_1_REG 0x13
#define TX_BUFFER_REG 0x14
#define ACC_MEM_REG 0x15
#define SCRATCH_RAM_REG 0x16
#define AES_RAM_REG 0x17
#define SET_1_2_REG 0x18
#define INDIRECT_PTR_A_REG 0x1D
#define INDIRECT_PTR_B_REG 0x1E
#define IN_PTR_CFG_REG 0x1F
#define TRANSMIT_DELAY 0x3B9ACA00
#define TRANSMIT_DIFF 0x1FF
#define NS_UNIT 4.0064102564102564    // ns
#define PS_UNIT 15.6500400641025641   // ps
#define SPEED_OF_LIGHT 0.029979245800 // in centimetres per picosecond
#define CLOCK_OFFSET_CHAN_5_CONSTANT -0.5731e-3f
#define CLOCK_OFFSET_CHAN_9_CONSTANT -0.1252e-3f
#define NO_OFFSET 0x0
#define DEBUG_OUTPUT 0
static int ANTENNA_DELAY = 16350;
int led_status = 0;
int destination = 0x0;
int sender = 0x0;

// OTP registers
#define OTP_CFG 0x08

SPIClass vspi = SPIClass(VSPI);

// Initial Radio Configuration
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
    PAC16,              // PAC
    DATARATE_850KB,    // Datarate
    PHR_MODE_STANDARD, // PHR Mode
    PHR_RATE_850KB     // PHR Rate
};

DWM3000Class dwm(config);

// Global variables
static int rx_status;
static int tx_status;
static int current_anchor_index = 0; // Index into anchors array
static int curr_stage = 0;

// Anchor data structure
struct AnchorData
{
    int anchor_id; // Anchor ID

    // Timing measurements
    int t_roundA = 0;
    int t_replyA = 0;
    long long rx = 0;
    long long tx = 0;
    int clock_offset = 0;

    // Distance measurements
    float distance = 0;
    float distance_history[FILTER_SIZE] = {0};
    int history_index = 0;
    float filtered_distance = 0;

    // Signal quality metrics
    float signal_strength = 0;    // RSSI in dBm
    float fp_signal_strength = 0; // First Path RSSI in dBm
};

// Dynamic array of anchor data
AnchorData anchors[NUM_ANCHORS];

// Helper functions for anchor management
void initializeAnchors()
{
    for (int i = 0; i < NUM_ANCHORS; i++)
    {
        anchors[i].anchor_id = FIRST_ANCHOR_ID + i;
        // Initialize all other fields to zero (default constructor handles this)
    }
}

AnchorData *getCurrentAnchor()
{
    return &anchors[current_anchor_index];
}

int getCurrentAnchorId()
{
    return anchors[current_anchor_index].anchor_id;
}

void switchToNextAnchor()
{
    current_anchor_index = (current_anchor_index + 1) % NUM_ANCHORS;
}

bool allAnchorsHaveValidData()
{
    for (int i = 0; i < NUM_ANCHORS; i++)
    {
        if (anchors[i].filtered_distance <= 0)
        {
            return false;
        }
    }
    return true;
}


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

void sendData()
{
    if (USEWIFI && !wifiConnected)
    {
        Serial.println("reconnecting to wifi...");
        connectToWiFi();
        if (!wifiConnected)
            return;
    }

    if (USEWIFI && !client.connected())
    {
        Serial.println("reconnecting to control server...");
        if (!client.connect(host, port))
        {
            Serial.println("Connection to host failed");
            wifiConnected = false;
            return;
        }
    }

    // Create JSON structure dynamically based on number of anchors
    String data = "{\"tag_id\":" + String(TAG_ID) + ",\"anchors\":{";

    for (int i = 0; i < NUM_ANCHORS; i++)
    {
        data += "\"A" + String(anchors[i].anchor_id) + "\":{";
        data += "\"distance\":" + String(anchors[i].filtered_distance, 2) + ",";
        data += "\"raw\":" + String(anchors[i].distance, 2) + ",";
        data += "\"rssi\":" + String(anchors[i].signal_strength, 2) + ",";
        data += "\"fp_rssi\":" + String(anchors[i].fp_signal_strength, 2) + ",";
        data += "\"round_time\":" + String(anchors[i].t_roundA) + ",";
        data += "\"reply_time\":" + String(anchors[i].t_replyA) + ",";
        data += "\"clock_offset\":" + String((double)dwm.getClockOffset(anchors[i].clock_offset), 6);
        data += "}";

        // Add comma if not the last anchor
        if (i < NUM_ANCHORS - 1)
        {
            data += ",";
        }
    }

    data += "}}\n";

    if(USEWIFI) client.print(data);

    // For debugging, print the JSON to serial
    // Serial.println("Sent JSON data:");
    Serial.print(millis());
    Serial.print(": ");
    Serial.println(data);
}

// Helper function to validate distance
bool isValidDistance(float distance)
{
    return (distance >= MIN_DISTANCE && distance <= MAX_DISTANCE);
}

float calculateMedian(float arr[], int size)
{
    float temp[size];
    for (int i = 0; i < size; i++)
    {
        temp[i] = arr[i];
    }

    for (int i = 0; i < size - 1; i++)
    {
        for (int j = i + 1; j < size; j++)
        {
            if (temp[j] < temp[i])
            {
                float t = temp[i];
                temp[i] = temp[j];
                temp[j] = t;
            }
        }
    }

    if (size % 2 == 0)
    {
        return (temp[size / 2 - 1] + temp[size / 2]) / 2.0;
    }
    else
    {
        return temp[size / 2];
    }
}

// updateFilteredDistance function
void updateFilteredDistance(AnchorData &data)
{
    data.distance_history[data.history_index] = data.distance;
    data.history_index = (data.history_index + 1) % FILTER_SIZE;

    float valid_distances[FILTER_SIZE];
    int valid_count = 0;

    for (int i = 0; i < FILTER_SIZE; i++)
    {
        if (isValidDistance(data.distance_history[i]))
        {
            valid_distances[valid_count++] = data.distance_history[i];
        }
    }

    if (valid_count > 0)
    {
        data.filtered_distance = calculateMedian(valid_distances, valid_count);
    }
    else
    {
        data.filtered_distance = 0;
    }
}

// Debug print function
void printDebugInfo(int anchor, long long rx, long long tx, int t_round, int t_reply, int clock_offset)
{
    Serial.print("Anchor ");
    Serial.print(anchor);
    Serial.println(" Debug Info:");
    Serial.print("RX timestamp: ");
    Serial.println(rx);
    Serial.print("TX timestamp: ");
    Serial.println(tx);
    Serial.print("t_round: ");
    Serial.println(t_round);
    Serial.print("t_reply: ");
    Serial.println(t_reply);
    Serial.print("Clock offset: ");
    Serial.println(clock_offset);

    int ranging_time = dwm.ds_processRTInfo(t_round, t_reply,
                                                dwm.read(0x12, 0x04), dwm.read(0x12, 0x08), clock_offset);
    Serial.print("Calculated distance: ");
    Serial.println(dwm.convertToCM(ranging_time));
}

void printAllDistances()
{
    Serial.print("Distances - ");
    for (int i = 0; i < NUM_ANCHORS; i++)
    {
        Serial.print("A");
        Serial.print(anchors[i].anchor_id);
        Serial.print(": ");
        if (anchors[i].filtered_distance > 0)
        {
            dwm.printDouble(anchors[i].filtered_distance, 100, false);
            Serial.print(" cm");
        }
        else
        {
            Serial.print("INVALID - ");
            dwm.printDouble(anchors[i].filtered_distance, 100, false);
        }

        if (i < NUM_ANCHORS - 1)
        {
            Serial.print(" | ");
        }
    }
    Serial.println();
}

void diagnostic(){
    for(int base = 0; base <= 10; base++){
        for(int sub = 0; sub <= 0x68; sub += 4){
            int result = dwm.read(base, sub);
            Serial.printf("%02x:%02x = %#010x\n", base, sub, result);
        }
    }
}

void setup()
{
    Serial.begin(115200);
    // Initialize anchor array
    initializeAnchors();

    Serial.print("Initialized ");
    Serial.print(NUM_ANCHORS);
    Serial.println(" anchors:");
    for (int i = 0; i < NUM_ANCHORS; i++)
    {
        Serial.print("  Anchor ");
        Serial.print(i);
        Serial.print(" - ID: ");
        Serial.println(anchors[i].anchor_id);
    }

    // Connect to WiFi first
    if(USEWIFI) connectToWiFi();

    // Initialize UWB
    dwm.begin();
    dwm.hardReset();
    delay(200);

    if (!dwm.checkSPI())
    {
        Serial.println("[ERROR] Could not establish SPI Connection to DWM3000!");
        while (1)
            ;
    }

    while (!dwm.checkForIDLE())
    {
        Serial.println("[ERROR] IDLE1 FAILED\r");
        delay(1000);
    }

    dwm.softReset();
    delay(200);

    if (!dwm.checkForIDLE())
    {
        Serial.println("[ERROR] IDLE2 FAILED\r");
        while (1)
            ;
    }

    dwm.init();
    dwm.setupGPIO();
    dwm.setTXAntennaDelay(16350);

    Serial.println("> TAG - Three Anchor Ranging System <");
    Serial.println("> With WiFi Communication <\n");
    Serial.println("[INFO] Setup is finished.");
    Serial.print("Antenna delay set to: ");
    Serial.println(dwm.getTXAntennaDelay());

    dwm.configureAsTX();
    dwm.clearSystemStatus();

    // diagnostic();
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
        uint32_t value = dwm.read(reg, offset);

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

        dwm.write(reg, offset, data);
        client.write("set OK");
    }else if(action == "otp"){
        if (firstSpace < 0) {
            client.println("ERR Invalid format. Use: otp <reg>");
            return;
        }

        int addr    = cmd.substring(firstSpace + 1).toInt();

        // readRegisterBytes(reg, offset, buffer, numBytes);
        uint32_t value = dwm.readOTP(addr);

        // Send bytes back
        client.write((uint8_t*)&value, sizeof(value));
    }else if(action == "stage"){
        uint32_t value = curr_stage;
        Serial.printf("stage: %d\n", curr_stage);
        client.write((uint8_t*)&value, sizeof(value));
    }
    else {
        client.println("ERR Unknown command");
    }
}

unsigned long sentmillis = 0;

void loop()
{
    AnchorData *currentAnchor = getCurrentAnchor();
    int currentAnchorId = getCurrentAnchorId();

    if (USEWIFI && !client.connected()) {
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

    switch (curr_stage)
    {
    case 0: // Start ranging with current target
        // Reset timing measurements for current anchor
        currentAnchor->t_roundA = 0;
        currentAnchor->t_replyA = 0;

        dwm.ds_sendFrame(1, TAG_ID, currentAnchorId);
        currentAnchor->tx = dwm.readTXTimestamp();
        curr_stage = 1;
        sentmillis = millis();
        break;

    case 1: // Await first response
        if (rx_status = dwm.receivedFrameSucc())
        {
            dwm.clearSystemStatus();
            if (rx_status == 1)
            {
                if (dwm.ds_isErrorFrame())
                {
                    Serial.print("[WARNING] Error frame from Anchor ");
                    Serial.print(currentAnchorId);
                    Serial.print("! Signal strength: ");
                    Serial.print(dwm.getSignalStrength());
                    Serial.println(" dBm");
                    curr_stage = 0;
                }
                else if (dwm.ds_getStage() != 2)
                {
                    Serial.print(millis());
                    Serial.print(": ");
                    Serial.print("[WARNING] Unexpected stage from Anchor ");
                    Serial.print(currentAnchorId);
                    Serial.print(": ");
                    Serial.println(dwm.ds_getStage());
                    dwm.ds_sendErrorFrame();
                    curr_stage = 0;
                }
                else
                {
                    curr_stage = 2;
                }
            }
            else
            {
                Serial.print(millis());
                Serial.print(": ");
                Serial.print("[ERROR] Receiver Error stage 2 from Anchor ");
                Serial.println(currentAnchorId);
                dwm.clearSystemStatus();
                curr_stage = 0;
            }
        }else{
            if(millis() - sentmillis > 500){
                dwm.clearSystemStatus();
                curr_stage = 0;
                Serial.println("RX timeout");
            }
        }
        break;

    case 2: // Response received. Send second ranging
        currentAnchor->rx = dwm.readRXTimestamp();
        dwm.ds_sendFrame(3, TAG_ID, currentAnchorId);

        currentAnchor->t_roundA = currentAnchor->rx - currentAnchor->tx;
        currentAnchor->tx = dwm.readTXTimestamp();
        currentAnchor->t_replyA = currentAnchor->tx - currentAnchor->rx;
        sentmillis = millis();

        curr_stage = 3;
        break;

    case 3: // Await second response
        if (rx_status = dwm.receivedFrameSucc())
        {
            dwm.clearSystemStatus();
            if (rx_status == 1)
            {
                if (dwm.ds_isErrorFrame())
                {
                    Serial.print("[WARNING] Error frame from Anchor ");
                    Serial.println(currentAnchorId);
                    curr_stage = 0;
                }
                else
                {
                    currentAnchor->clock_offset = dwm.getRawClockOffset();
                    curr_stage = 4;
                }
            }
            else
            {
                Serial.print(millis());
                Serial.print(": ");
                Serial.print("[ERROR] Receiver Error stage 3 from Anchor ");
                Serial.println(currentAnchorId);
                dwm.clearSystemStatus();
                curr_stage = 0;
            }
        }else{
            if(millis() - sentmillis > 100){
                dwm.clearSystemStatus();
                curr_stage = 0;
                Serial.print(millis());
                Serial.print(": ");
                Serial.println("RX timeout");
            }
        }
        break;

    case 4: // Response received. Calculating results
    {
        int ranging_time = dwm.ds_processRTInfo(
            currentAnchor->t_roundA,
            currentAnchor->t_replyA,
            dwm.read(0x12, 0x04), // reading receive buffer
            dwm.read(0x12, 0x08),
            currentAnchor->clock_offset); // time of flight in clock pulses

        currentAnchor->distance = dwm.convertToCM(ranging_time);
        currentAnchor->signal_strength = dwm.getSignalStrength();
        currentAnchor->fp_signal_strength = dwm.getFirstPathSignalStrength();
        updateFilteredDistance(*currentAnchor);
    }

        // Print current distances
        // printAllDistances();

        // Send data over WiFi if all anchors have valid data
        if (allAnchorsHaveValidData())
        {
            sendData();
        }

        // Switch to next anchor
        switchToNextAnchor();
        curr_stage = 0;
        break;

    default:
        Serial.print("Entered stage (");
        Serial.print(curr_stage);
        Serial.println("). Reverting back to stage 0");
        curr_stage = 0;
        break;
    }
}