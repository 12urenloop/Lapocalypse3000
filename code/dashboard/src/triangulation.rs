use bevy::{platform::collections::HashMap, prelude::*};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

// ---------------------------------------------------------------------------
// Distance measurement event — the universal interface for all providers
// ---------------------------------------------------------------------------

/// Any distance provider emits this event to feed a new measurement into the
/// triangulation system. `anchor_index` maps to the anchor list in
/// [`TriangulationState`] (0 = A, 1 = B, …).
#[derive(Event, Debug, Clone)]
pub struct DistanceMeasurement {
    pub anchor_id: usize,
    pub tag_id: usize,
    pub distance: f32,
}

// ---------------------------------------------------------------------------
// Provider selection
// ---------------------------------------------------------------------------

/// Identifies a distance-provider implementation.  Add new variants here when
/// you create a new provider plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DistanceProviderKind {
    #[default]
    Manual,
    Mqtt,
}

impl std::fmt::Display for DistanceProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manual => write!(f, "Manual"),
            Self::Mqtt => write!(f, "MQTT"),
        }
    }
}

/// Resource that controls which provider's measurements are accepted.
#[derive(Resource)]
pub struct ActiveDistanceProvider {
    pub kind: DistanceProviderKind,
    /// List of providers that have been registered (for the UI combo-box).
    pub available: Vec<DistanceProviderKind>,
}

impl Default for ActiveDistanceProvider {
    fn default() -> Self {
        Self {
            kind: DistanceProviderKind::Mqtt,
            // Manual is always available; other providers register themselves.
            available: vec![DistanceProviderKind::Manual],
        }
    }
}

// ---------------------------------------------------------------------------
// Triangulation plugin
// ---------------------------------------------------------------------------

/// Plugin that triangulates a point in 2D from distances to two anchors
/// and visualizes the result.
pub struct TriangulationPlugin;

impl Plugin for TriangulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TriangulationState>()
            .init_resource::<ActiveDistanceProvider>()
            .add_event::<DistanceMeasurement>()
            .add_systems(
                Update,
                (consume_distance_events, draw_triangulation).chain(),
            )
            .add_systems(EguiPrimaryContextPass, triangulation_ui);
    }
}

// ---------------------------------------------------------------------------
// Triangulation state
// ---------------------------------------------------------------------------

pub struct TagState {
    pub distance_a: f32,
    pub distance_b: f32,
    pub solutions: Option<(Vec2, Vec2)>,
    pub show_radii: bool,
}

