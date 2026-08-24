use bevy::{platform::collections::HashMap, prelude::*};
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::triangulation::{ActiveDistanceProvider, DistanceMeasurement, DistanceProviderKind};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Resource, Clone)]
pub struct UdpDistanceProviderConfig {
    /// Base UDP port. Anchor 1 listens on `base_port`, anchor 2 on `base_port + 1`, etc.
    pub base_port: u16,
    /// Number of anchor ports to listen on.
    pub anchor_count: usize,
}

impl Default for UdpDistanceProviderConfig {
    fn default() -> Self {
        Self {
            base_port: 5001,
            anchor_count: 5,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct UdpDistancePayload {
    anchor_id: usize,
    tag_id: usize,
    distance: f32,
}

#[derive(Resource)]
struct UdpDistanceReceiver {
    payloads: Arc<Mutex<Vec<UdpDistancePayload>>>,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct UdpDistanceProviderPlugin;

impl Plugin for UdpDistanceProviderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UdpDistanceProviderConfig>();

        app.add_systems(Startup, setup_udp_connections)
            .add_systems(Update, forward_udp_to_events);
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn setup_udp_connections(
    mut commands: Commands,
    config: Res<UdpDistanceProviderConfig>,
    mut provider: ResMut<ActiveDistanceProvider>,
) {
    println!(
        "starting UDP listener on {} ports ({}-{}",
        config.anchor_count,
        config.base_port,
        config.base_port + config.anchor_count as u16 - 1,
    );

    if !provider.available.contains(&DistanceProviderKind::Udp) {
        provider.available.push(DistanceProviderKind::Udp);
    }

    let payloads: Arc<Mutex<Vec<UdpDistancePayload>>> = Arc::new(Mutex::new(Vec::new()));

    for i in 0..config.anchor_count {
        let port = config.base_port + i as u16;
        let payloads_clone = payloads.clone();

        thread::spawn(move || {
            let addr = format!("0.0.0.0:{port}");
            let socket = match UdpSocket::bind(&addr) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("UDP bind failed for {addr}: {e}");
                    return;
                }
            };
            // Set a large enough buffer for multiple messages arriving quickly
            socket
                .set_nonblocking(true)
                .expect("cannot set nonblocking");

            println!("UDP listening on {addr}");

            let mut buf = [0u8; 2048];
            loop {
                match socket.recv_from(&mut buf) {
                    Ok((len, _from)) => {
                        if let Ok(text) = std::str::from_utf8(&buf[..len]) {
                            let text = text.trim();
                            if let Some(mut parsed) = parse_udp_message(text) {
                                // The anchor_id is redundant with the port we received on,
                                // but we trust the message content.
                                if let Ok(mut p) = payloads_clone.lock() {
                                    p.append(&mut parsed);
                                }
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // No data right now, sleep briefly to avoid busy-waiting
                        thread::sleep(std::time::Duration::from_millis(1));
                    }
                    Err(e) => {
                        eprintln!("UDP recv error on port {port}: {e}");
                        thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            }
        });
    }

    commands.insert_resource(UdpDistanceReceiver { payloads });
    println!("UDP provider ok");
}

/// Parse a message of the form `1 | 1 = 1.23 | 2 = 4.56`
/// Returns one payload per tag-distance pair.
fn parse_udp_message(text: &str) -> Option<Vec<UdpDistancePayload>> {
    let parts: Vec<&str> = text.split('|').map(|s| s.trim()).collect();
    if parts.is_empty() {
        return None;
    }

    let anchor_id: usize = parts[0].parse().ok()?;
    let mut results = Vec::new();

    for part in &parts[1..] {
        // Each part looks like "1 = 1.23" or " 2 = 4.56"
        let kv: Vec<&str> = part.splitn(2, '=').map(|s| s.trim()).collect();
        if kv.len() == 2 {
            if let Ok(tag_id) = kv[0].parse::<usize>() {
                if let Ok(distance) = kv[1].parse::<f32>() {
                    results.push(UdpDistancePayload {
                        anchor_id,
                        tag_id,
                        distance,
                    });
                }
            }
        }
    }

    if results.is_empty() {
        None
    } else {
        Some(results)
    }
}

/// Drains the shared buffer and emits [`DistanceMeasurement`] events.
fn forward_udp_to_events(
    receiver: Option<Res<UdpDistanceReceiver>>,
    provider: Res<ActiveDistanceProvider>,
    mut events: MessageWriter<DistanceMeasurement>,
    mut tracker: Local<HashMap<(usize, usize), f64>>,
    time: Res<Time>,
) {
    if provider.kind != DistanceProviderKind::Udp {
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
            });
            false
        } else {
            true
        }
    });
}
