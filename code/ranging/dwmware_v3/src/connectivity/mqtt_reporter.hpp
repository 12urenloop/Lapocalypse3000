#include <WiFi.h>
#include <WiFiClient.h>
#include <Dw3000/src/dw3000.h>
#include <types/taginfo.hpp>
#include <env/wificonfig.hpp>
#include <PubSubClient.h>

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
PubSubClient mqttclient(client);
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

    mqttclient.setServer(host, 1883);
}

// timing recording variables
unsigned long start, end = 0;

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
  setup_wifi();
  mqttclient.setServer(host, 1883);
  mqttclient.setCallback(callback);
}


void sendData(TagInfo taginfo)
{
    // ensureConnection();

    // Create JSON structure dynamically based on number of anchors
    String data = "{\"anchor_id\":" + String(ANCHOR_ID) + ",\"tag_id\":" + String(taginfo.tagID) + ",\"distance\":" + String(taginfo.distance * 100.0, 2) + "}";

    if(USEWIFI) mqttclient.publish("uwb/anchormsg/test", data.c_str());
    // if(USEWIFI) client.print(data);

    // For debugging, print the JSON to serial
    // Serial.println("Sent JSON data:");
    Serial.print(millis());
    Serial.print(": ");
    Serial.println(data);
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
    if (!mqttclient.connected()) {
        reconnect();
    }
    mqttclient.loop();
}