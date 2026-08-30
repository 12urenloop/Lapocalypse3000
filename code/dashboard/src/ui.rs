use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use egui::{LayerId, Ui, UiBuilder};

use crate::{
    log_distance_provider::{LogDistanceUiState, log_sidepanel_ui},
    triangulation::{TriangulationUiState, lut_ui, triangulation_ui},
};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(EguiPrimaryContextPass, sidepanel_ui);
    }
}

fn sidepanel_ui(
    mut contexts: EguiContexts,
    commands: Commands,
    mut params: ParamSet<(LogDistanceUiState, TriangulationUiState)>,
    // mut log_params: LogDistanceUiState,
    // mut triangulation_params: TriangulationUiState,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let mut viewport_ui = Ui::new(
        ctx.clone(),
        "viewport".into(),
        UiBuilder::new()
            .layer_id(LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );

    egui::Panel::left("left_panel")
        .resizable(true)
        .show(&mut viewport_ui, |ui| {
            ui.label("Left resizeable panel: logs?");

            triangulation_ui(ui, params.p1());
            ui.collapsing("distance LUT", |ui| lut_ui(ui, params.p1()));

            ui.collapsing("Log provider", |ui| {
                log_sidepanel_ui(ui, commands, params.p0())
            });
            // log_sidepanel_ui(ui, commands, log_params);

            ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
        });
}
