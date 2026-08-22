use bevy::{platform::collections::HashMap, prelude::*};
use lapin::{
    Channel, Connection, ConnectionProperties,
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicQosOptions, QueueBindOptions,
        QueueDeclareOptions,
    },
    types::FieldTable,
};
use serde::Deserialize;
use std::collections::HashMap as StdHashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::triangulation::{ActiveDistanceProvider, DistanceMeasurement, DistanceProviderKind};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Resource that configures RabbitMQ connection/consume settings.
///
/// Default behavior:
/// - Connect to `amqp://uwb:uwb@localhost:5672/%2f`
/// - Consume queue `uwb.data`
/// - Parse payloads shaped like:
///   {
///     "nodeId": "pi-01",
///     "data": [
///       { "tagId": "0", "cm": 53 },
///       ...
///     ]
///   }
///
/// Optional binding behavior:
/// - If `bind_exchange` is `Some`, the consumer will bind `queue` to that
///   exchange with `bind_routing_key`.
#[derive(Resource, Clone)]
pub struct AmqpDistanceProviderConfig {
    pub uri: String,
    pub queue: String,
    pub consumer_tag: String,
    pub bind_exchange: Option<String>,
    pub bind_routing_key: String,
    pub declare_queue: bool,
    pub durablequeue: bool,
}

impl Default for AmqpDistanceProviderConfig {
    fn default() -> Self {
        Self {
            uri: "amqp://uwb:uwb@localhost:5672".into(),
            queue: "uwb.tri".into(),
            consumer_tag: "bevy_triangulation_amqp".into(),
            bind_exchange: Some("uwb.data".into()),
            bind_routing_key: "node.*".into(),
            declare_queue: true,
            durablequeue: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal payload types
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug, Clone)]
struct AmqpEnvelope {
    #[serde(rename = "nodeId")]
    node_id: String,
    data: Vec<AmqpSample>,
}

#[derive(Deserialize, Debug, Clone)]
struct AmqpSample {
    #[serde(rename = "tagId")]
    tag_id: String,
    cm: f32,
}

#[derive(Debug, Clone)]
struct ParsedDistancePayload {
    anchor_id: usize,
    tag_id: usize,
    distance_m: f32,
}

/// Shared buffer between AMQP background thread and Bevy systems.
#[derive(Resource)]
struct AmqpDistanceReceiver {
    payloads: Arc<Mutex<Vec<ParsedDistancePayload>>>,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct AmqpDistanceProviderPlugin;

impl Plugin for AmqpDistanceProviderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AmqpDistanceProviderConfig>()
            .add_systems(Startup, setup_amqp_connection)
            .add_systems(Update, forward_amqp_to_events);
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn setup_amqp_connection(
    mut commands: Commands,
    config: Res<AmqpDistanceProviderConfig>,
    mut provider: ResMut<ActiveDistanceProvider>,
) {
    if !provider.available.contains(&DistanceProviderKind::Amqp) {
        provider.available.push(DistanceProviderKind::Amqp);
    }

    let payloads: Arc<Mutex<Vec<ParsedDistancePayload>>> = Arc::new(Mutex::new(Vec::new()));
    let payloads_clone = payloads.clone();
    let cfg = config.clone();

    thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(err) => {
                eprintln!("[AMQP] runtime creation failed: {err}");
                return;
            }
        };

