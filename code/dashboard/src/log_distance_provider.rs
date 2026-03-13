use bevy::{platform::collections::HashMap, prelude::*};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use crate::triangulation::{ActiveDistanceProvider, DistanceMeasurement, DistanceProviderKind};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Duration;

pub struct LogDistanceProviderPlugin;

impl Plugin for LogDistanceProviderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LogPlaybackState::default())
            .add_systems(Startup, setup_log_provider)
            .add_systems(Update, log_playback_system)
            .add_systems(EguiPrimaryContextPass, log_playback_ui);
    }
}

#[derive(Resource)]
pub struct LogPlaybackState {
    pub recording_name: String,
    pub is_playing: bool,
    pub current_time_ms: u64,
    pub measurements: Vec<LogMeasurement>,
    pub max_time_ms: u64,
    pub last_frame_time: Option<Duration>,
}

impl Default for LogPlaybackState {
    fn default() -> Self {
        Self {
            recording_name: "exp2".to_string(),
            is_playing: false,
            current_time_ms: 0,
            measurements: Vec::new(),
            max_time_ms: 0,
            last_frame_time: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogMeasurement {
    pub anchor_id: usize,
    pub tag_id: usize,
    pub distance: f32,
    pub timestamp_ms: u64,
}

fn setup_log_provider(mut provider: ResMut<ActiveDistanceProvider>) {
    provider.available.push(DistanceProviderKind::LogFiles);
}

fn log_playback_system(
    time: Res<Time>,
    mut state: ResMut<LogPlaybackState>,
    provider: Res<ActiveDistanceProvider>,
    mut events: EventWriter<DistanceMeasurement>,
    mut previous_state: Local<HashMap<(usize, usize), Option<(u64, f32)>>>,
) {
    if provider.kind != DistanceProviderKind::LogFiles {
        state.last_frame_time = None;
        previous_state.clear();
        return;
    }

    if state.is_playing {
        let elapsed = time.elapsed();
        let delta = if let Some(last) = state.last_frame_time {
            elapsed.saturating_sub(last)
        } else {
            Duration::ZERO
        };
        state.last_frame_time = Some(elapsed);

        let delta_ms = delta.as_millis() as u64;
        state.current_time_ms += delta_ms;

        if state.current_time_ms > state.max_time_ms && state.max_time_ms > 0 {
            state.current_time_ms %= state.max_time_ms; // Loop playback
        }
    } else {
        state.last_frame_time = None;
    }

    let mut current_state = HashMap::new();
    for m in &state.measurements {
        if m.timestamp_ms <= state.current_time_ms {
            let age = state.current_time_ms.saturating_sub(m.timestamp_ms);
            let val = if age <= 2000 {
                Some((m.timestamp_ms, m.distance))
            } else {
                None
            };
            current_state.insert((m.anchor_id, m.tag_id), val);
        }
    }

    for (key, val) in current_state {
        if previous_state.get(&key) != Some(&val) {
            previous_state.insert(key, val);
            events.write(DistanceMeasurement {
                anchor_id: key.0,
                tag_id: key.1,
                distance: val.map(|(_, d)| d),
            });
        }
    }
}

fn log_playback_ui(
    mut contexts: EguiContexts,
    mut state: ResMut<LogPlaybackState>,
    provider: Res<ActiveDistanceProvider>,
) {
    if provider.kind != DistanceProviderKind::LogFiles {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::Window::new("Log Playback").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Recording Name:");
            ui.text_edit_singleline(&mut state.recording_name);
            if ui.button("Load").clicked() {
                let measurements = load_logs(&state.recording_name);
                let max_time = measurements.last().map(|m| m.timestamp_ms).unwrap_or(0);
                state.measurements = measurements;
                state.max_time_ms = max_time;
                state.current_time_ms = 0;
                state.is_playing = false;
                state.last_frame_time = None;
            }
        });

        ui.separator();

        if state.measurements.is_empty() {
            ui.label("No data loaded.");
            return;
        }

        ui.label(format!("Loaded {} measurements.", state.measurements.len()));

        ui.horizontal(|ui| {
            if ui.button(if state.is_playing { "Pause" } else { "Play" }).clicked() {
                state.is_playing = !state.is_playing;
                if state.is_playing {
                    // Reset the frame time so that we don't jump on resume
                    state.last_frame_time = None;
                }
            }

            if ui.button("Restart").clicked() {
                state.current_time_ms = 0;
            }
        });

        let mut time_f64 = state.current_time_ms as f64;
        let slider = egui::Slider::new(&mut time_f64, 0.0..=state.max_time_ms as f64)
            .text("ms");
        if ui.add(slider).changed() {
            state.current_time_ms = time_f64 as u64;
        }
    });
}

fn load_logs(recording_name: &str) -> Vec<LogMeasurement> {
    let mut measurements = Vec::new();
    let dir = Path::new("data").join(recording_name);
    if !dir.exists() {
        return measurements;
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name.starts_with("anchor") && file_name.ends_with(".txt") {
                    let id_str = &file_name[6..file_name.len() - 4];
                    if let Ok(anchor_id) = id_str.parse::<usize>() {
                        if let Ok(file) = File::open(&path) {
                            let reader = BufReader::new(file);
                            for line in reader.lines().flatten() {
                                if line.starts_with("= ") {
                                    // format: "= <tag id> <distance in centimeter> mesh <synced time in millis> <unsynced millis>"
                                    // example: = 1 1.55 mesh 4990319 716230
                                    let parts: Vec<&str> = line.split_whitespace().collect();
                                    if parts.len() >= 6 && parts[3] == "mesh" {
                                        if let (Ok(tag_id), Ok(distance), Ok(timestamp_ms)) = (
                                            parts[1].parse::<usize>(),
                                            parts[2].parse::<f32>(),
                                            parts[4].parse::<u64>(),
                                        ) {
                                            measurements.push(LogMeasurement {
                                                anchor_id,
                                                tag_id,
                                                distance,
                                                timestamp_ms,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    measurements.sort_by_key(|m| m.timestamp_ms);
    if let Some(first) = measurements.first().cloned() {
        let first_time = first.timestamp_ms;
        for m in &mut measurements {
            m.timestamp_ms = m.timestamp_ms.saturating_sub(first_time);
        }
    }

    measurements
}
