# 12UL next gen UWB tracking

### Developing a multi tag UWB realtime tracking system around the Qorvo DW3000 platform for potential use in future 12Urenloop editions.

## Goals
- [x] Ranging between 2 modules
- [x] Improving range (ideally ~30 meters)
- [x] Triangulation
- [ ] Multi Tag multi Anchor (this is a big one!)
    - [x] Refactor DW3000 API
    - [x] Test SSTWR (single side two way ranging)
    - [x] Design multi tag method
    - [x] Implement multi tag method
    - [ ] Calibration of positioning
    - [ ] Timing / power measurements
    - [ ] precision / reliability / throughput / latency measurements
- [ ] Integrate into batons
- [ ] Integrate into stations
- [ ] Lap counting/timing/tracking system


## code/ranging/dwmware_v3
Anchor and tag firmware using PlatformIO and Arduino framework, targetting ESP32-Wroom with DWM3000EVB board. The synchronisation between anchors is via ESP-NOW (this will probably change to GPS timing or UWB in the future). 

## code/dashboard
Positioning server and visualization for data coming from UWB modules.
- Plots realtime metrics like RSSI coming from MQTT.
- Triangulates positions and displays the results visually.
- Can simulate realtime data from log files from UWB modules for later analysis

## code/controlserver
For sending commands to anchors and tags to configure UWB modules in realtime (reading and writing registers and OTP. Receives metrics (like distance and rssi) and publishes them on MQTT.

# Project status
### First outdoor test (bad coverage, worst case scenario): [Raw data video here](https://mattermost.zeus.gent/files/bj4pd1zdd7rg5xdo87kudsfhje/public?h=66uNqb_PoHTUd7TrrOXdnlIjC-n7Gycecf7ml2fecYE)
