# Dashboard Codebase Summary

## What This Project Does

This is a Bevy-based desktop dashboard for UWB-style distance data.

It has two primary responsibilities:

1. Collect distance and telemetry data from MQTT and publish metrics.
2. Triangulate/multilaterate 2D positions from any number of anchor distances and visualize anchors, tag estimates, and radii in real time.

The UI is built with `bevy_egui`, and rendering uses Bevy gizmos.

## High-Level Architecture

The app is composed of plugins and systems that run in Bevy schedules:

- `DefaultPlugins` for core engine/runtime
- `EguiPlugin` for UI
- `RegistryPlugin` + `DashboardPlugin` (`bevy_metrics_dashboard`) for metrics display
- `TriangulationPlugin` for triangulation/multilateration state, event consumption, UI, and drawing
- `MqttDistanceProviderPlugin` for provider-style distance ingestion via MQTT

## Source File Structure

### `src/main.rs`

Application entrypoint.

- Defines legacy MQTT telemetry structs/resources
- Starts an MQTT thread subscribed to `uwb/distance`
- Builds Bevy app and registers plugins

### `src/triangulation.rs`

Core triangulation domain logic and visualization.

Key types:

- `DistanceMeasurement` (`Event`): normalized event contract for distance providers
- `DistanceProviderKind`: provider selection enum (`Manual`, `Mqtt`)
- `ActiveDistanceProvider` (`Resource`): selected provider and available provider list
- `TagState`: per-tag tracking state (distances map, solutions, estimated position)
- `TriangulationState` (`Resource`): global state containing anchors map and tagstates

Key systems:

- `consume_distance_events`: Reads `DistanceMeasurement` events into per-tag `TagState` entries
- `triangulation_ui`: Egui primary context pass for provider selection, anchor/distance editing, and solving
- `draw_triangulation`: Renders axes/grid, anchors (with pseudo-random coloring), optional radii, estimated points, and connector lines using Gizmos

Math:

- `trilaterate_2d`: Exact circle-circle intersection in 2D (for exactly 2 anchors)
- `multilaterate_least_squares`: Gradient descent approach to minimize squared errors for arbitrary number of anchors (> 2)

### `src/mqtt_distance_provider.rs`

MQTT-backed implementation of the generalized distance provider pattern.

- `MqttDistanceProviderPlugin`: Initializes config and connects to MQTT
- Subscribes to `uwb/anchormsg/#` topics, deserializes payloads, and emits `DistanceMeasurement` events continuously
