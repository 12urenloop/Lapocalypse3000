module;

#include <Arduino.h>
#include <WiFi.h>
#include <esp_now.h>

export module gateway;

import espnow_reporter;
import serial_commands;

#define ANCHOR_ID 0

void onDataRecv(const esp_now_recv_info_t *info, const uint8_t *data, int len)
{
    if (len < sizeof(PacketHeader) + 15)
    {
        Serial.printf("Invalid packet size: %d (expected %u)\n",
                      len, sizeof(Packet));
        return;
    }

    Packet packet;
    memcpy(&packet, data, len);

    uint16_t msgs = (len - sizeof(PacketHeader)) / 15;

    Serial.print(packet.header.source);

    for (int i = 0; i < msgs; i++)
    {
        const TagMsg &tag = packet.tagmsgs[i];

        Serial.printf(
            " | %u=%.3f@%llu",
            tag.tag_id,
            tag.distance,
            tag.timestamp
        );
    }
    Serial.println();
}

export void gateway_setup()
{
    Serial.begin(115200);

    setup_espnow();

    esp_now_register_recv_cb(onDataRecv);

    Serial.println("ESP-NOW receiver ready");
}
static String serial_buffer;

void process_serial()
{
    while (Serial.available())
    {
        static String buffer;
        buffer += static_cast<char>(Serial.read());
        if (buffer.endsWith("\n"))
        {
            if (serial_commands::is_get_info_command(buffer.c_str()))
            {
                Serial.print("INFO=");
                Serial.println(INFOSTRING);
            }
            buffer.remove(0);
        }
    }
}

export void gateway_loop()
{
    process_serial();
}