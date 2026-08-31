---
type: wiki entrypoint
title: Lapocalypse3000 quickstart
description: Entry point for the Lapocalypse3000 wiki. Use this page to route from a change intent to the owning subsystem, source entrypoints, focused tests, and the narrowest safe validation path.
tags: [quickstart, repository-map, uwb, dwm3000]
---

# Lapocalypse3000 quickstart

This wiki documents the repository’s UWB tracking stack end to end:

<!-- openwiki: broken internal link [./ranging/dwmware_v3/overview.md] file "./ranging/dwmware_v3/overview.md" does not exist. Fix the href or restore the target, then delete this comment. -->
- DWM3000 firmware and uploader tooling in [`ranging/dwmware_v3`](./ranging/dwmware_v3/overview.md)
<!-- openwiki: broken internal link [./dashboard/overview.md] file "./dashboard/overview.md" does not exist. Fix the href or restore the target, then delete this comment. -->
- Rust dashboard and triangulation in [`dashboard`](./dashboard/overview.md)
<!-- openwiki: broken internal link [./aggregation/overview.md] file "./aggregation/overview.md" does not exist. Fix the href or restore the target, then delete this comment. -->
- Aggregation services in [`aggregation`](./aggregation/overview.md)
<!-- openwiki: broken internal link [./controlserver/overview.md] file "./controlserver/overview.md" does not exist. Fix the href or restore the target, then delete this comment. -->
- Control relay runtime in [`controlserver`](./controlserver/overview.md)
<!-- openwiki: broken internal link [./vibeviz/overview.md] file "./vibeviz/overview.md" does not exist. Fix the href or restore the target, then delete this comment. -->
- SvelteKit charting in [`vibeviz`](./vibeviz/overview.md)
<!-- openwiki: broken internal link [./workflows/serial-to-dashboard.md] file "./workflows/serial-to-dashboard.md" does not exist. Fix the href or restore the target, then delete this comment. -->
- Cross-system workflows in [`workflows`](./workflows/serial-to-dashboard.md)

If you are changing behavior, start from the workflow or subsystem page that owns the runtime path, then jump to the narrow page for the exact contract or transport.

## What this repository does

Lapocalypse3000 is a multi-stage UWB tracking stack built around the Qorvo DW3000 platform. Firmware on anchors and tags emits ranging data, aggregation services move that data over RabbitMQ and MQTT, the dashboard computes and visualizes positions, and vibeviz renders recorded range/error data.

The repository also contains a Bun TCP/MQTT control relay and uploader automation for identifying devices and flashing the correct PlatformIO environment.

## Main sections