/// Holds anchor positions, measured distances, and the computed result.
#[derive(Resource)]
pub struct TriangulationState {
    pub anchor_a: Vec2,
    pub anchor_b: Vec2,
    pub tagstates: HashMap<usize, TagState>,
    /// The two candidate solutions (if they exist).
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
            tagstates: HashMap::new(),
            use_second_solution: false,
            scale: 150.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Consume distance events from whichever provider is active
// ---------------------------------------------------------------------------

fn consume_distance_events(
    mut events: EventReader<DistanceMeasurement>,
    provider: Res<ActiveDistanceProvider>,
    mut state: ResMut<TriangulationState>,
) {
    // When in Manual mode the UI writes directly to state, so we skip events.
    if provider.kind == DistanceProviderKind::Manual {
        events.clear();
        return;
    }

    for ev in events.read() {
        let tagstate = state.tagstates.entry(ev.tag_id).or_insert(TagState {
            distance_a: 0.0,
            distance_b: 0.0,
            solutions: None,
            show_radii: false,
        });
        match ev.anchor_id {
            1 => tagstate.distance_a = ev.distance,
            2 => tagstate.distance_b = ev.distance,
            _ => warn!(
                "DistanceMeasurement for unknown anchor_index {}",
                ev.anchor_id
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Math
// ---------------------------------------------------------------------------

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

    if d > r1 + r2 || d < (r1 - r2).abs() || d < 1e-9 {
        return None;
    }

    let x = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
    let y_sq = r1 * r1 - x * x;
    let y = if y_sq < 0.0 { 0.0 } else { y_sq.sqrt() };

    let ex = d_vec / d;
    let ey = Vec2::new(-ex.y, ex.x);

    let p1 = a + ex * x + ey * y;
    let p2 = a + ex * x - ey * y;

    Some((p1, p2))
}

// ---------------------------------------------------------------------------
// UI
// ---------------------------------------------------------------------------

/// egui window for editing anchor positions, distances, and provider selection.
fn triangulation_ui(
    mut contexts: EguiContexts,
    mut state: ResMut<TriangulationState>,
    mut provider: ResMut<ActiveDistanceProvider>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::Window::new("Triangulation")
        .default_width(300.0)
        .show(ctx, |ui| {
            // ----- Provider selector -----
            ui.heading("Distance Provider");
            {
                let current_label = provider.kind.to_string();
                egui::ComboBox::from_label("Source")
                    .selected_text(&current_label)
                    .show_ui(ui, |ui| {
                        for &kind in &provider.available.clone() {
                            ui.selectable_value(&mut provider.kind, kind, kind.to_string());
                        }
                    });
            }

            ui.separator();

            // ----- Anchors (always editable) -----
            ui.heading("Anchors");

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

            // ----- Distances -----
            ui.heading("Distances");

            let manual = provider.kind == DistanceProviderKind::Manual;

            let anchor_a = state.anchor_a;
            let anchor_b = state.anchor_b;
            let mut use_second_solution = state.use_second_solution;

            ui.checkbox(&mut use_second_solution, "Use second solution");

            for (tag_id, tagstate) in state.tagstates.iter_mut() {
                ui.label(format!("Tag {}", tag_id));
                ui.checkbox(&mut tagstate.show_radii, "Show radii");

                ui.horizontal(|ui| {
                    ui.label("d(A -> P):");
                    if manual {
                        ui.add(
                            egui::DragValue::new(&mut tagstate.distance_a)
                                .speed(0.05)
                                .range(0.0..=f32::MAX),
                        );
                    } else {
                        ui.label(format!("{:.4}", tagstate.distance_a));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("d(B -> P):");
                    if manual {
                        ui.add(
                            egui::DragValue::new(&mut tagstate.distance_b)
                                .speed(0.05)
                                .range(0.0..=f32::MAX),
                        );
                    } else {
                        ui.label(format!("{:.4}", tagstate.distance_b));
                    }
                });

                // ----- Solve -----

                tagstate.solutions =
                    trilaterate_2d(anchor_a, tagstate.distance_a, anchor_b, tagstate.distance_b);

                match tagstate.solutions {
                    Some((p1, p2)) => {
                        let chosen = if use_second_solution { p2 } else { p1 };
                        ui.label(format!("Solution 1: ({:.3}, {:.3})", p1.x, p1.y));
                        ui.label(format!("Solution 2: ({:.3}, {:.3})", p2.x, p2.y));
                        ui.label(format!("Selected:   ({:.3}, {:.3})", chosen.x, chosen.y));
                    }
                    None => {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 100, 100),
                            "No intersection - check distances",
                        );
                    }
                }
                ui.separator();
            }

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

            if ui.button("Auto scale").clicked() {
                // Maximum distance between 2 points for auto scaling
                let mut maxdist = (state.anchor_a - state.anchor_b).length();
                for (tag_id, tagstate) in &state.tagstates {
                    if let Some((p1, p2)) = tagstate.solutions {
                        maxdist = maxdist.max((state.anchor_a - p1).length());
                        maxdist = maxdist.max((state.anchor_b - p1).length());
                        maxdist = maxdist.max((state.anchor_a - p2).length());
                        maxdist = maxdist.max((state.anchor_b - p2).length());
                    }
                }

                state.scale = TriangulationState::default().scale / (maxdist / 2.0);
            }
        });
}

/// Draw anchors, distance circles, and the estimated position using gizmos.
fn draw_triangulation(state: Res<TriangulationState>, mut gizmos: Gizmos) {
    let s = state.scale;

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

    // --- Anchor B (green) ---
    gizmos.circle_2d(b_screen, 8.0, Color::srgb(0.2, 1.0, 0.4));

    // --- Line between anchors ---
    gizmos.line_2d(a_screen, b_screen, Color::srgba(1.0, 1.0, 1.0, 0.25));

    for (tag_id, tagstate) in state.tagstates.iter() {
        if tagstate.show_radii || tagstate.solutions == None {
            gizmos.circle_2d(
                a_screen,
                tagstate.distance_a * s,
                Color::srgba(0.2, 0.4, 1.0, 0.35),
            );

            gizmos.circle_2d(
                b_screen,
                tagstate.distance_b * s,
                Color::srgba(0.2, 1.0, 0.4, 0.35),
            );
        }

        // --- Solutions ---
        if let Some((p1, p2)) = tagstate.solutions {
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
}
