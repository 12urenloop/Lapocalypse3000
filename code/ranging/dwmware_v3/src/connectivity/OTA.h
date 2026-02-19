#include <env/wificonfig.h>

#if defined(ESP8266)
  #include <ESP8266WiFi.h>
  #include <WiFiClient.h>
  #include <ESP8266WebServer.h>
#elif defined(ESP32)
  #include <WiFi.h>
  #include <WiFiClient.h>
  #include <WebServer.h>
#endif

#include <ElegantOTA.h>

#if defined(ESP8266)
  ESP8266WebServer server(80);
#elif defined(ESP32)
  WebServer server(80);
#endif

void OTA_setup(){
    WiFi.mode(WIFI_STA);
    WiFi.begin(ssid, password);

    // Wait for connection
    while (WiFi.status() != WL_CONNECTED) {
        delay(200);
        Serial.print(".");
    }

    server.on("/", []() {
        server.send(200, "text/plain", "Hi! This is ElegantOTA Demo.");
    });

    ElegantOTA.begin(&server);    // Start ElegantOTA
}

void OTA_loop(){
    server.handleClient();
    ElegantOTA.loop();
}