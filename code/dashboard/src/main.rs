use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use bevy_metrics_dashboard::{DashboardPlugin, DashboardWindow, RegistryPlugin};
use metrics::{
    counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram, Unit,
};
use rumqttc::{Client, Event, MqttOptions, Packet, QoS};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::thread;


#[derive(Deserialize, Debug)]
struct MqttMessage {
  distance: f64,
  raw: f64,
  rssi: f64,
  fp_rssi: f64,
  round_time: u64,
  reply_time: u64,
  clock_offset: f64,
}

#[derive(Resource)]
struct MqttReceiver {
    messages: Arc<Mutex<Vec<MqttMessage>>>,
}

fn main() {
    // Set up MQTT client
    let mut mqttoptions = MqttOptions::new("bevy_metrics_client", "localhost", 1883);
    mqttoptions.set_keep_alive(std::time::Duration::from_secs(5));

    let (client, mut connection) = Client::new(mqttoptions, 10);
    
    // Subscribe to topic
    client.subscribe("uwb/distance", QoS::AtMostOnce).unwrap();

    // Shared buffer for messages
    let messages: Arc<Mutex<Vec<MqttMessage>>> = Arc::new(Mutex::new(Vec::new()));
    let messages_clone = messages.clone();

    // Spawn thread to handle MQTT events
    thread::spawn(move || {
        for notification in connection.iter() {
            if let Ok(Event::Incoming(Packet::Publish(publish))) = notification {
                if let Ok(payload_str) = std::str::from_utf8(&publish.payload) {
                    if let Ok(msg) = serde_json::from_str::<MqttMessage>(payload_str) {
                        if let Ok(mut msgs) = messages_clone.lock() {
                            msgs.push(msg);
                        }
                    }
                }
            }
        }
    });

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .add_plugins(RegistryPlugin::default())
        .add_plugins(DashboardPlugin)
        .insert_resource(MqttReceiver { messages })
        .add_systems(Startup, (describe_metrics, create_dashboard))
        .add_systems(Update, (update_metrics, process_mqtt_messages))
        .run();
}

fn describe_metrics() {

    gauge!("uwb/distance").set(-10.0);
    gauge!("uwb/raw").set(-10.0);
    gauge!("uwb/rssi").set(-10.0);
    gauge!("uwb/fp_rssi").set(-10.0);
    gauge!("uwb/round_time").set(-10.0);
    gauge!("uwb/reply_time").set(-10.0);
    gauge!("uwb/clock_offset").set(-10.0);
}

fn create_dashboard(
    mut commands: Commands,
) {
    commands.spawn(Camera2d);
    let dashwin = DashboardWindow::new("Metrics Dashboard");
    commands.spawn(dashwin);
}

fn update_metrics() {
    // let mut rng = rand::thread_rng();

    // histogram!("foo").record(rng.gen_range(0.0..10.0));
    // gauge!("foo").set(rng.gen_range(0.0..10.0));
    // counter!("foo").increment(rng.gen_range(0..10));
}

fn process_mqtt_messages(mqtt_receiver: Res<MqttReceiver>) {
    if let Ok(mut messages) = mqtt_receiver.messages.lock() {
        for msg in messages.drain(..) {
            gauge!("uwb/distance").set(msg.distance);
            gauge!("uwb/raw").set(msg.raw);
            gauge!("uwb/rssi").set(msg.rssi);
            gauge!("uwb/fp_rssi").set(msg.fp_rssi);
            gauge!("uwb/round_time").set(msg.round_time as f64);
            gauge!("uwb/reply_time").set(msg.reply_time as f64);
            gauge!("uwb/clock_offset").set(msg.clock_offset);
        }
    }
}
