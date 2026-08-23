#pragma once
#include <WiFi.h>
#include <WiFiClient.h>
#include <Dw3000/src/dw3000.h>
#include <types/taginfo.hpp>
#include <env/wificonfig.hpp>
#include <PubSubClient.h>
#include <uwb/UWB_common.hpp>


#ifndef USEWIFI
#define USEWIFI true
#endif

#ifndef TAG_ID
#define TAG_ID 1
#endif

#ifndef ANCHOR_ID
#define ANCHOR_ID 0
#endif

WiFiClient client;
PubSubClient mqttclient(client);

void callback(char* topic, byte* message, unsigned int length) {}

const String clientID = "Lapocalypse-A-" + String(ANCHOR_ID);

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

  Serial.println("");
  Serial.println("WiFi connected");
  Serial.println("IP address: ");
  Serial.println(WiFi.localIP());
}

void mqtt_setup() {
  if(USEWIFI){
    setup_wifi();
    mqttclient.setServer(host, 1883);
    mqttclient.setCallback(callback);
  }
}


void sendData(byte* anchorIds, double* distances)
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

      for(int i = 0; i < N_TAGS; i++){
        String data = "{\"anchor_id\":" + String(ANCHOR_ID) + ", \"tag_id\":" + String(i + 1) + ", \"distance\":" + String(distances[i]) + "}";  
        if(USEWIFI) mqttclient.publish("uwb/anchormsg/test", data.c_str());
        Serial.print(millis());
        Serial.print(": ");
        Serial.println(data);
      }

      // if(USEWIFI) mqttclient.publish("uwb/anchormsg/test", data.c_str());
      // if(USEWIFI) client.print(data);
  
      // For debugging, print the JSON to serial
      // Serial.println("Sent JSON data:");
    }
}

void reconnect() {
  // Loop until we're reconnected
  while (!mqttclient.connected()) {
    Serial.print("Attempting MQTT connection...");
    // Attempt to connect
    if (mqttclient.connect(clientID.c_str())) {
      Serial.println("connected");
      // Subscribe
    //   mqttclient.subscribe("esp32/output");
    } else {
      Serial.print("failed, rc=");
      Serial.print(mqttclient.state());
      Serial.println(" try again in 5 seconds");
      // Wait 5 seconds before retrying
      delay(5000);
    }
  }
}

void mqtt_loop(){
    if(USEWIFI){
      if (!mqttclient.connected()) {
          reconnect();
      }
      mqttclient.loop();
    }
}