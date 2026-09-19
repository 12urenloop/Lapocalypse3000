use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::egui;
use egui::Ui;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs};

use crate::deformable_image::DeformableImage;
use crate::log_distance_provider::{LogPlaybackState, VideoSprite};
use crate::triangulation::TriangulationState;
use crate::triangulation::{ActiveDistanceProvider, DistanceProviderKind};

#[derive(Debug, Serialize, Deserialize, Resource, Clone, Default)]
pub struct ConfigFile {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "default",
        alias = "default_config"
    )]
    pub default_env: Option<String>,
    pub envs: BTreeMap<String, NamedEnv>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct NamedEnv {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub anchors: BTreeMap<String, AnchorConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none", alias = "recording")]
    pub recording_name: Option<String>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "video_name",
        alias = "video"
    )]
    pub video_path: Option<String>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_vec3_opt",
        alias = "position"
    )]
    pub video_position: Option<Vec3Config>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_vec3_opt",
        alias = "scale"
    )]
    pub video_scale: Option<Vec3Config>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_vec3_opt",
        alias = "rotation"
    )]
    pub video_rotation: Option<Vec3Config>,

    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_corners",
        alias = "corners"
    )]
    pub deformable_corners: Option<[Vec2Config; 4]>,
    pub ui_scale: Option<f32>,

    /// Offset in milliseconds: `video_timestamp_ms - log_timestamp_ms` at the sync point.
    /// When set, log playback will seek the video to `current_log_time_ms + video_sync_offset_ms`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_sync_offset_ms: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<DistanceProviderKind>,
}

