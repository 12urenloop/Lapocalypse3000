use crate::deformable_image::DeformableImage;
use crate::ffmpeg::{VideoPlayer, VideoResource, make_video};
use crate::triangulation::{ActiveDistanceProvider, DistanceMeasurement, DistanceProviderKind};
use bevy::ecs::system::SystemParam;
use bevy::render::render_resource::TextureFormat;
use bevy::{platform::collections::HashMap, prelude::*};
use bevy_egui::egui;
use egui::Ui;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Duration;

pub struct LogDistanceProviderPlugin;

impl Plugin for LogDistanceProviderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LogPlaybackState::default())
            .add_systems(Startup, setup_log_provider)
            .add_systems(Update, log_playback_system);
        // .add_systems(EguiPrimaryContextPass, log_playback_ui);
    }
}

// marker component for sprite displaying video frames
#[derive(Component)]
pub struct VideoSprite {
    pub image: Handle<Image>,
}

#[derive(Resource)]
pub struct LogPlaybackState {
    pub recording_name: String,
    pub video_name: String,
    pub is_playing: bool,
    pub current_time_ms: u64,
    pub measurements: Vec<LogMeasurement>,
    pub measurement_index: usize, // last index in measurements that is before current_time_ms
    pub max_time_ms: u64,
    pub last_frame_time: Option<Duration>,
    pub last_contact: HashMap<(usize, usize), u64>, // (anchorid, tagid) -> last event timestamp

    /// Offset in ms: `video_timestamp_ms - log_timestamp_ms` at the sync point.
    /// `None` means no sync has been set yet.
    pub video_sync_offset_ms: Option<i64>,

    /// True while we are waiting for the user to click a second time to capture the video
    /// timestamp for the current sync operation.  The first click stores the log timestamp.
    pub pending_sync_log_ts_ms: Option<u64>,

    /// The video timestamp (ms) that the user enters in step 2 of the sync workflow.
    pub sync_video_ts_ms: u64,

    /// Set by the UI when it performs a discontinuous jump (scrub/restart/load)
    /// that the playback system must account for: cached per-pair state is
    /// stale and has to be re-emitted from scratch.
    pub request_reset: bool,

    /// Set when selecting an environment (or otherwise) to request that the
    /// log panel loads `recording_name`/`video_name` without the user having
    /// to press the Load button. Consumed by `log_sidepanel_ui`.
    pub request_load: bool,
}

