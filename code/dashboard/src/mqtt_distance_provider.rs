use bevy::{platform::collections::HashMap, prelude::*};
use rumqttc::{Client, Event, MqttOptions, Packet, QoS};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::triangulation::{ActiveDistanceProvider, DistanceMeasurement, DistanceProviderKind};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Resource that configures which MQTT topics map to which anchor indices.
/// By default it subscribes to `uwb/triangulation/#` and expects JSON payloads
/// of `{ "anchor_index": 0, "distance": 1.23 }`.
#[derive(Resource, Clone)]
pub struct MqttDistanceProviderConfig {
    pub client_id: String,
    pub host: String,
    pub port: u16,
    pub topic: String,
}

impl Default for MqttDistanceProviderConfig {
    fn default() -> Self {
        Self {
            client_id: "bevy_triangulation_mqtt".into(),
            host: "localhost".into(),
            port: 1883,
            topic: "uwb/anchormsg/#".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug, Clone)]
struct MqttDistancePayload {
    anchor_id: usize,
    tag_id: usize,
    distance: f32,
}

/// Shared buffer between the MQTT background thread and bevy systems.
#[derive(Resource)]
struct MqttDistanceReceiver {
    payloads: Arc<Mutex<Vec<MqttDistancePayload>>>,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Bevy plugin that connects to an MQTT broker, receives distance payloads,
/// and emits [`DistanceMeasurement`] events so the triangulation system can
/// consume them.
///
/// # Usage
/// ```rust,ignore
/// app.add_plugins(MqttDistanceProviderPlugin);
/// // optionally insert a custom config before the plugin runs:
/// app.insert_resource(MqttDistanceProviderConfig { host: "192.168.1.5".into(), ..default() });
/// ```
pub struct MqttDistanceProviderPlugin;

impl Plugin for MqttDistanceProviderPlugin {
    fn build(&self, app: &mut App) {
        // Ensure a default config exists (user can override by inserting
        // their own MqttDistanceProviderConfig before adding this plugin).
        app.init_resource::<MqttDistanceProviderConfig>();

        app.add_systems(Startup, setup_mqtt_connection)
            .add_systems(Update, forward_mqtt_to_events);
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn setup_mqtt_connection(
    mut commands: Commands,
    config: Res<MqttDistanceProviderConfig>,
    mut provider: ResMut<ActiveDistanceProvider>,
) {
    // Register ourselves in the provider list so the UI shows "MQTT".
    if !provider.available.contains(&DistanceProviderKind::Mqtt) {
        provider.available.push(DistanceProviderKind::Mqtt);
    }

    let mut opts = MqttOptions::new(&config.client_id, &config.host, config.port);
    opts.set_keep_alive(std::time::Duration::from_secs(5));

    let (client, mut connection) = Client::new(opts, 64);
    client
        .subscribe(&config.topic, QoS::AtMostOnce)
        .expect("MQTT subscribe failed");

    let payloads: Arc<Mutex<Vec<MqttDistancePayload>>> = Arc::new(Mutex::new(Vec::new()));
    let payloads_clone = payloads.clone();

    thread::spawn(move || {
        for notification in connection.iter() {
            if let Ok(Event::Incoming(Packet::Publish(publish))) = notification {
                if let Ok(text) = std::str::from_utf8(&publish.payload) {
                    if let Ok(payload) = serde_json::from_str::<MqttDistancePayload>(text) {
                        if let Ok(mut buf) = payloads_clone.lock() {
                            buf.push(payload);
                        }
                    }
                }
            }
        }
    });

    commands.insert_resource(MqttDistanceReceiver { payloads });
}

/// Drains the shared buffer and emits [`DistanceMeasurement`] events.
fn forward_mqtt_to_events(
    receiver: Option<Res<MqttDistanceReceiver>>,
    provider: Res<ActiveDistanceProvider>,
    mut events: MessageWriter<DistanceMeasurement>,
    mut tracker: Local<HashMap<(usize, usize), f64>>,
    time: Res<Time>,
) {
    if provider.kind != DistanceProviderKind::Mqtt {
        tracker.clear();
        return;
    }

    let now = time.elapsed_secs_f64();

    if let Some(receiver) = receiver {
        if let Ok(mut buf) = receiver.payloads.lock() {
            for payload in buf.drain(..) {
                let key = (payload.anchor_id, payload.tag_id);
                tracker.insert(key, now);
                events.write(DistanceMeasurement {
                    anchor_id: payload.anchor_id,
                    tag_id: payload.tag_id,
                    distance: Some(payload.distance / 100.0),
                });
            }
        }
    }

    tracker.retain(|&(anchor_id, tag_id), last_seen| {
        if now - *last_seen > 2.0 {
            events.write(DistanceMeasurement {
                anchor_id,
                tag_id,
                distance: None,
            });
            false
        } else {
            true
        }
    });
}
