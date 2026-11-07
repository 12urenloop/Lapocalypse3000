#define ANCHOR_ID 0

#include <connectivity/espnow_reporter.hpp>
#include "serial_commands.hpp"


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

void setup()
{
    Serial.begin(115200);

    setup_espnow();

    esp_now_register_recv_cb(onDataRecv);

    Serial.println("ESP-NOW receiver ready");
}
static String serial_buffer;

void loop()
{
    serial_commands::process(Serial, serial_buffer, INFOSTRING);
}