impl Default for LogPlaybackState {
    fn default() -> Self {
        Self {
            recording_name: "exp2".to_string(),
            video_name: "./assets/gras1.mp4".to_string(),
            is_playing: false,
            current_time_ms: 0,
            measurements: Vec::new(),
            measurement_index: 0,
            max_time_ms: 0,
            last_frame_time: None,
            last_contact: HashMap::new(),
            video_sync_offset_ms: None,
            pending_sync_log_ts_ms: None,
            sync_video_ts_ms: 0,
            request_reset: false,
            request_load: false,
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

fn setup_log_provider(
    mut provider: ResMut<ActiveDistanceProvider>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
) {
    provider.available.push(DistanceProviderKind::LogFiles);

    let video_handle = images.add(Image::new_target_texture(
        1920,
        1080,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    ));

    commands.spawn((
        Sprite::from_image(video_handle.clone()),
        Transform::default(),
        VideoSprite {
            image: video_handle,
        },
    ));
}

/// Seek the synced video so that `video_ts == log_ts + offset`.
/// No-op when no sync offset is set or no video player exists.
fn seek_video_to_log_time(
    state: &LogPlaybackState,
    video_resource: &mut VideoResource,
    video_player_query: &Query<(Entity, &mut VideoPlayer), With<VideoSprite>>,
) {
    if let Some(offset_ms) = state.video_sync_offset_ms {
        let video_ts_ms = (state.current_time_ms as i64 + offset_ms).max(0);
        if let Ok((video_entity, _)) = video_player_query.single() {
            video_resource.seek(video_entity, video_ts_ms);
        }
    }
}

fn log_playback_system(
    time: Res<Time>,
    mut state: ResMut<LogPlaybackState>,
    provider: Res<ActiveDistanceProvider>,
    mut events: MessageWriter<DistanceMeasurement>,
    mut previous_state: Local<HashMap<(usize, usize), Option<(u64, f32)>>>,
    mut last_resync_ms: Local<u64>,
    mut video_player_query: Query<(Entity, &mut VideoPlayer), With<VideoSprite>>,
    mut video_resource: NonSendMut<VideoResource>,
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
            // Discontinuous jump: cached pair state refers to the end of the
            // recording and must be rebuilt for the start.
            previous_state.clear();
            state.last_contact.clear();
            state.measurement_index = 0;
            // Keep the video in sync across the loop boundary.
            seek_video_to_log_time(&state, &mut video_resource, &video_player_query);
        }
    } else {
        state.last_frame_time = None;
    }

    // A scrub/restart/load performed by the UI is a discontinuous jump: the
    // incremental scan cursor is meaningless afterwards. Rewind the cursor to
    // 0 and clear the per-pair caches so the scan below replays the log from
    // the start up to the new position and re-emits the correct state.
    if state.request_reset {
        state.request_reset = false;
        previous_state.clear();
        state.last_contact.clear();
        state.measurement_index = 0;
    } else if state.measurement_index > 0
        && state.measurement_index < state.measurements.len()
        && !state.measurements.is_empty()
    {
        // Safety net: if the cursor points past the current time (backwards
        // jump without a reset flag), rewind and replay so no measurement
        // is missed. Caches are kept so only real diffs are emitted.
        let cursor_ts = state.measurements[state.measurement_index].timestamp_ms;
        if cursor_ts > state.current_time_ms {
            state.measurement_index = 0;
        }
    }

    // --- Video sync: mirror play/pause and correct drift ---
    if let Ok((video_entity, mut video_player)) = video_player_query.single_mut() {
        if let Some(offset_ms) = state.video_sync_offset_ms {
            let expected_video_ms = state.current_time_ms as i64 + offset_ms;

            // Mirror play/pause state to video.
            let should_pause = !state.is_playing;
            if video_player.paused != should_pause {
                video_player.paused = should_pause;

                // On transition to playing: seek to the correct video position
                // so playback resumes in sync.
                if !should_pause {
                    video_resource.seek(video_entity, expected_video_ms.max(0));
                    *last_resync_ms = state.current_time_ms;
                }
            } else if state.is_playing {
                // While playing, the video advances by decode rate and the log
                // by wall-clock, so they drift apart. Re-seek when the drift
                // exceeds the threshold (checked at most ~2x per second).
                let time_since_resync = state.current_time_ms.saturating_sub(*last_resync_ms);
                if time_since_resync > 500 {
                    *last_resync_ms = state.current_time_ms;
                    if let Some(actual_video_ms) = video_resource.position_ms(video_entity) {
                        let drift = (actual_video_ms - expected_video_ms.max(0)).abs();
                        if drift > 250 {
                            video_resource.seek(video_entity, expected_video_ms.max(0));
                        }
                    }
                }
            }
        }
    }

    // Incremental scan over the timestamp-sorted measurements. Everything at
    // or before `measurement_index` has already been folded into
    // `last_contact`, so only newer entries need visiting.
    let mut scan_index = state.measurement_index;
    let mut current_state: HashMap<(usize, usize), Option<(u64, f32)>> = HashMap::new();
    for m in state.measurements.iter().skip(scan_index) {
        if m.timestamp_ms <= state.current_time_ms {
            let age = state.current_time_ms.saturating_sub(m.timestamp_ms);
            let val = if age <= 700 {
                Some((m.timestamp_ms, m.distance))
            } else {
                None
            };
            // Later (newer) measurements for the same pair overwrite older ones.
            current_state.insert((m.anchor_id, m.tag_id), val);
            scan_index += 1;
        } else {
            break; // sorted, so nothing after this can match either.
        }
    }
    state.measurement_index = scan_index;

    for ((anchorid, tagid), lastms) in &state.last_contact {
        let age = state.current_time_ms.saturating_sub(*lastms);
        if age >= 700 {
            current_state.insert((*anchorid, *tagid), None); // out of range
        }
    }

    for (key, val) in current_state {
        let (anchorid, tagid) = key;
        let cms = state.current_time_ms;
        state.last_contact.insert(key, cms);
        if previous_state.get(&key) != Some(&val) {
            previous_state.insert(key, val);
            events.write(DistanceMeasurement {
                anchor_id: anchorid,
                tag_id: tagid,
                distance: val.map(|(_, d)| d),
                timestamp: val.map(|(ts, _)| ts).unwrap_or(0) as u32,
            });
        }
    }
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

#[derive(SystemParam)]
pub struct LogDistanceUiState<'w, 's> {
    state: ResMut<'w, LogPlaybackState>,
    provider: Res<'w, ActiveDistanceProvider>,
    _images: ResMut<'w, Assets<Image>>,
    video_resource: NonSendMut<'w, VideoResource>,
    videosprite: Query<'w, 's, (Entity, &'static mut Transform, &'static VideoSprite)>,
    deformable: Query<'w, 's, &'static mut DeformableImage>,
    video_player: Query<'w, 's, (Entity, &'static mut VideoPlayer), With<VideoSprite>>,
}

/// (Re)load log measurements and the video for the current
/// `recording_name`/`video_name`. Resets playback to the start but preserves
/// the video sync offset (it belongs to this recording/video pair, e.g. from
/// the environment config).
fn load_recording(
    state: &mut LogPlaybackState,
    commands: &mut Commands,
    sprite_entity: Entity,
    image: Handle<Image>,
    video_resource: &mut VideoResource,
) {
    let measurements = load_logs(&state.recording_name);
    let max_time = measurements.last().map(|m| m.timestamp_ms).unwrap_or(0);

    // Don't crash the whole app when the video file is missing (e.g. a fresh
    // environment whose assets aren't downloaded yet); the log data still loads.
    if Path::new(&state.video_name).exists() {
        let videoplayer = make_video(&state.video_name, image, video_resource, sprite_entity);
        commands.entity(sprite_entity).insert(videoplayer);
    }

    state.measurements = measurements;
    state.max_time_ms = max_time;
    state.current_time_ms = 0;
    state.measurement_index = 0;
    state.is_playing = false;
    state.last_frame_time = None;
    state.pending_sync_log_ts_ms = None;
    state.request_reset = true;
    state.request_load = false;
}

pub fn log_sidepanel_ui(ui: &mut Ui, mut commands: Commands, mut params: LogDistanceUiState) {
    if params.provider.kind != DistanceProviderKind::LogFiles {
        return;
    }
    // Environment selection (or startup) requested a load: perform it without
    // requiring a manual click, so the config's video sync offset takes effect.
    if params.state.request_load {
        if let Ok(single) = params.videosprite.single() {
            let sprite_entity = single.0;
            let image = single.2.image.clone();
            load_recording(
                &mut params.state,
                &mut commands,
                sprite_entity,
                image,
                &mut params.video_resource,
            );
        } else {
            params.state.request_load = false;
        }
    }
    ui.horizontal(|ui| {
        ui.label("Recording Name:");
        ui.text_edit_singleline(&mut params.state.recording_name);
    });
    ui.horizontal(|ui| {
        ui.label("Video Name:");
        ui.text_edit_singleline(&mut params.state.video_name);
    });
    if ui.button("Load").clicked() {
        if let Ok(single) = params.videosprite.single() {
            let sprite_entity = single.0;
            let image_handle = single.2.image.clone();
            load_recording(
                &mut params.state,
                &mut commands,
                sprite_entity,
                image_handle,
                &mut params.video_resource,
            );
        }
    }

    ui.separator();

    if let Ok((_, mut transform, _)) = params.videosprite.single_mut() {
        ui.separator();
        ui.label("Video Sprite Transform");

        let mut position = transform.translation;
        let (mut rot_x, mut rot_y, mut rot_z) = transform.rotation.to_euler(EulerRot::XYZ);
        let mut scale = transform.scale;

        let mut changed = false;

        ui.collapsing("Position", |ui| {
            changed |= ui
                .add(
                    egui::DragValue::new(&mut position.x)
                        .speed(0.01)
                        .prefix("x: "),
                )
                .changed();
            changed |= ui
                .add(
                    egui::DragValue::new(&mut position.y)
                        .speed(0.01)
                        .prefix("y: "),
                )
                .changed();
            changed |= ui
                .add(
                    egui::DragValue::new(&mut position.z)
                        .speed(0.01)
                        .prefix("z: "),
                )
                .changed();
        });

        ui.collapsing("Rotation (degrees)", |ui| {
            let mut deg_x = rot_x.to_degrees();
            let mut deg_y = rot_y.to_degrees();
            let mut deg_z = rot_z.to_degrees();

            let cx = ui
                .add(egui::DragValue::new(&mut deg_x).speed(0.5).prefix("x: "))
                .changed();
            let cy = ui
                .add(egui::DragValue::new(&mut deg_y).speed(0.5).prefix("y: "))
                .changed();
            let cz = ui
                .add(egui::DragValue::new(&mut deg_z).speed(0.5).prefix("z: "))
                .changed();

            if cx || cy || cz {
                rot_x = deg_x.to_radians();
                rot_y = deg_y.to_radians();
                rot_z = deg_z.to_radians();
                changed = true;
            }
        });

        ui.collapsing("Scale", |ui| {
            changed |= ui
                .add(egui::DragValue::new(&mut scale.x).speed(0.01).prefix("x: "))
                .changed();
        });

        if changed {
            transform.translation = position;
            transform.rotation = Quat::from_euler(EulerRot::XYZ, rot_x, rot_y, rot_z);
            scale.y = scale.x;
            transform.scale = scale;
        }

        if let Ok(mut deformable) = params.deformable.single_mut() {
            ui.separator();
            ui.label("4-Corner Image Deformation");
            ui.checkbox(&mut deformable.enabled, "Enable Drag Handles Gizmo");

            ui.collapsing("Corner Coordinates (Local)", |ui| {
                let labels = ["Top-Left", "Top-Right", "Bottom-Right", "Bottom-Left"];
                for i in 0..4 {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}:", labels[i]));
                        let cx = ui
                            .add(
                                egui::DragValue::new(&mut deformable.corners[i].x)
                                    .speed(0.1)
                                    .prefix("x: "),
                            )
                            .changed();
                        let cy = ui
                            .add(
                                egui::DragValue::new(&mut deformable.corners[i].y)
                                    .speed(0.1)
                                    .prefix("y: "),
                            )
                            .changed();
                        if cx || cy {
                            deformable.is_dirty = true;
                        }
                    });
                }
            });

            if ui.button("Reset Corner Quad").clicked() {
                deformable.reset_rect();
            }
        }
    }
    if params.state.measurements.is_empty() {
        ui.label("No data loaded.");
        return;
    }