impl NamedEnv {
    pub fn is_empty(&self) -> bool {
        self.anchors.is_empty()
            && self.recording_name.is_none()
            && self.video_path.is_none()
            && self.video_position.is_none()
            && self.video_scale.is_none()
            && self.video_rotation.is_none()
            && self.deformable_corners.is_none()
            && self.ui_scale.is_none()
            && self.video_sync_offset_ms.is_none()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub struct AnchorConfig {
    pub x: f32,
    pub y: f32,
}

impl From<Vec2> for AnchorConfig {
    fn from(v: Vec2) -> Self {
        Self { x: v.x, y: v.y }
    }
}

impl From<AnchorConfig> for Vec2 {
    fn from(a: AnchorConfig) -> Self {
        Vec2::new(a.x, a.y)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub struct Vec2Config {
    pub x: f32,
    pub y: f32,
}

impl From<Vec2> for Vec2Config {
    fn from(v: Vec2) -> Self {
        Self { x: v.x, y: v.y }
    }
}

impl From<Vec2Config> for Vec2 {
    fn from(v: Vec2Config) -> Self {
        Vec2::new(v.x, v.y)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub struct Vec3Config {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<Vec3> for Vec3Config {
    fn from(v: Vec3) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

impl From<Vec3Config> for Vec3 {
    fn from(v: Vec3Config) -> Self {
        Vec3::new(v.x, v.y, v.z)
    }
}

fn deserialize_vec3_opt<'de, D>(deserializer: D) -> Result<Option<Vec3Config>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Helper {
        Struct {
            x: f32,
            y: f32,
            #[serde(default)]
            z: f32,
        },
        Array3([f32; 3]),
        Array2([f32; 2]),
        Scalar(f32),
    }

    let opt = Option::<Helper>::deserialize(deserializer)?;
    match opt {
        Some(Helper::Struct { x, y, z }) => Ok(Some(Vec3Config { x, y, z })),
        Some(Helper::Array3([x, y, z])) => Ok(Some(Vec3Config { x, y, z })),
        Some(Helper::Array2([x, y])) => Ok(Some(Vec3Config { x, y, z: 0.0 })),
        Some(Helper::Scalar(s)) => Ok(Some(Vec3Config { x: s, y: s, z: s })),
        None => Ok(None),
    }
}

fn deserialize_corners<'de, D>(deserializer: D) -> Result<Option<[Vec2Config; 4]>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Vec2Helper {
        Struct { x: f32, y: f32 },
        Array([f32; 2]),
    }

    let opt = Option::<Vec<Vec2Helper>>::deserialize(deserializer)?;
    match opt {
        Some(vec) if vec.len() == 4 => {
            let convert = |h: &Vec2Helper| match h {
                Vec2Helper::Struct { x, y } => Vec2Config { x: *x, y: *y },
                Vec2Helper::Array([x, y]) => Vec2Config { x: *x, y: *y },
            };
            Ok(Some([
                convert(&vec[0]),
                convert(&vec[1]),
                convert(&vec[2]),
                convert(&vec[3]),
            ]))
        }
        Some(vec) => Err(serde::de::Error::custom(format!(
            "expected 4 corners for deformable_corners, found {}",
            vec.len()
        ))),
        None => Ok(None),
    }
}

pub struct ConfigPlugin;

impl Plugin for ConfigPlugin {
    fn build(&self, app: &mut App) {
        let yaml = fs::read_to_string("config.yaml").unwrap_or_else(|_| "envs: {}".to_string());

        let mut config: ConfigFile =
            serde_yaml::from_str(&yaml).expect("Failed to parse config.yaml");
        config
            .envs
            .insert("default".to_string(), NamedEnv::default());

        let default_env_name = config
            .default_env
            .clone()
            .filter(|name| config.envs.contains_key(name))
            .unwrap_or_else(|| {
                if config.envs.contains_key("gras1") {
                    "gras1".to_string()
                } else if config.envs.contains_key("default") {
                    "default".to_string()
                } else if let Some(first) = config.envs.keys().next() {
                    first.clone()
                } else {
                    "default".to_string()
                }
            });

        app.insert_resource(config).insert_resource(ConfigState {
            envname: default_env_name,
            initial_loaded: false,
            export_target_mode: ExportTargetMode::Current,
            existing_target_name: String::new(),
            new_env_name: String::new(),
            set_as_default_on_export: false,
            status_message: None,
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportTargetMode {
    #[default]
    Current,
    Existing,
    New,
}

#[derive(Resource)]
pub struct ConfigState {
    pub envname: String,
    pub initial_loaded: bool,
    pub export_target_mode: ExportTargetMode,
    pub existing_target_name: String,
    pub new_env_name: String,
    pub set_as_default_on_export: bool,
    pub status_message: Option<(String, bool)>,
}

#[derive(SystemParam)]
pub struct ConfigUiState<'w, 's> {
    pub config: ResMut<'w, ConfigState>,
    pub configfile: ResMut<'w, ConfigFile>,
    pub triangulation: ResMut<'w, TriangulationState>,
    pub log_state: ResMut<'w, LogPlaybackState>,
    pub videosprite: Query<'w, 's, (Entity, &'static mut Transform, &'static VideoSprite)>,
    pub deformable: Query<'w, 's, &'static mut DeformableImage>,
    pub provider: ResMut<'w, ActiveDistanceProvider>,
}

pub fn load_environment(params: &mut ConfigUiState) {
    let Some(env) = params.configfile.envs.get(&params.config.envname).cloned() else {
        return;
    };

    // 1. Anchors
    params.triangulation.anchors.clear();
    for (name, anchor) in env.anchors {
        if let Ok(id) = name.parse::<usize>() {
            params.triangulation.anchors.insert(
                id,
                Vec2 {
                    x: anchor.x,
                    y: anchor.y,
                },
            );
        }
    }

    // 2. Log provider recording name and video path.
    // Anything that changes the recording/video pair (or its sync offset)
    // requests a (re)load so the log panel picks it up without a manual click.
    let mut log_changed = false;
    if let Some(recording_name) = env.recording_name {
        if params.log_state.recording_name != recording_name {
            log_changed = true;
        }
        params.log_state.recording_name = recording_name;
    }
    if let Some(video_path) = env.video_path {
        if params.log_state.video_name != video_path {
            log_changed = true;
        }
        params.log_state.video_name = video_path;
    }

    // 3. Video transform (position, scale, rotation in degrees)
    if let Some((_, mut transform, _)) = params.videosprite.iter_mut().next() {
        if let Some(pos) = env.video_position {
            transform.translation = Vec3::new(pos.x, pos.y, pos.z);
        }
        if let Some(rot) = env.video_rotation {
            transform.rotation = Quat::from_euler(
                EulerRot::XYZ,
                rot.x.to_radians(),
                rot.y.to_radians(),
                rot.z.to_radians(),
            );
        }
        if let Some(scale) = env.video_scale {
            transform.scale = Vec3::new(scale.x, scale.y, scale.z);
        }
    }

    // 4. Deformable corners
    if let Some(corners) = env.deformable_corners {
        if let Some(mut deformable) = params.deformable.iter_mut().next() {
            deformable.corners = [
                Vec2::new(corners[0].x, corners[0].y),
                Vec2::new(corners[1].x, corners[1].y),
                Vec2::new(corners[2].x, corners[2].y),
                Vec2::new(corners[3].x, corners[3].y),
            ];
            deformable.is_dirty = true;
        }
    }

    // Ui settings

    if let Some(ui_scale) = env.ui_scale {
        params.triangulation.scale = ui_scale
    }

    // 5. Video sync offset
    if params.log_state.video_sync_offset_ms != env.video_sync_offset_ms {
        params.log_state.video_sync_offset_ms = env.video_sync_offset_ms;
        log_changed = true;
    }
    if log_changed {
        params.log_state.request_load = true;
    }

    if let Some(kind) = env.provider {
        params.provider.kind = kind;
    }
}

pub fn export_environment(params: &mut ConfigUiState, target_env_name: String) {
    if target_env_name.trim().is_empty() {
        params.config.status_message =
            Some(("Environment name cannot be empty".to_string(), false));
        return;
    }

    // 1. Anchors
    let mut anchors = BTreeMap::new();
    for (&id, &pos) in params.triangulation.anchors.iter() {
        anchors.insert(id.to_string(), AnchorConfig { x: pos.x, y: pos.y });
    }

    // 2. Log provider recording name and video path
    let recording_name = if params.log_state.recording_name.trim().is_empty() {
        None
    } else {
        Some(params.log_state.recording_name.clone())
    };

    let video_path = if params.log_state.video_name.trim().is_empty() {
        None
    } else {
        Some(params.log_state.video_name.clone())
    };

    // 3. Video transform (position, scale, rotation in degrees)
    let (video_position, video_scale, video_rotation) =
        if let Some((_, transform, _)) = params.videosprite.iter().next() {
            let (rx, ry, rz) = transform.rotation.to_euler(EulerRot::XYZ);
            (
                Some(Vec3Config {
                    x: transform.translation.x,
                    y: transform.translation.y,
                    z: transform.translation.z,
                }),
                Some(Vec3Config {
                    x: transform.scale.x,
                    y: transform.scale.y,
                    z: transform.scale.z,
                }),
                Some(Vec3Config {
                    x: rx.to_degrees(),
                    y: ry.to_degrees(),
                    z: rz.to_degrees(),
                }),
            )
        } else {
            (None, None, None)
        };

    // 4. Deformable corners
    let deformable_corners = if let Some(deformable) = params.deformable.iter().next() {
        Some([
            Vec2Config {
                x: deformable.corners[0].x,
                y: deformable.corners[0].y,
            },
            Vec2Config {
                x: deformable.corners[1].x,
                y: deformable.corners[1].y,
            },
            Vec2Config {
                x: deformable.corners[2].x,
                y: deformable.corners[2].y,
            },
            Vec2Config {
                x: deformable.corners[3].x,
                y: deformable.corners[3].y,
            },
        ])
    } else {
        None
    };

    let ui_scale = params.triangulation.scale;

    // 5. Video sync offset
    let video_sync_offset_ms = params.log_state.video_sync_offset_ms;

    let new_env = NamedEnv {
        anchors,
        recording_name,
        video_path,
        video_position,
        video_scale,
        video_rotation,
        deformable_corners,
        ui_scale: Some(ui_scale),
        video_sync_offset_ms,
        provider: Some(params.provider.kind),
    };

    if params.config.set_as_default_on_export {
        params.configfile.default_env = Some(target_env_name.clone());
    }

    params
        .configfile
        .envs
        .insert(target_env_name.clone(), new_env);

    // Save to config.yaml
    let mut to_save = params.configfile.clone();
    if target_env_name != "default" {
        if let Some(def) = to_save.envs.get("default") {
            if def.is_empty() {
                to_save.envs.remove("default");
            }
        }
    }

    match serde_yaml::to_string(&to_save) {
        Ok(yaml) => match fs::write("config.yaml", yaml) {
            Ok(()) => {
                params.config.status_message = Some((
                    format!("Exported to \"{}\" in config.yaml", target_env_name),
                    true,
                ));
                params.config.envname = target_env_name;
                params.config.new_env_name.clear();
            }
            Err(e) => {
                params.config.status_message =
                    Some((format!("Failed to write config.yaml: {e}"), false));
            }
        },
        Err(e) => {
            params.config.status_message = Some((format!("Serialization error: {e}"), false));
        }
    }
}

pub fn set_default_environment(params: &mut ConfigUiState, default_name: Option<String>) {
    params.configfile.default_env = default_name.clone();

    // Save to config.yaml
    let mut to_save = params.configfile.clone();
    if let Some(ref target) = default_name {
        if target != "default" {
            if let Some(def) = to_save.envs.get("default") {
                if def.is_empty() {
                    to_save.envs.remove("default");
                }
            }
        }
    }

    match serde_yaml::to_string(&to_save) {
        Ok(yaml) => match fs::write("config.yaml", yaml) {
            Ok(()) => {
                let name_display = default_name.unwrap_or_else(|| "None".to_string());
                params.config.status_message = Some((
                    format!("Set \"{}\" as default in config.yaml", name_display),
                    true,
                ));
            }
            Err(e) => {
                params.config.status_message =
                    Some((format!("Failed to write config.yaml: {e}"), false));
            }
        },
        Err(e) => {
            params.config.status_message = Some((format!("Serialization error: {e}"), false));
        }
    }
}

pub fn config_ui(ui: &mut Ui, mut params: ConfigUiState) {
    if !params.config.initial_loaded {
        load_environment(&mut params);
        params.config.initial_loaded = true;
    }

    ui.label(egui::RichText::new("Environment Configuration").heading());

    let oldenv = params.config.envname.clone();
    let envnames: Vec<String> = params.configfile.envs.keys().cloned().collect();

    ui.horizontal(|ui| {
        ui.label("Active:");
        let default_env = params.configfile.default_env.clone();
        egui::ComboBox::from_id_salt("active_environment_select")
            .selected_text(&params.config.envname)
            .show_ui(ui, |ui| {
                for envname in &envnames {
                    let label = if default_env.as_deref() == Some(envname) {
                        format!("{} ★", envname)
                    } else {
                        envname.clone()
                    };
                    ui.selectable_value(&mut params.config.envname, envname.clone(), label);
                }
            });

        if ui
            .button("Reload")
            .on_hover_text("Reload settings for the active environment")
            .clicked()
        {
            load_environment(&mut params);
            params.config.status_message = Some((
                format!("Reloaded environment \"{}\"", params.config.envname),
                true,
            ));
        }
    });

    ui.horizontal(|ui| {
        ui.label("Default on launch:");
        let current_default = params
            .configfile
            .default_env
            .clone()
            .unwrap_or_else(|| "None".to_string());
        egui::ComboBox::from_id_salt("default_environment_select")
            .selected_text(&current_default)
            .show_ui(ui, |ui| {
                let no_default = params.configfile.default_env.is_none();
                if ui.selectable_label(no_default, "— None —").clicked() && !no_default {
                    set_default_environment(&mut params, None);
                }
                for envname in &envnames {
                    let is_current = params.configfile.default_env.as_deref() == Some(envname);
                    if ui.selectable_label(is_current, envname).clicked() && !is_current {
                        set_default_environment(&mut params, Some(envname.clone()));
                    }
                }
            });

        if params.configfile.default_env.as_ref() != Some(&params.config.envname) {
            if ui
                .button("Set Active as Default")
                .on_hover_text("Save active environment as the default to load on startup")
                .clicked()
            {
                let name = params.config.envname.clone();
                set_default_environment(&mut params, Some(name));
            }
        } else {
            ui.label(egui::RichText::new("★ Default").color(egui::Color32::GOLD));
        }
    });

    if params.config.envname != oldenv {
        load_environment(&mut params);
        params.config.status_message = None;
    }

    ui.add_space(4.0);
    ui.separator();
    ui.label(egui::RichText::new("Export Environment").strong());

    let current_label = format!("Current ({})", params.config.envname);
    ui.horizontal(|ui| {
        ui.radio_value(
            &mut params.config.export_target_mode,
            ExportTargetMode::Current,
            current_label,
        );
        ui.radio_value(
            &mut params.config.export_target_mode,
            ExportTargetMode::Existing,
            "Existing",
        );
        ui.radio_value(
            &mut params.config.export_target_mode,
            ExportTargetMode::New,
            "New",
        );
    });

    let target_env_name = match params.config.export_target_mode {
        ExportTargetMode::Current => params.config.envname.clone(),
        ExportTargetMode::Existing => {
            ui.horizontal(|ui| {
                ui.label("Select:");
                egui::ComboBox::from_id_salt("export_existing_select")
                    .selected_text(if params.config.existing_target_name.is_empty() {
                        "Choose..."
                    } else {
                        &params.config.existing_target_name
                    })
                    .show_ui(ui, |ui| {
                        for envname in &envnames {
                            ui.selectable_value(
                                &mut params.config.existing_target_name,
                                envname.clone(),
                                envname.clone(),
                            );
                        }
                    });
            });
            params.config.existing_target_name.clone()
        }
        ExportTargetMode::New => {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut params.config.new_env_name);
            });
            params.config.new_env_name.trim().to_string()
        }
    };

    ui.checkbox(
        &mut params.config.set_as_default_on_export,
        "Set as default on launch",
    );

    let can_export = !target_env_name.is_empty();
    let export_button = egui::Button::new(if can_export {
        format!("Export to \"{}\"", target_env_name)
    } else {
        "Export (select or enter an environment)".to_string()
    });

    if ui.add_enabled(can_export, export_button).clicked() {
        export_environment(&mut params, target_env_name);
    }

    if let Some((msg, success)) = &params.config.status_message {
        let color = if *success {
            egui::Color32::from_rgb(100, 220, 100)
        } else {
            egui::Color32::from_rgb(240, 100, 100)
        };
        ui.label(egui::RichText::new(msg).color(color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_existing_anchors() {
        let yaml = r#"
envs:
  gras1:
    anchors:
      1:
        x: 0.0
        y: 0.0
      2:
        x: 10.0
        y: 0.0
      3:
        x: 5.0
        y: 8.0
"#;
        let config: ConfigFile = serde_yaml::from_str(yaml).expect("Failed to parse");
        assert!(config.envs.contains_key("gras1"));
        let env = &config.envs["gras1"];
        assert_eq!(env.anchors.len(), 3);
        assert_eq!(env.anchors["1"], AnchorConfig { x: 0.0, y: 0.0 });
        assert_eq!(env.anchors["2"], AnchorConfig { x: 10.0, y: 0.0 });
        assert_eq!(env.anchors["3"], AnchorConfig { x: 5.0, y: 8.0 });
    }

    #[test]
    fn test_roundtrip_all_fields() {
        let mut anchors = BTreeMap::new();
        anchors.insert("1".to_string(), AnchorConfig { x: 1.0, y: 2.0 });

        let env = NamedEnv {
            anchors,
            recording_name: Some("test_rec".to_string()),
            video_path: Some("./assets/video.mp4".to_string()),
            video_position: Some(Vec3Config {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            }),
            video_scale: Some(Vec3Config {
                x: 0.5,
                y: 0.5,
                z: 1.0,
            }),
            video_rotation: Some(Vec3Config {
                x: 0.0,
                y: 0.0,
                z: 45.0,
            }),
            deformable_corners: Some([
                Vec2Config { x: -50.0, y: 50.0 },
                Vec2Config { x: 50.0, y: 50.0 },
                Vec2Config { x: 50.0, y: -50.0 },
                Vec2Config { x: -50.0, y: -50.0 },
            ]),
            ui_scale: None,
            video_sync_offset_ms: None,
            provider: None,
        };

        let mut envs = BTreeMap::new();
        envs.insert("test_env".to_string(), env.clone());
        let file = ConfigFile {
            default_env: None,
            envs,
        };

        let yaml = serde_yaml::to_string(&file).expect("Failed to serialize");
        let parsed: ConfigFile = serde_yaml::from_str(&yaml).expect("Failed to deserialize");

        let parsed_env = &parsed.envs["test_env"];
        assert_eq!(parsed_env.recording_name.as_deref(), Some("test_rec"));
        assert_eq!(parsed_env.video_path.as_deref(), Some("./assets/video.mp4"));
        assert_eq!(
            parsed_env.video_position,
            Some(Vec3Config {
                x: 1.0,
                y: 2.0,
                z: 3.0
            })
        );
        assert_eq!(
            parsed_env.video_scale,
            Some(Vec3Config {
                x: 0.5,
                y: 0.5,
                z: 1.0
            })
        );
        assert_eq!(
            parsed_env.video_rotation,
            Some(Vec3Config {
                x: 0.0,
                y: 0.0,
                z: 45.0
            })
        );
        assert_eq!(
            parsed_env.deformable_corners,
            Some([
                Vec2Config { x: -50.0, y: 50.0 },
                Vec2Config { x: 50.0, y: 50.0 },
                Vec2Config { x: 50.0, y: -50.0 },
                Vec2Config { x: -50.0, y: -50.0 },
            ])
        );
    }

    #[test]
    fn test_load_actual_config_yaml() {
        let yaml = fs::read_to_string("config.yaml").expect("Failed to read config.yaml");
        let config: ConfigFile = serde_yaml::from_str(&yaml).expect("Failed to parse config.yaml");
        assert!(config.envs.contains_key("gras1"));
        let gras1 = &config.envs["gras1"];
        assert_eq!(gras1.recording_name.as_deref(), Some("gras1"));
        assert_eq!(gras1.video_path.as_deref(), Some("./assets/gras1.mp4"));
        assert_eq!(gras1.anchors.len(), 3);
        assert!(gras1.video_position.is_some());
        assert!(gras1.video_scale.is_some());
        assert!(gras1.video_rotation.is_some());
        assert!(gras1.deformable_corners.is_some());
    }
}
