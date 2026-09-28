#pragma once
#include <WiFi.h>
#include <esp_now.h>
#include <types/taginfo.hpp>
#include <env/wificonfig.hpp>
#include <uwb/UWB_common.hpp>
#include <uwb/SSTWR_initiator_uwbsync.hpp>

#ifndef USEWIFI
#define USEWIFI true
#endif

#ifndef TAG_ID
#define TAG_ID 1
#endif

#ifndef ANCHOR_ID
#define ANCHOR_ID 0
#endif

#define GATEWAYMAC {0x34, 0x86, 0x5D, 0xFD, 0x54, 0x08}
#define ANCHOR1MAC {0xB0, 0xCB, 0xD8, 0xC7, 0x79, 0x48}

#if ANCHOR_ID == 2
uint8_t nextHop[] = ANCHOR1MAC;
#else
uint8_t nextHop[] = GATEWAYMAC;
#endif
constexpr uint8_t CHANNEL = 7;

struct __attribute__((packed)) PacketHeader
{
    uint8_t source;      // Original node ID
    uint8_t destination; // Usually receiver ID
                         // uint8_t  sequence;    // Or uint16_t if needed
                         // uint8_t  hops;        // Incremented by every relay

    // uint8_t  data[12];    // Your actual measurement
};

struct __attribute__((packed)) TagMsg
{
    uint64_t timestamp;
    double distance;
    uint8_t tag_id;
};

struct __attribute__((packed)) Packet
{
    PacketHeader header;
    TagMsg tagmsgs[N_TAGS];
};

void anchorOnReceive(const esp_now_recv_info_t *info, const uint8_t *data, int len)
{
    // Packet recv_packet;
    // memcpy(&recv_packet, data, sizeof(recv_packet));

    // if (packet.destination == MY_ID) {
    //     process_packet(packet);
    //     return;
    // }

    // if (packet.hops >= MAX_HOPS) {
    //     return;
    // }

    // packet.hops++;

    esp_now_send(nextHop,
                 data,
                 len);
}

void setup_espnow()
{
    WiFi.mode(WIFI_STA);
    WiFi.setChannel(CHANNEL);

    if (esp_now_init() != ESP_OK)
    {
        ESP.restart();
    }

    esp_now_peer_info_t peer{};

    memcpy(peer.peer_addr, nextHop, 6);
    peer.channel = CHANNEL;
    peer.encrypt = false;

    esp_now_add_peer(&peer);

    Serial.print("ESP32 MAC Address: ");
    Serial.println(WiFi.macAddress());

    // esp_now_register_recv_cb(onReceive);
}

Packet packet;

void sendData(byte *anchorIds, TagState *distances)
{

    packet.header.source = ANCHOR_ID;
    packet.header.destination = 0xFF;

    int packetind = 0;
    for (int i = 0; i < N_TAGS; i++)
    {
        if (distances[i].consumed)
            continue;
        
        packet.tagmsgs[packetind].distance = distances[i].distance;
        packet.tagmsgs[packetind].timestamp = distances[i].timestamp;
        packet.tagmsgs[packetind].tag_id = i + 1;
        packetind++;
    }

    if(packetind >= 1){
        auto * esp_packet = reinterpret_cast<uint8_t*>(&packet);
        esp_now_send(nextHop, esp_packet, sizeof(PacketHeader) + packetind * sizeof(TagMsg));
    }
}