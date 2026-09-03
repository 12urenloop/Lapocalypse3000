#![feature(default_field_values)]

mod amqp_distance_provider;
mod amqp_position_publisher;
mod config;
mod deformable_image;
mod ffmpeg;
mod log_distance_provider;
mod mqtt_distance_provider;
mod rate_monitor;
mod serial_distance_provider;
mod simulated_distance_provider;
mod triangulation;
mod udp_distance_provider;
mod ui;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use deformable_image::DeformableImagePlugin;
use ffmpeg::FfmpegPlugin;
use mqtt_distance_provider::MqttDistanceProviderPlugin;
use serial_distance_provider::SerialDistanceProviderPlugin;
use triangulation::TriangulationPlugin;
use udp_distance_provider::UdpDistanceProviderPlugin;

use simulated_distance_provider::SimulatedDistanceProviderPlugin;

use crate::{config::ConfigPlugin, ui::UiPlugin};

fn main() {
    println!("starting");
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        // .add_plugins(DashboardPlugin)
        // .add_plugins(RegistryPlugin::default())
        .add_plugins(DeformableImagePlugin)
        .add_plugins(ConfigPlugin)
        .add_plugins(TriangulationPlugin)
        .add_plugins(MqttDistanceProviderPlugin)
        // .add_plugins(AmqpDistanceProviderPlugin)
        .add_plugins(FfmpegPlugin)
        .add_plugins(log_distance_provider::LogDistanceProviderPlugin)
        .add_plugins(SimulatedDistanceProviderPlugin)
        .add_plugins(UdpDistanceProviderPlugin)
        .add_plugins(SerialDistanceProviderPlugin)
        .add_plugins(UiPlugin)
        // .add_plugins(AmqpPositionPublisherPlugin)
        // .insert_resource(MqttReceiver { messages })
        .add_systems(Startup, world_setup)
        // .add_systems(Startup, (describe_metrics /*create_dashboard*/,))
        // .add_systems(Update, (update_metrics, process_mqtt_messages))
        .run();
    println!("started");
}

#[derive(Component)]
pub struct MainCamera {}
fn world_setup(mut commands: Commands) {
    commands.spawn((Camera2d::default(), MainCamera {}));
    // commands.spawn((
    //     Camera3d::default(),
    //     Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    // ));
}