        rt.block_on(async move {
            eprintln!("[AMQP] connecting to {}", cfg.uri);

            let conn = match Connection::connect(&cfg.uri, ConnectionProperties::default()).await {
                Ok(c) => c,
                Err(err) => {
                    eprintln!("[AMQP] connection failed: {err}");
                    return;
                }
            };

            let channel = match conn.create_channel().await {
                Ok(ch) => ch,
                Err(err) => {
                    eprintln!("[AMQP] create channel failed: {err}");
                    return;
                }
            };

            if let Err(err) = setup_queue_and_binding(&channel, &cfg).await {
                eprintln!("[AMQP] queue/bind setup failed: {err}");
                return;
            }

            if let Err(err) = channel.basic_qos(32, BasicQosOptions::default()).await {
                eprintln!("[AMQP] qos setup failed: {err}");
                return;
            }

            let mut consumer = match channel
                .basic_consume(
                    &cfg.queue,
                    &cfg.consumer_tag,
                    BasicConsumeOptions::default(),
                    FieldTable::default(),
                )
                .await
            {
                Ok(c) => c,
                Err(err) => {
                    eprintln!("[AMQP] consume setup failed: {err}");
                    return;
                }
            };

            eprintln!(
                "[AMQP] consuming queue='{}' consumer_tag='{}' bind_exchange={:?} bind_routing_key='{}'",
                cfg.queue, cfg.consumer_tag, cfg.bind_exchange, cfg.bind_routing_key
            );

            use futures_util::StreamExt;

            while let Some(delivery_result) = consumer.next().await {
                let delivery = match delivery_result {
                    Ok(d) => d,
                    Err(err) => {
                        eprintln!("[AMQP] delivery stream error: {err}");
                        continue;
                    }
                };

                match std::str::from_utf8(&delivery.data) {
                    Ok(text) => match serde_json::from_str::<AmqpEnvelope>(text) {
                        Ok(msg) => {
                            let anchor_id: usize = msg
                                .node_id
                                .chars()
                                .filter(|c| c.is_ascii_digit())
                                .collect::<String>()
                                .parse::<u32>()
                                .map(|v| v as usize)
                                .unwrap_or_else(|parse_err| {
                                    eprintln!(
                                        "[AMQP] invalid nodeId='{}': {}",
                                        msg.node_id, parse_err
                                    );
                                    0
                                });
                            // let anchor_id = &msg.node_id;

                            // only use the last sample per tag id in each message
                            let mut last_by_tag: StdHashMap<usize, f32> = StdHashMap::new();
                            for sample in msg.data {
                                match sample.tag_id.parse::<usize>() {
                                    Ok(tag_id) => {
                                        last_by_tag.insert(tag_id, sample.cm / 100.0);
                                    }
                                    Err(parse_err) => {
                                        eprintln!(
                                            "[AMQP] invalid tagId='{}': {}",
                                            sample.tag_id, parse_err
                                        );
                                    }
                                }
                            }

                            if last_by_tag.is_empty() {
                                eprintln!(
                                    "[AMQP] parsed message for nodeId='{}' but no usable samples",
                                    msg.node_id
                                );
                            }

                            if let Ok(mut buf) = payloads_clone.lock() {
                                for (tag_id, distance_m) in last_by_tag {
                                    buf.push(ParsedDistancePayload {
                                        anchor_id,
                                        tag_id,
                                        distance_m,
                                    });
                                }
                            }
                        }
                        Err(err) => {
                            let preview = payload_preview(text, 300);
                            eprintln!("[AMQP] JSON parse error: {err}; payload='{preview}'");
                        }
                    },
                    Err(err) => {
                        eprintln!(
                            "[AMQP] non-UTF8 payload ({} bytes): {err}",
                            delivery.data.len()
                        );
                    }
                }

                if let Err(err) = delivery.ack(BasicAckOptions::default()).await {
                    eprintln!("[AMQP] ack failed: {err}");
                }
            }

            eprintln!("[AMQP] consumer stream ended");
        });
    });

    commands.insert_resource(AmqpDistanceReceiver { payloads });
}

/// Drains shared buffer and emits [`DistanceMeasurement`] events.
fn forward_amqp_to_events(
    receiver: Option<Res<AmqpDistanceReceiver>>,
    provider: Res<ActiveDistanceProvider>,
    mut events: MessageWriter<DistanceMeasurement>,
    mut tracker: Local<HashMap<(usize, usize), f64>>,
    time: Res<Time>,
) {
    if provider.kind != DistanceProviderKind::Amqp {
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
                    distance: Some(payload.distance_m),
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn setup_queue_and_binding(
    channel: &Channel,
    cfg: &AmqpDistanceProviderConfig,
) -> Result<(), lapin::Error> {
    if cfg.declare_queue {
        channel
            .queue_declare(
                &cfg.queue,
                QueueDeclareOptions {
                    durable: cfg.durablequeue,
                    ..QueueDeclareOptions::default()
                },
                FieldTable::default(),
            )
            .await?;
        eprintln!("[AMQP] declared queue '{}'", cfg.queue);
    } else {
        eprintln!("[AMQP] queue declaration disabled for '{}'", cfg.queue);
    }

    if let Some(exchange) = &cfg.bind_exchange {
        channel
            .queue_bind(
                &cfg.queue,
                exchange,
                &cfg.bind_routing_key,
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await?;
        eprintln!(
            "[AMQP] bound queue '{}' -> exchange '{}' with routing_key '{}'",
            cfg.queue, exchange, cfg.bind_routing_key
        );
    }

    Ok(())
}

fn payload_preview(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= max_chars {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}