| Section | What to read first | Why it matters |
| --- | --- | --- |
<!-- openwiki: broken internal link [./ranging/dwmware_v3/overview.md] file "./ranging/dwmware_v3/overview.md" does not exist. Fix the href or restore the target, then delete this comment. -->
| DWM3000 firmware | [`ranging/dwmware_v3/overview.md`](./ranging/dwmware_v3/overview.md) | Owns the device runtime, SS-TWR behavior, serial contract, and board-specific build targets. |
<!-- openwiki: broken internal link [./ranging/dwmware_v3/dwm3000-intricacies.md] file "./ranging/dwmware_v3/dwm3000-intricacies.md" does not exist. Fix the href or restore the target, then delete this comment. -->
| DWM3000 intricacies | [`ranging/dwmware_v3/dwm3000-intricacies.md`](./ranging/dwmware_v3/dwm3000-intricacies.md) | Captures timing, framing, antenna-delay, and calibration caveats that affect correctness. |
<!-- openwiki: broken internal link [./ranging/dwmware_v3/uploader.md] file "./ranging/dwmware_v3/uploader.md" does not exist. Fix the href or restore the target, then delete this comment. -->
| Firmware upload | [`ranging/dwmware_v3/uploader.md`](./ranging/dwmware_v3/uploader.md) | Explains how ports are discovered, devices are identified, and PlatformIO environments are selected. |
<!-- openwiki: broken internal link [./dashboard/overview.md] file "./dashboard/overview.md" does not exist. Fix the href or restore the target, then delete this comment. -->
| Dashboard | [`dashboard/overview.md`](./dashboard/overview.md) | Describes the Bevy app, provider plugins, triangulation, and export path. |
<!-- openwiki: broken internal link [./dashboard/triangulation.md] file "./dashboard/triangulation.md" does not exist. Fix the href or restore the target, then delete this comment. -->
| Triangulation core | [`dashboard/triangulation.md`](./dashboard/triangulation.md) | The shared event/state model that all providers feed. |
<!-- openwiki: broken internal link [./aggregation/overview.md] file "./aggregation/overview.md" does not exist. Fix the href or restore the target, then delete this comment. -->
| Aggregation stack | [`aggregation/overview.md`](./aggregation/overview.md) | Maps publisher, consumer, broker/control tooling, and websocket bridge. |
<!-- openwiki: broken internal link [./controlserver/overview.md] file "./controlserver/overview.md" does not exist. Fix the href or restore the target, then delete this comment. -->
| Control relay | [`controlserver/overview.md`](./controlserver/overview.md) | Documents the TCP/stdin relay and its MQTT topic contract. |
<!-- openwiki: broken internal link [./vibeviz/overview.md] file "./vibeviz/overview.md" does not exist. Fix the href or restore the target, then delete this comment. -->
| Vibeviz | [`vibeviz/overview.md`](./vibeviz/overview.md) | Documents the chart input format and browser-only SvelteKit charting contract. |
<!-- openwiki: broken internal link [./workflows/serial-to-dashboard.md] file "./workflows/serial-to-dashboard.md" does not exist. Fix the href or restore the target, then delete this comment. -->
| End-to-end flows | [`workflows/serial-to-dashboard.md`](./workflows/serial-to-dashboard.md) | Shows where data originates, how it is transformed, and where it is consumed. |

## Fast route from intent to source

