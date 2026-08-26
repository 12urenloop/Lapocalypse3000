#pragma once
#include <WiFi.h>
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

#define UDPPORT 5000

void callback(char* topic, byte* message, unsigned int length) {}

const String clientID = "Lapocalypse-A-" + String(ANCHOR_ID);
WiFiUDP udp;

void setup_wifi() {
  delay(10);
  // We start by connecting to a WiFi network
  Serial.println();
  Serial.print("Connecting to ");
  Serial.println(ssid);

  WiFi.begin(ssid, password);

  while (WiFi.status() != WL_CONNECTED) {
    delay(100);
    Serial.print(".");
  }

  WiFi.setSleep(false);

  Serial.println("");
  Serial.println("WiFi connected");
  Serial.println("IP address: ");
  Serial.println(WiFi.localIP());

  udp.begin(12345);  // Local UDP port; can be arbitrary
}

void udp_setup() {
  if(USEWIFI){
    setup_wifi();
  }
}


void sendData(byte* anchorIds, TagState* distances)
{
    if(USEWIFI){
      // ensureConnection();
  
      // Create JSON structure dynamically based on number of anchors
      // String data = "{\"anchor_id\":" + String(ANCHOR_ID) + ",\"tag_id\":" + String(taginfo.tagID) + ",\"distance\":" + String(taginfo.distance * 100.0, 2) + "}";
  
      // String data = "{\"anchor_id\":" + String(ANCHOR_ID) + ", \"tags\":{";

      //   for(int i = 0; i < N_TAGS; i++){
      //     data += "\"" + String(anchorIds[i]) + "\":{";
      //       data += "\"distance\":" + String(distances[i]);
      //     data += "}";
      //   }

      // data += "}";

      String data = String(ANCHOR_ID);
      for(int i = 0; i < N_TAGS; i++){
        if(distances[i].consumed) continue;
        // String data = "{\"anchor_id\":" + String(ANCHOR_ID) + ", \"tag_id\":" + String(i + 1) + ", \"distance\":" + String(distances[i]) + "}\n";  
        data += " | " + String(i + 1) + "=" + String(distances[i].distance) + "@" + String(distances[i].rollovers) + "-" + String(distances[i].timestamp);
        distances[i].consumed = true;
      }
      data += "\n";
      udp.beginPacket(host, UDPPORT + ANCHOR_ID);
      udp.print(data);
      udp.endPacket();
      Serial.print(millis());
      Serial.print(": ");
      Serial.println(data);

      // if(USEWIFI) mqttclient.publish("uwb/anchormsg/test", data.c_str());
      // if(USEWIFI) client.print(data);
  
      // For debugging, print the JSON to serial
      // Serial.println("Sent JSON data:");
    }
}