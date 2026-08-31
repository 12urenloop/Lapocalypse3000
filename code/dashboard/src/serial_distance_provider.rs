use bevy::{platform::collections::HashMap, prelude::*};
use regex::Regex;
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::triangulation::{ActiveDistanceProvider, DistanceMeasurement, DistanceProviderKind};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Resource, Clone)]
pub struct SerialDistanceProviderConfig {
    /// Serial port device path (e.g., "/dev/ttyUSB0").
    pub port: String,
    /// Baud rate for the serial connection.
    pub baud_rate: u32,
}

impl Default for SerialDistanceProviderConfig {
    fn default() -> Self {
        Self {
            port: "/dev/ttyUSB0".to_string(),
            baud_rate: 115200,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct SerialDistancePayload {
    anchor_id: usize,
    tag_id: usize,
    distance: f32,
    timestamp: u32,
}

#[derive(Resource)]
struct SerialDistanceReceiver {
    payloads: Arc<Mutex<Vec<SerialDistancePayload>>>,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct SerialDistanceProviderPlugin;

impl Plugin for SerialDistanceProviderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SerialDistanceProviderConfig>();

        app.add_systems(Startup, setup_serial_connection)
            .add_systems(Update, forward_serial_to_events);
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn setup_serial_connection(
    mut commands: Commands,
    config: Res<SerialDistanceProviderConfig>,
    mut provider: ResMut<ActiveDistanceProvider>,
) {
    println!(
        "starting Serial listener on port {} @ {} baud",
        config.port, config.baud_rate
    );

    if !provider.available.contains(&DistanceProviderKind::Serial) {
        provider.available.push(DistanceProviderKind::Serial);
    }

    let payloads: Arc<Mutex<Vec<SerialDistancePayload>>> = Arc::new(Mutex::new(Vec::new()));
    let payloads_clone = payloads.clone();
    let port_path = config.port.clone();
    let baud_rate = config.baud_rate;

    thread::spawn(move || {
        loop {
            match serialport::new(&port_path, baud_rate)
                .timeout(Duration::from_millis(100))
                .open()
            {
                Ok(port) => {
                    println!("Serial port {} opened successfully", port_path);
                    let mut reader = BufReader::new(port);
                    let mut line = String::new();

                    loop {
                        line.clear();
                        match reader.read_line(&mut line) {
                            Ok(0) => {
                                // EOF or disconnected
                                eprintln!("Serial port {} disconnected (EOF)", port_path);
                                break;
                            }
                            Ok(_) => {
                                let text = line.trim();
                                if !text.is_empty() {
                                    if let Some(mut parsed) = parse_serial_message(text) {
                                        if let Ok(mut p) = payloads_clone.lock() {
                                            p.append(&mut parsed);
                                        }
                                    }
                                }
                            }
                            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                                // Timeout reading line, retry seamlessly
                                continue;
                            }
                            Err(e) => {
                                eprintln!("Serial read error on {}: {}", port_path, e);
                                thread::sleep(Duration::from_millis(500));
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to open serial port {}: {}", port_path, e);
                    thread::sleep(Duration::from_secs(2));
                }
            }
        }
    });

    commands.insert_resource(SerialDistanceReceiver { payloads });
    println!("Serial provider setup ok");
}

/// Parse a message of the form `1 | 1 = 1.23@100-200 | 2 = 4.56@100-200`
/// Returns one payload per tag-distance pair.
fn parse_serial_message(text: &str) -> Option<Vec<SerialDistancePayload>> {
    let parts: Vec<&str> = text.split('|').map(|s| s.trim()).collect();
    if parts.is_empty() {
        return None;
    }

    let anchor_id: usize = parts[0].parse().ok()?;
    let mut results = Vec::new();

    let re = Regex::new(r"(\d+)=(-?\d+\.\d+)@(\d+)-(\d+)").expect("regex nocompile");

    for part in parts[1..].iter() {
        if let Some(caps) = re.captures(part) {
            let tag_id: usize = match caps[1].parse() {
                Ok(val) => val,
                Err(_) => continue,
            };
            let distance: f32 = match caps[2].parse() {
                Ok(val) => val,
                Err(_) => continue,
            };
            let timestamp: u32 = match caps[4].parse() {
                Ok(val) => val,
                Err(_) => continue,
            };

            results.push(SerialDistancePayload {
                anchor_id,
                tag_id,
                distance,
                timestamp,
            });
        }
    }

    if results.is_empty() {
        None
    } else {
        Some(results)
    }
}

/// Drains the shared buffer and emits [`DistanceMeasurement`] events.
fn forward_serial_to_events(
    receiver: Option<Res<SerialDistanceReceiver>>,
    provider: Res<ActiveDistanceProvider>,
    mut events: MessageWriter<DistanceMeasurement>,
    mut tracker: Local<HashMap<(usize, usize), f64>>,
    time: Res<Time>,
) {
    if provider.kind != DistanceProviderKind::Serial {
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
                    distance: Some(payload.distance),
                    timestamp: payload.timestamp,
                });
            }
        }
    }

    // Staleness: if nothing seen for 2s, mark as out of range
    tracker.retain(|&(anchor_id, tag_id), last_seen| {
        if now - *last_seen > 2.0 {
            events.write(DistanceMeasurement {
                anchor_id,
                tag_id,
                distance: None,
                timestamp: 0,
            });
            false
        } else {
            true
        }
    });
}