| Change intent | Read these pages | Owning source entrypoints | Focused validation |
| --- | --- | --- | --- |
<!-- openwiki: broken internal link [./ranging/dwmware_v3/firmware-entrypoints.md] file "./ranging/dwmware_v3/firmware-entrypoints.md" does not exist. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [./ranging/dwmware_v3/ss-twr-anchor.md] file "./ranging/dwmware_v3/ss-twr-anchor.md" does not exist. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [./ranging/dwmware_v3/ss-twr-tag.md] file "./ranging/dwmware_v3/ss-twr-tag.md" does not exist. Fix the href or restore the target, then delete this comment. -->
| Change anchor/tag firmware behavior | [`ranging/dwmware_v3/firmware-entrypoints.md`](./ranging/dwmware_v3/firmware-entrypoints.md), [`ranging/dwmware_v3/ss-twr-anchor.md`](./ranging/dwmware_v3/ss-twr-anchor.md), [`ranging/dwmware_v3/ss-twr-tag.md`](./ranging/dwmware_v3/ss-twr-tag.md) | `ranging/dwmware_v3/src/main.cpp`, `src/anchor.hpp`, `src/tag.hpp`, `src/uwb/*` | Build the relevant PlatformIO env, confirm serial output shape, and verify the DWM3000 timing assumptions in `dwm3000-intricacies.md`. |
<!-- openwiki: broken internal link [./ranging/dwmware_v3/boards-and-envs.md] file "./ranging/dwmware_v3/boards-and-envs.md" does not exist. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [./ranging/dwmware_v3/uploader.md] file "./ranging/dwmware_v3/uploader.md" does not exist. Fix the href or restore the target, then delete this comment. -->
| Adjust board/env selection or flashing | [`ranging/dwmware_v3/boards-and-envs.md`](./ranging/dwmware_v3/boards-and-envs.md), [`ranging/dwmware_v3/uploader.md`](./ranging/dwmware_v3/uploader.md) | `ranging/dwmware_v3/platformio.ini`, `uploader/auto_upload_by_device_info.ts`, `uploader/getinfo_serial.ts` | Identify a device, map it to an env, and run the upload path for one board. |
<!-- openwiki: broken internal link [./dashboard/triangulation.md] file "./dashboard/triangulation.md" does not exist. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [./dashboard/providers-mqtt.md] file "./dashboard/providers-mqtt.md" does not exist. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [./dashboard/providers-amqp.md] file "./dashboard/providers-amqp.md" does not exist. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [./dashboard/providers-log.md] file "./dashboard/providers-log.md" does not exist. Fix the href or restore the target, then delete this comment. -->
| Tune distance ingestion or triangulation | [`dashboard/triangulation.md`](./dashboard/triangulation.md), [`dashboard/providers-mqtt.md`](./dashboard/providers-mqtt.md), [`dashboard/providers-amqp.md`](./dashboard/providers-amqp.md), [`dashboard/providers-log.md`](./dashboard/providers-log.md) | `dashboard/src/main.rs`, `src/triangulation.rs`, provider modules | Validate provider selection, stale-data handling, and exported positions. |
<!-- openwiki: broken internal link [./aggregation/publisher-bun.md] file "./aggregation/publisher-bun.md" does not exist. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [./aggregation/consumer-simple.md] file "./aggregation/consumer-simple.md" does not exist. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [./aggregation/resilience-and-errors.md] file "./aggregation/resilience-and-errors.md" does not exist. Fix the href or restore the target, then delete this comment. -->
| Change RabbitMQ batching or persistence | [`aggregation/publisher-bun.md`](./aggregation/publisher-bun.md), [`aggregation/consumer-simple.md`](./aggregation/consumer-simple.md), [`aggregation/resilience-and-errors.md`](./aggregation/resilience-and-errors.md) | `aggregation/publisher-bun/src/index.ts`, `consumer-simple/src/index.ts` | Exercise a publish/consume loop and confirm routing keys, buffering, and error events. |
<!-- openwiki: broken internal link [./aggregation/rabbitmq-server.md] file "./aggregation/rabbitmq-server.md" does not exist. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [./workflows/control-and-reconfiguration.md] file "./workflows/control-and-reconfiguration.md" does not exist. Fix the href or restore the target, then delete this comment. -->
| Change control messages or runtime limits | [`aggregation/rabbitmq-server.md`](./aggregation/rabbitmq-server.md), [`workflows/control-and-reconfiguration.md`](./workflows/control-and-reconfiguration.md) | `aggregation/rabbitmq-server/send-control.ts`, publisher control-handling code | Send a control message to one node and verify the runtime limit update and confirm behavior. |
<!-- openwiki: broken internal link [./aggregation/websocket-publisher.md] file "./aggregation/websocket-publisher.md" does not exist. Fix the href or restore the target, then delete this comment. -->
| Change websocket delivery to a UI/client | [`aggregation/websocket-publisher.md`](./aggregation/websocket-publisher.md) | `aggregation/websocket-publisher/src/index.ts` | Verify `/healthz`, `/ws`, message validation, and AMQP reconnect behavior. |
<!-- openwiki: broken internal link [./dashboard/providers-log.md] file "./dashboard/providers-log.md" does not exist. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [./dashboard/ffmpeg-video.md] file "./dashboard/ffmpeg-video.md" does not exist. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [./vibeviz/range-chart.md] file "./vibeviz/range-chart.md" does not exist. Fix the href or restore the target, then delete this comment. -->
| Change recorded-data replay or charting | [`dashboard/providers-log.md`](./dashboard/providers-log.md), [`dashboard/ffmpeg-video.md`](./dashboard/ffmpeg-video.md), [`vibeviz/range-chart.md`](./vibeviz/range-chart.md) | `dashboard/src/log_distance_provider.rs`, `dashboard/src/ffmpeg.rs`, `vibeviz/src/lib/components/RangeChart.svelte` | Load recorded data, verify playback freshness windows, and confirm chart series/file parsing. |
<!-- openwiki: broken internal link [./controlserver/runtime.md] file "./controlserver/runtime.md" does not exist. Fix the href or restore the target, then delete this comment. -->
<!-- openwiki: broken internal link [./workflows/error-observability.md] file "./workflows/error-observability.md" does not exist. Fix the href or restore the target, then delete this comment. -->
| Change the TCP/MQTT relay | [`controlserver/runtime.md`](./controlserver/runtime.md), [`workflows/error-observability.md`](./workflows/error-observability.md) | `controlserver/debug.ts` | Connect a TCP client, send stdin commands, and verify the MQTT topic contract. |

