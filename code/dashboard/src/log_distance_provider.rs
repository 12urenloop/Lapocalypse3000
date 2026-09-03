use crate::deformable_image::{self, DeformableImage};
use crate::ffmpeg::{VideoResource, make_video};
use crate::triangulation::{ActiveDistanceProvider, DistanceMeasurement, DistanceProviderKind};
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    provider.available.push(DistanceProviderKind::LogFiles);

    let image = Image::new_target_texture(
        1920,
        1080,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );
    let first_pass_layer = RenderLayers::layer(1);
    let image_handle = images.add(image);
    let video_handle = images.add(Image::new_target_texture(
        1920,
        1080,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    ));

    commands.spawn((
        Camera2d::default(),
        Camera {
            // render before the "main pass" camera
            order: -1,
            clear_color: Color::NONE.into(),
            ..default()
        },
        RenderTarget::Image(image_handle.clone().into()),
        first_pass_layer.clone(),
    ));

    let border = meshes.add(Rectangle::new(1900.0, 1060.0).to_ring(20.0));

    commands.spawn((
        Mesh2d(border),
        MeshMaterial2d(materials.add(Color::WHITE)),
        first_pass_layer,
    ));

    let (deformable, mesh_handle) =
        DeformableImage::new_rect(Vec2::new(192.0, 108.0), 16, &mut meshes);

    let material_handle = materials.add(ColorMaterial {
        texture: Some(image_handle.clone()),
        ..default()
    });

    commands.spawn((
        Sprite::from_image(video_handle.clone()),
        Transform::default(),
        VideoSprite {
            image: video_handle,
        },
    ));

    commands.spawn((
        Mesh2d(mesh_handle),
        MeshMaterial2d(material_handle),
        Transform::default(),
        deformable,
    ));
}

fn log_playback_system(
    time: Res<Time>,
    mut state: ResMut<LogPlaybackState>,
    provider: Res<ActiveDistanceProvider>,
    mut events: MessageWriter<DistanceMeasurement>,
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
    for (i, m) in (&state.measurements)
        .iter()
        .skip(state.measurement_index)
        .enumerate()
    {
        if m.timestamp_ms <= state.current_time_ms {
            let age = state.current_time_ms.saturating_sub(m.timestamp_ms);
            let val = if age <= 700 {
                Some((m.timestamp_ms, m.distance))
            } else {
                None
            };
            current_state.insert((m.anchor_id, m.tag_id), val);
        } else {
            state.measurement_index = i - 1;
            break; // sorted so no problem, right?
        }
    }

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
}

pub fn log_sidepanel_ui(ui: &mut Ui, mut commands: Commands, mut params: LogDistanceUiState) {
    if params.provider.kind != DistanceProviderKind::LogFiles {
        return;
    }
    ui.label("Recording Name:");
    ui.text_edit_singleline(&mut params.state.recording_name);
    ui.label("Video Name:");
    ui.text_edit_singleline(&mut params.state.video_name);
    ui.horizontal(|ui| {
        ui.label("Recording Name:");
        ui.text_edit_singleline(&mut params.state.recording_name);
    });
    ui.horizontal(|ui| {
        ui.label("Video Name:");
        ui.text_edit_singleline(&mut params.state.video_name);
    });
    if ui.button("Load").clicked() {
        let measurements = load_logs(&params.state.recording_name);
        let max_time = measurements.last().map(|m| m.timestamp_ms).unwrap_or(0);

        let videoplayer = make_video(
            &params.state.video_name,
            params.videosprite.single().unwrap().2.image.clone(),
            params.video_resource,
            params.videosprite.single().unwrap().0,
        );

        commands
            .entity(params.videosprite.single().unwrap().0)
            .insert(videoplayer);

        params.state.measurements = measurements;
        params.state.max_time_ms = max_time;
        params.state.current_time_ms = 0;
        params.state.measurement_index = 0;
        params.state.is_playing = false;
        params.state.last_frame_time = None;
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
            params.state.measurement_index = 0;
        }
    });

    let mut time_f64 = params.state.current_time_ms as f64;
    ui.spacing_mut().slider_width = 300.0;
    let slider = egui::Slider::new(&mut time_f64, 0.0..=params.state.max_time_ms as f64).text("ms");
    if ui.add(slider).changed() {
        params.state.current_time_ms = time_f64 as u64;
        params.state.measurement_index = 0;
    }
}
