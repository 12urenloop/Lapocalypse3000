use bevy::{ecs::system::SystemParam, platform::collections::HashMap, prelude::*};
use bevy_egui::egui;
use egui::Ui;
use serde::Deserialize;
use std::fs;

use crate::triangulation::TriangulationState;

#[derive(Debug, Deserialize, Resource)]
pub struct ConfigFile {
    envs: HashMap<String, NamedEnv>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NamedEnv {
    anchors: HashMap<String, AnchorConfig>,
}
impl Default for NamedEnv {
    fn default() -> Self {
        return NamedEnv {
            anchors: HashMap::new(),
        };
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct AnchorConfig {
    x: f32,
    y: f32,
}

pub struct ConfigPlugin;

impl Plugin for ConfigPlugin {
    fn build(&self, app: &mut App) {
        let yaml = fs::read_to_string("config.yaml").expect("Failed to read config.yaml");

        let mut config: ConfigFile =
            serde_yaml::from_str(&yaml).expect("Failed to parse config.yaml");
        config.envs.insert(
            "default".to_string(),
            NamedEnv {
                anchors: HashMap::new(),
            },
        );
        app.insert_resource(config).insert_resource(ConfigState {
            envname: "default".to_string(),
        });
    }
}

#[derive(Resource)]
pub struct ConfigState {
    envname: String,
}

#[derive(SystemParam)]
pub struct ConfigUiState<'w> {
    config: ResMut<'w, ConfigState>,
    configfile: Res<'w, ConfigFile>,
    triangulation: ResMut<'w, TriangulationState>,
}

pub fn config_ui(ui: &mut Ui, mut params: ConfigUiState) {
    ui.label("Environment:");

    let oldenv = params.config.envname.clone();
    let envnames = params.configfile.envs.keys().clone();
    egui::ComboBox::from_label("Environment")
        .selected_text(&params.config.envname)
        .show_ui(ui, |ui| {
            for envname in envnames {
                ui.selectable_value(&mut params.config.envname, envname.clone(), envname.clone());
            }
        });

    if params.config.envname != oldenv {
        params.triangulation.anchors.clear();
        for (name, anchor) in params
            .configfile
            .envs
            .get(&params.config.envname)
            .cloned()
            .unwrap_or_default()
            .anchors
        {
            params.triangulation.anchors.insert(
                name.parse::<usize>().unwrap_or_default(),
                Vec2 {
                    x: anchor.x,
                    y: anchor.y,
                },
            );
        }
    }
    return;
}
