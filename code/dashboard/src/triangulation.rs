use std::usize::MAX;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};

/// Plugin that triangulates a point in 2D from distances to two anchors
/// and visualizes the result.
pub struct TriangulationPlugin;

impl Plugin for TriangulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TriangulationState>()
            // .add_plugins(EguiPlugin::default())
            .add_systems(Update, (draw_triangulation))
            .add_systems(EguiPrimaryContextPass, (ui_example_system, triangulation_ui));
    }
}

/// Holds anchor positions, measured distances, and the computed result.
#[derive(Resource)]
pub struct TriangulationState {
    pub anchor_a: Vec2,
    pub anchor_b: Vec2,
    pub distance_a: f32,
    pub distance_b: f32,
    /// The two candidate solutions (if they exist).
    pub solutions: Option<(Vec2, Vec2)>,
    /// Which solution to display: false = first, true = second.
    pub use_second_solution: bool,
    /// Visual scale: pixels per unit distance.
    pub scale: f32,
}

impl Default for TriangulationState {
    fn default() -> Self {
        Self {
            anchor_a: Vec2::new(-1.0, 0.0),
            anchor_b: Vec2::new(1.0, 0.0),
            distance_a: 1.5,
            distance_b: 1.5,
            solutions: None,
            use_second_solution: false,
            scale: 150.0,
        }
    }
}

fn ui_example_system(mut contexts: EguiContexts) -> Result {
    egui::Window::new("Hello").show(contexts.ctx_mut()?, |ui| {
        ui.label("world");
    });
    Ok(())
}

/// 2D trilateration: find the intersection of two circles.
///
/// Circle 1: center `a`, radius `r1`
/// Circle 2: center `b`, radius `r2`
///
/// Returns `None` if circles don't intersect, otherwise two points
/// (which may be identical when tangent).
fn trilaterate_2d(a: Vec2, r1: f32, b: Vec2, r2: f32) -> Option<(Vec2, Vec2)> {
    let d_vec = b - a;
    let d = d_vec.length();

    // No solution if circles are too far apart or one contains the other
    if d > r1 + r2 || d < (r1 - r2).abs() || d < 1e-9 {
        return None;
    }

    // Distance from a along the line a->b to the midpoint of intersections
    let x = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
    let y_sq = r1 * r1 - x * x;
    let y = if y_sq < 0.0 { 0.0 } else { y_sq.sqrt() };

    // Unit vectors: ex along a->b, ey perpendicular
    let ex = d_vec / d;
    let ey = Vec2::new(-ex.y, ex.x);

    let p1 = a + ex * x + ey * y;
    let p2 = a + ex * x - ey * y;

    Some((p1, p2))
}

/// egui window for editing anchor positions and distances.
fn triangulation_ui(mut contexts: EguiContexts, mut state: ResMut<TriangulationState>) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    egui::Window::new("Triangulation")
        .default_width(280.0)
        .show(ctx, |ui| {
            ui.heading("Anchors");
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Anchor A  x:");
                ui.add(egui::DragValue::new(&mut state.anchor_a.x).speed(0.05));
                ui.label("y:");
                ui.add(egui::DragValue::new(&mut state.anchor_a.y).speed(0.05));
            });

            ui.horizontal(|ui| {
                ui.label("Anchor B  x:");
                ui.add(egui::DragValue::new(&mut state.anchor_b.x).speed(0.05));
                ui.label("y:");
                ui.add(egui::DragValue::new(&mut state.anchor_b.y).speed(0.05));
            });

            ui.separator();
            ui.heading("Distances");

            ui.horizontal(|ui| {
                ui.label("d(A → P):");
                ui.add(
                    egui::DragValue::new(&mut state.distance_a)
                        .speed(0.05)
                        .range(0.0..=f32::MAX),
                );
            });
            ui.horizontal(|ui| {
                ui.label("d(B → P):");
                ui.add(
                    egui::DragValue::new(&mut state.distance_b)
                        .speed(0.05)
                        .range(0.0..=f32::MAX),
                );
            });

            ui.separator();
            ui.heading("Display");

            ui.horizontal(|ui| {
                ui.label("Scale (px/unit):");
                ui.add(
                    egui::DragValue::new(&mut state.scale)
                        .speed(1.0)
                        .range(10.0..=1000.0),
                );
            });

            // Solve
            state.solutions = trilaterate_2d(
                state.anchor_a,
                state.distance_a,
                state.anchor_b,
                state.distance_b,
            );

            ui.separator();
            match state.solutions {
                Some((p1, p2)) => {
                    ui.checkbox(&mut state.use_second_solution, "Use second solution");
                    let chosen = if state.use_second_solution { p2 } else { p1 };
                    ui.label(format!("Solution 1: ({:.3}, {:.3})", p1.x, p1.y));
                    ui.label(format!("Solution 2: ({:.3}, {:.3})", p2.x, p2.y));
                    ui.label(format!("Selected:   ({:.3}, {:.3})", chosen.x, chosen.y));
                }
                None => {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 100, 100),
                        "No intersection – check distances",
                    );
                }
            }
        });
}

