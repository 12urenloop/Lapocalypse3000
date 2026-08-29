#![feature(default_field_values)]

mod amqp_distance_provider;
mod amqp_position_publisher;
mod ffmpeg;
mod log_distance_provider;
mod mqtt_distance_provider;
mod rate_monitor;
mod simulated_distance_provider;
mod triangulation;
mod udp_distance_provider;

use amqp_distance_provider::AmqpDistanceProviderPlugin;
use amqp_position_publisher::AmqpPositionPublisherPlugin;
use bevy::prelude::*;
use bevy_egui::EguiPlugin;
// use bevy_metrics_dashboard::{DashboardPlugin, DashboardWindow, RegistryPlugin};
// use metrics::{
//     Unit, counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram,
// };
use mqtt_distance_provider::MqttDistanceProviderPlugin;
use rumqttc::{Client, Event, MqttOptions, Packet, QoS};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::thread;
use triangulation::TriangulationPlugin;
use udp_distance_provider::UdpDistanceProviderPlugin;

use crate::ffmpeg::FfmpegPlugin;
use simulated_distance_provider::SimulatedDistanceProviderPlugin;

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
    println!("starting");
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        // .add_plugins(RegistryPlugin::default())
        // .add_plugins(DashboardPlugin)
        .add_plugins(TriangulationPlugin)
        .add_plugins(MqttDistanceProviderPlugin)
        // .add_plugins(AmqpDistanceProviderPlugin)
        .add_plugins(FfmpegPlugin)
        .add_plugins(log_distance_provider::LogDistanceProviderPlugin)
        .add_plugins(SimulatedDistanceProviderPlugin)
        .add_plugins(UdpDistanceProviderPlugin)
        // .add_plugins(AmqpPositionPublisherPlugin)
        // .insert_resource(MqttReceiver { messages })
        .add_systems(Startup, world_setup)
        // .add_systems(Startup, (describe_metrics /*create_dashboard*/,))
        // .add_systems(Update, (update_metrics, process_mqtt_messages))
        .run();
    println!("started");
}

fn world_setup(mut commands: Commands) {
    commands.spawn(Camera2d::default());
    // commands.spawn((
    //     Camera3d::default(),
    //     Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    // ));
}

// fn describe_metrics() {
//     gauge!("uwb/distance").set(-10.0);
//     gauge!("uwb/raw").set(-10.0);
//     gauge!("uwb/rssi").set(-10.0);
//     gauge!("uwb/fp_rssi").set(-10.0);
//     gauge!("uwb/round_time").set(-10.0);
//     gauge!("uwb/reply_time").set(-10.0);
//     gauge!("uwb/clock_offset").set(-10.0);
// }

// fn create_dashboard(mut commands: Commands) {
//     let dashwin = DashboardWindow::new("Metrics Dashboard");
//     commands.spawn(dashwin);
// }

fn update_metrics() {
    // let mut rng = rand::thread_rng();

    // histogram!("foo").record(rng.gen_range(0.0..10.0));
    // gauge!("foo").set(rng.gen_range(0.0..10.0));
    // counter!("foo").increment(rng.gen_range(0..10));
}

// fn process_mqtt_messages(mqtt_receiver: Res<MqttReceiver>) {
//     if let Ok(mut messages) = mqtt_receiver.messages.lock() {
//         for msg in messages.drain(..) {
//             gauge!("uwb/distance").set(msg.distance);
//             gauge!("uwb/raw").set(msg.raw);
//             gauge!("uwb/rssi").set(msg.rssi);
//             gauge!("uwb/fp_rssi").set(msg.fp_rssi);
//             gauge!("uwb/round_time").set(msg.round_time as f64);
//             gauge!("uwb/reply_time").set(msg.reply_time as f64);
//             gauge!("uwb/clock_offset").set(msg.clock_offset);
//         }
//     }
// }