## Key concepts

### DWM3000 and SS-TWR

The firmware uses the DW3000’s single-sided two-way ranging path. The important contracts are:

- anchor and tag builds are selected at compile time by PlatformIO environment macros
- the serial output `= <tag id> <distance in centimeter> mesh <synced time in millis> <unsynced millis>` is the input contract for the aggregation publisher
- the device info probe `AT+GETINFO` / `INFO=` is used by the uploader to map hardware to an env
- timing, antenna delay, and timestamp handling are not optional details; they determine measured range quality

<!-- openwiki: broken internal link [./ranging/dwmware_v3/dwm3000-intricacies.md] file "./ranging/dwmware_v3/dwm3000-intricacies.md" does not exist. Fix the href or restore the target, then delete this comment. -->
Read [`ranging/dwmware_v3/dwm3000-intricacies.md`](./ranging/dwmware_v3/dwm3000-intricacies.md) before changing the ranging loop.

### Dashboard provider architecture

<!-- openwiki: broken internal link [./dashboard/triangulation.md] file "./dashboard/triangulation.md" does not exist. Fix the href or restore the target, then delete this comment. -->
The dashboard is a Bevy app that accepts distance measurements from multiple providers and converts them into triangulated positions. The shared model lives in [`dashboard/triangulation.md`](./dashboard/triangulation.md): provider plugins register themselves, one active provider is selected, and distance events are either consumed or ignored based on that selection.

### Aggregation and control plane

The aggregation stack uses RabbitMQ as the transport spine. The publisher batches serial measurements into `uwb.data`, the consumer persists them and measures latency, the websocket bridge republishes positions to clients, and the control sender publishes updates to `uwb.control`.

### Vibeviz

Vibeviz is a SvelteKit charting app that reads static files from `static/`, not a general-purpose API. The chart component expects specific timestamped line formats and is browser-only because `lightweight-charts` needs the DOM.

## Operational notes

<!-- openwiki: broken internal link [./aggregation/systemd-and-deploy.md] file "./aggregation/systemd-and-deploy.md" does not exist. Fix the href or restore the target, then delete this comment. -->
- Systemd, log rotation, and deployment scripts for the publisher live under [`aggregation/systemd-and-deploy.md`](./aggregation/systemd-and-deploy.md).
- DWM3000 upload automation depends on serial discovery under `/dev/ttyUSB*` and PlatformIO env names in `platformio.ini`.
<!-- openwiki: broken internal link [./architecture/resilience-and-errors.md] file "./architecture/resilience-and-errors.md" does not exist. Fix the href or restore the target, then delete this comment. -->
- Error events are part of the runtime contract, not debug noise. If you change failure handling, update [`architecture/resilience-and-errors.md`](./architecture/resilience-and-errors.md) and the affected subsystem page together.

## Backlog

No intentionally deferred areas are known yet. If you need to leave a topic out, add it to the relevant page’s backlog section with a source anchor and the reason it could not be covered safely.
