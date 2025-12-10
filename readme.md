# 12UL next gen UWB tracking

### Developing a multi tag UWB realtime tracking system around the Qorvo DWM3000EVB for potential use in future 12Urenloop editions.

## Goals
- [x] Ranging between 2 modules
- [ ] Improving range (ideally ~30 meters)
- [ ] Multi Tag (this is a big one!)
- [ ] Triangulation


## code/controlserver
For sending commands to the ESP32 (reading and writing registers and OTP. Receives metrics (like distance and rssi) from UWB modules and publishes them on MQTT.

## code/dashboard
Bevy app that plots realtime metrics coming from MQTT.

## code/ESP32/tag
tag code from [CircuitDigest](https://github.com/Circuit-Digest/ESP32-DWM3000-UWB-Indoor-RTLS-Tracker), with extra comments and logic to receive commands from server. This allows you to get and set registers and OTP memory interactively.

## code/ESP32/anchor
anchor code from [CircuitDigest](https://github.com/Circuit-Digest/ESP32-DWM3000-UWB-Indoor-RTLS-Tracker), with extra comments and (coming soon) logic to receive commands from server. This allows you to get and set registers and OTP memory interactively.
