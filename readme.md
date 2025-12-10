# 12UL next gen UWB tracking

### Developing a multi tag UWB realtime tracking system around the Qorvo DWM3000EVB for potential use in future 12Urenloop editions.

## Goals
[x] Ranging between 2 modules
[ ] Improving range (ideally ~30 meters)
[ ] Multi Tag (this is a big one!)
[ ] Triangulation


## code/controlserver
For sending commands to the ESP32 (reading and writing registers and OTP)

## code/ESP32/dwm-debug.ino
code from [CircuitDigest](https://github.com/Circuit-Digest/ESP32-DWM3000-UWB-Indoor-RTLS-Tracker), with extra comments and logic to receive commands from server.
