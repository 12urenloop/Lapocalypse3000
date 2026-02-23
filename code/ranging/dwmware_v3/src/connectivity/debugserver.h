#include <WiFi.h>
#include <WiFiClient.h>
#include <Dw3000/src/dw3000.h>
#include <types/taginfo.h>
#include <env/wificonfig.h>

#ifndef USEWIFI
#define USEWIFI true
#endif

#ifndef TAG_ID
#define TAG_ID 1
#endif

#ifndef ANCHOR_ID
#define ANCHOR_ID 0
#endif

const int port = 7007; // Choose a port number
WiFiClient client;
bool wifiConnected = false;

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

// timing recording variables
unsigned long start, end = 0;

void handleCommand(const String &cmd)
{
    // Tokenize
    int firstSpace = cmd.indexOf(' ');
    int secondSpace = cmd.indexOf(' ', firstSpace + 1);
    int thirdSpace = cmd.indexOf(' ', secondSpace + 1);

    String action = cmd.substring(0, firstSpace);

    if (action == "get")
    {
        if (firstSpace < 0 || secondSpace < 0)
        {
            client.println("ERR Invalid format. Use: get <reg> <offset>");
            return;
        }

        int reg = cmd.substring(firstSpace + 1, secondSpace).toInt();
        int offset = cmd.substring(secondSpace + 1).toInt();

        // readRegisterBytes(reg, offset, buffer, numBytes);
        // uint32_t value = dwm.read(reg, offset);
        uint32_t value = dwt_read32bitreg((reg << 12) + offset);

        // Send bytes back
        client.write((uint8_t *)&value, sizeof(value));
    }
    else if (action == "set")
    {
        if (firstSpace < 0 || secondSpace < 0 || thirdSpace < 0)
        {
            client.println("ERR Invalid format. Use: set <reg> <offset>");
            return;
        }

        int reg = cmd.substring(firstSpace + 1, secondSpace).toInt();
        int offset = cmd.substring(secondSpace + 1, thirdSpace).toInt();

        uint32_t data = cmd.substring(thirdSpace + 1).toInt();

        // dwm.write(reg, offset, data);
        dwt_write32bitreg((reg << 12) + offset, data);
        client.write("set OK");
    }
    else if (action == "otp")
    {
        if (firstSpace < 0)
        {
            client.println("ERR Invalid format. Use: otp <reg>");
            return;
        }

        int addr = cmd.substring(firstSpace + 1).toInt();

        // readRegisterBytes(reg, offset, buffer, numBytes);
        // uint32_t value = dwm.readOTP(addr);
        uint32_t value = 0xBEEF;

        // Send bytes back
        client.write((uint8_t *)&value, sizeof(value));
    }
    else
    {
        client.println("ERR Unknown command");
    }
}

void ensureConnection(){
    if (!wifiConnected)
    {
        connectToWiFi();
        if (!wifiConnected)
            return;
    }

    if (!client.connected())
    {
        Serial.println("Disconnected. Reconnecting...");
        while (!client.connect(host, port))
        {
            delay(500);
        }
        Serial.println("connected!");
    }
}

void diagnostic()
{
    for (int base = 0; base <= 10; base++)
    {
        for (int sub = 0; sub <= 0x68; sub += 4)
        {
            int result = dwt_read32bitreg(base << 12 + sub);
            Serial.printf("%02x:%02x = %#010x\n", base, sub, result);
        }
    }
}

void sendData(int numtags, TagInfo taginfos[])
{
    ensureConnection();

    // Create JSON structure dynamically based on number of anchors
    String data = "{\"anchor_id\":" + String(ANCHOR_ID) + ",\"tags\":{";

    for (int i = 0; i < numtags; i++)
    {
        TagInfo tag = taginfos[i];
        data += "\"T" + String(tag.tagID) + "\":{";
        data += "\"distance\":" + String(tag.distance * 100.0, 2);
        // data += ",\"raw\":" + String(anchors[i].distance, 2) + ",";
        // data += "\"rssi\":" + String(anchors[i].signal_strength, 2) + ",";
        // data += "\"fp_rssi\":" + String(anchors[i].fp_signal_strength, 2) + ",";
        // data += "\"round_time\":" + String(anchors[i].t_roundA) + ",";
        // data += "\"reply_time\":" + String(anchors[i].t_replyA) + ",";
        // data += "\"clock_offset\":" + String((double)dwm.getClockOffset(anchors[i].clock_offset), 6);
        data += "}";

        // Add comma if not the last anchor
        if (i < numtags - 1)
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

void debugserver_loop()
{
    if(USEWIFI){
        ensureConnection();    
    
        if (client.available())
        {
            String command = client.readStringUntil('\n');
            command.trim();
    
            if (command.length() > 0)
            {
                Serial.println("Received command: " + command);
                handleCommand(command);
            }
        }
    }
}