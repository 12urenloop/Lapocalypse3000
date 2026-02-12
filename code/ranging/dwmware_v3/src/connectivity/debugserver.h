#include <WiFi.h>
#include <WiFiClient.h>
#include <Dw3000/src/dw3000.h>


#include <env/wificonfig.h>

#ifndef USEWIFI
#define USEWIFI true
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

void debugserver_loop()
{
    if (USEWIFI && !wifiConnected)
    {
        connectToWiFi();
        if (!wifiConnected)
            return;
    }

    if (USEWIFI && !client.connected() && USEWIFI)
    {
        Serial.println("Disconnected. Reconnecting...");
        while (!client.connect(host, port))
        {
            delay(500);
        }
        Serial.println("connected!");
    }

    if (USEWIFI && client.available())
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