    ui.label(format!(
        "Loaded {} measurements.",
        params.state.measurements.len()
    ));

    ui.horizontal(|ui| {
        if ui
            .button(if params.state.is_playing {
                "Pause"
            } else {
                "Play"
            })
            .clicked()
        {
            params.state.is_playing = !params.state.is_playing;
            if params.state.is_playing {
                // Reset the frame time so that we don't jump on resume
                params.state.last_frame_time = None;
            }
        }

        if ui.button("Restart").clicked() {
            params.state.current_time_ms = 0;
            params.state.request_reset = true;
            // Restart the synced video from its mapped start position.
            seek_video_to_log_time(
                &params.state,
                &mut params.video_resource,
                &params.video_player,
            );
        }
    });

    let mut time_f64 = params.state.current_time_ms as f64;
    ui.spacing_mut().slider_width = 300.0;
    let slider = egui::Slider::new(&mut time_f64, 0.0..=params.state.max_time_ms as f64).text("ms");
    if ui.add(slider).changed() {
        params.state.current_time_ms = time_f64 as u64;
        params.state.request_reset = true;

        // When the user scrubs the slider while synced, also seek the video.
        seek_video_to_log_time(
            &params.state,
            &mut params.video_resource,
            &params.video_player,
        );
    }

    // --- Video sync section ---
    ui.separator();
    ui.label(egui::RichText::new("Video Sync").strong());