/// Draw anchors, distance circles, and the estimated position using gizmos.
fn draw_triangulation(state: Res<TriangulationState>, mut gizmos: Gizmos) {
    let mut maxdist = ((state.anchor_a - state.anchor_b).length());

    if let Some((p1, p2)) = state.solutions {
        maxdist = maxdist.max((state.anchor_a - p1).length());
        maxdist = maxdist.max((state.anchor_b - p1).length());
        maxdist = maxdist.max((state.anchor_a - p2).length());
        maxdist = maxdist.max((state.anchor_b - p2).length());
    }
        
    // let s = state.scale;
    let s = state.scale / (maxdist / 2.0);

    let a_screen = state.anchor_a * s;
    let b_screen = state.anchor_b * s;

    // --- Grid / origin marker ---
    let grid_half = 5.0 * state.scale;
    gizmos.line_2d(
        Vec2::new(-grid_half, 0.0),
        Vec2::new(grid_half, 0.0),
        Color::srgba(0.3, 0.3, 0.3, 0.5),
    );
    gizmos.line_2d(
        Vec2::new(0.0, -grid_half),
        Vec2::new(0.0, grid_half),
        Color::srgba(0.3, 0.3, 0.3, 0.5),
    );

    // --- Anchor A (blue) ---
    gizmos.circle_2d(a_screen, 8.0, Color::srgb(0.2, 0.4, 1.0));
    gizmos.circle_2d(
        a_screen,
        state.distance_a * s,
        Color::srgba(0.2, 0.4, 1.0, 0.35),
    );

    // --- Anchor B (green) ---
    gizmos.circle_2d(b_screen, 8.0, Color::srgb(0.2, 1.0, 0.4));
    gizmos.circle_2d(
        b_screen,
        state.distance_b * s,
        Color::srgba(0.2, 1.0, 0.4, 0.35),
    );

    // --- Line between anchors ---
    gizmos.line_2d(a_screen, b_screen, Color::srgba(1.0, 1.0, 1.0, 0.25));

    // --- Solutions ---
    if let Some((p1, p2)) = state.solutions {
        let p1_screen = p1 * s;
        let p2_screen = p2 * s;

        let (chosen_screen, other_screen) = if state.use_second_solution {
            (p2_screen, p1_screen)
        } else {
            (p1_screen, p2_screen)
        };

        // Dimmed alternate solution
        gizmos.circle_2d(other_screen, 5.0, Color::srgba(1.0, 1.0, 0.0, 0.25));

        // Chosen estimated position (yellow)
        gizmos.circle_2d(chosen_screen, 7.0, Color::srgb(1.0, 1.0, 0.0));
        // Cross-hair on chosen position
        let ch = 12.0;
        gizmos.line_2d(
            chosen_screen + Vec2::new(-ch, 0.0),
            chosen_screen + Vec2::new(ch, 0.0),
            Color::srgb(1.0, 1.0, 0.0),
        );
        gizmos.line_2d(
            chosen_screen + Vec2::new(0.0, -ch),
            chosen_screen + Vec2::new(0.0, ch),
            Color::srgb(1.0, 1.0, 0.0),
        );

        // Lines from anchors to chosen point
        gizmos.line_2d(a_screen, chosen_screen, Color::srgba(0.2, 0.4, 1.0, 0.5));
        gizmos.line_2d(b_screen, chosen_screen, Color::srgba(0.2, 1.0, 0.4, 0.5));
    }
}
