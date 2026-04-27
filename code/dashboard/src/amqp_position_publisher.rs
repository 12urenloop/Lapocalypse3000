use bevy::prelude::*;
use lapin::{
    BasicProperties, Connection, ConnectionProperties, ExchangeKind,
    options::{BasicPublishOptions, ExchangeDeclareOptions},
    types::FieldTable,
};
use serde::Serialize;
use std::thread;
use tokio::sync::mpsc;

use crate::triangulation::PositionMessage;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Resource, Clone)]
pub struct AmqpPositionPublisherConfig {
    pub uri: String,
    pub exchange: String,
    pub routing_key_prefix: String,
}

impl Default for AmqpPositionPublisherConfig {
    fn default() -> Self {
        Self {
            uri: "amqp://uwb:uwb@localhost:5672".into(),
            exchange: "uwb.positions".into(),
            routing_key_prefix: "position.tag.".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct PositionPayload {
    #[serde(rename = "tag_id")]
    tag_id: String,
    x: Option<f32>,
    y: Option<f32>,
}

#[derive(Resource)]
struct PositionSender {
    tx: mpsc::UnboundedSender<PositionMessage>,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct AmqpPositionPublisherPlugin;

impl Plugin for AmqpPositionPublisherPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AmqpPositionPublisherConfig>()
            .add_systems(Startup, setup_amqp_publisher)
            .add_systems(Update, publish_position_messages);
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn setup_amqp_publisher(mut commands: Commands, config: Res<AmqpPositionPublisherConfig>) {
    let (tx, mut rx) = mpsc::unbounded_channel::<PositionMessage>();
    let cfg = config.clone();

    thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(err) => {
                eprintln!("[AMQP Publisher] runtime creation failed: {err}");
                return;
            }
        };

        rt.block_on(async move {
            eprintln!("[AMQP Publisher] connecting to {}", cfg.uri);

            let conn = match Connection::connect(&cfg.uri, ConnectionProperties::default()).await {
                Ok(c) => c,
                Err(err) => {
                    eprintln!("[AMQP Publisher] connection failed: {err}");
                    return;
                }
            };

            let channel = match conn.create_channel().await {
                Ok(ch) => ch,
                Err(err) => {
                    eprintln!("[AMQP Publisher] create channel failed: {err}");
                    return;
                }
            };

            if let Err(err) = channel
                .exchange_declare(
                    &cfg.exchange,
                    ExchangeKind::Topic,
                    ExchangeDeclareOptions {
                        durable: false,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await
            {
                eprintln!("[AMQP Publisher] exchange declare failed: {err}");
                return;
            }

            eprintln!(
                "[AMQP Publisher] ready to publish to exchange '{}'",
                cfg.exchange
            );

            while let Some(msg) = rx.recv().await {
                let payload = PositionPayload {
                    tag_id: msg.tag_id.to_string(),
                    x: msg.position.map(|p| p.x),
                    y: msg.position.map(|p| p.y),
                };

                let routing_key = format!("{}{}", cfg.routing_key_prefix, msg.tag_id);

                if let Ok(json) = serde_json::to_vec(&payload) {
                    let _ = channel
                        .basic_publish(
                            &cfg.exchange,
                            &routing_key,
                            BasicPublishOptions::default(),
                            &json,
                            BasicProperties::default(),
                        )
                        .await;
                }
            }

            eprintln!("[AMQP Publisher] publishing stream ended");
        });
    });

    commands.insert_resource(PositionSender { tx });
}

fn publish_position_messages(
    mut messages: MessageReader<PositionMessage>,
    sender: Option<Res<PositionSender>>,
) {
    if let Some(sender) = sender {
        for msg in messages.read() {
            // Send to the background tokio task
            let _ = sender.tx.send(PositionMessage {
                tag_id: msg.tag_id,
                position: msg.position,
            });
        }
    }
}