    // Show current sync status
    match params.state.video_sync_offset_ms {
        Some(offset) => {
            ui.colored_label(
                egui::Color32::from_rgb(100, 220, 100),
                format!("✔ Synced (video offset: {offset:+} ms)"),
            );
        }
        None => {
            ui.colored_label(egui::Color32::GRAY, "Not synced");
        }
    }

    // Live video clock readout (last decoded pts / container duration).
    if let Ok((video_entity, _)) = params.video_player.single() {
        let pos = params.video_resource.position_ms(video_entity);
        let dur = params.video_resource.duration_ms(video_entity);
        match (pos, dur) {
            (Some(p), Some(d)) => {
                ui.label(format!("Video position: {p} ms / {d} ms"));
            }
            (Some(p), None) => {
                ui.label(format!("Video position: {p} ms"));
            }
            _ => {
                ui.label("Video position: — (no frames decoded yet)");
            }
        }
    } else {
        ui.label("Video position: — (video not loaded)");
    }

    ui.add_space(2.0);

    // Two-step sync: step 1 freezes the log and captures its timestamp;
    // step 2 seeks the (paused) video to the matching frame and reads back
    // the real decoder position to compute `offset = video_ts - log_ts`.
    match params.state.pending_sync_log_ts_ms {
        None => {
            // Step 1: pause video + log, capture log timestamp
            let btn = ui
                .button("🎬 Sync: capture log position")
                .on_hover_text(
                    "Pause here, then click to record the current log timestamp.\n\
                     Next, seek the video to the matching frame and click the second button.",
                );
            if btn.clicked() {
                // Pause playback while the user positions the video manually
                params.state.is_playing = false;
                params.state.last_frame_time = None;
                // Pause video
                if let Ok((_, mut vp)) = params.video_player.single_mut() {
                    vp.paused = true;
                }
                params.state.pending_sync_log_ts_ms = Some(params.state.current_time_ms);
                // Pre-fill the manual field with the current decoder position
                // so Confirm works even before any new frame is decoded.
                if let Ok((video_entity, _)) = params.video_player.single() {
                    if let Some(pos) = params.video_resource.position_ms(video_entity) {
                        params.state.sync_video_ts_ms = pos.max(0) as u64;
                    }
                }
            }
        }
        Some(captured_log_ts) => {
            ui.colored_label(
                egui::Color32::YELLOW,
                format!(
                    "Step 2 – log position captured at {captured_log_ts} ms.\n\
                     Seek the video to the matching frame, then click below."
                ),
            );

            // Let the user scrub the paused video directly to the matching frame.
            if let Ok((video_entity, mut vp)) = params.video_player.single_mut() {
                vp.paused = true; // keep frozen while picking the sync frame
                let current_pos = params
                    .video_resource
                    .position_ms(video_entity)
                    .unwrap_or(params.state.sync_video_ts_ms as i64)
                    .max(0) as u64;
                let max_pos = params
                    .video_resource
                    .duration_ms(video_entity)
                    .unwrap_or(current_pos as i64 + 60_000)
                    .max(1) as f64;
                let mut video_f64 = current_pos as f64;
                ui.spacing_mut().slider_width = 300.0;
                if ui
                    .add(egui::Slider::new(&mut video_f64, 0.0..=max_pos).text("video ms"))
                    .changed()
                {
                    params.video_resource.seek(video_entity, video_f64 as i64);
                    params.state.sync_video_ts_ms = video_f64 as u64;
                }
            }

            if ui
                .button("📍 Confirm: use this video frame as sync point")
                .on_hover_text(
                    "Reads the current video decode position and computes the sync offset.",
                )
                .clicked()
            {
                // Prefer the live decoder clock; fall back to the manual field
                // if no frame has been decoded yet (e.g. right after Load).
                let video_ts_ms = params
                    .video_player
                    .single()
                    .ok()
                    .and_then(|(e, _)| params.video_resource.position_ms(e))
                    .unwrap_or(params.state.sync_video_ts_ms as i64)
                    .max(0);
                params.state.sync_video_ts_ms = video_ts_ms as u64;
                let offset = video_ts_ms - captured_log_ts as i64;
                params.state.video_sync_offset_ms = Some(offset);
                params.state.pending_sync_log_ts_ms = None;

                // Snap the video to the log's current position under the new offset.
                seek_video_to_log_time(
                    &params.state,
                    &mut params.video_resource,
                    &params.video_player,
                );
                // Keep video paused until the user presses Play
                if let Ok((_, mut vp)) = params.video_player.single_mut() {
                    vp.paused = true;
                }
            }

            if ui.button("✖ Cancel sync").clicked() {
                params.state.pending_sync_log_ts_ms = None;
                // Unpause video if it was playing before
                if let Ok((_, mut vp)) = params.video_player.single_mut() {
                    vp.paused = !params.state.is_playing;
                }
            }
        }
    }

    // When a sync is in progress (step 2), show a manual override for the
    // video timestamp (fallback when the decoder position is unavailable).
    if params.state.pending_sync_log_ts_ms.is_some() {
        ui.horizontal(|ui| {
            ui.label("Video timestamp override (ms):");
            let mut v = params.state.sync_video_ts_ms as f64;
            if ui
                .add(egui::DragValue::new(&mut v).speed(10.0).suffix(" ms"))
                .changed()
            {
                params.state.sync_video_ts_ms = v.max(0.0) as u64;
            }
        });
    }

    if params.state.video_sync_offset_ms.is_some() {
        if ui
            .button("✖ Clear sync")
            .on_hover_text("Remove the video sync so they play independently.")
            .clicked()
        {
            params.state.video_sync_offset_ms = None;
            // Let video run freely
            if let Ok((_, mut vp)) = params.video_player.single_mut() {
                vp.paused = false;
            }
        }
    }
}
