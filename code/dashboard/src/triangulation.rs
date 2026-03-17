use std::collections::HashSet;

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
    pub distance: Option<f32>,
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
    LogFiles,
}

impl std::fmt::Display for DistanceProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manual => write!(f, "Manual"),
            Self::Mqtt => write!(f, "MQTT"),
            Self::LogFiles => write!(f, "Log Files"),
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
            kind: DistanceProviderKind::LogFiles,
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
            .add_systems(Startup, setup)
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
    pub distances: HashMap<usize, Option<f32>>,
    pub solutions: Option<(Vec2, Vec2)>,
    pub estimated_position: Option<Vec2>,
    pub show_radii: bool,
    pub use_second_solution: bool,
}

/// Holds anchor positions, measured distances, and the computed result.
#[derive(Resource)]
pub struct TriangulationState {
    pub anchors: HashMap<usize, Vec2>,
    pub tagstates: HashMap<usize, TagState>,
    /// The two candidate solutions (if they exist for 2 anchors).
    /// Which solution to display: false = first, true = second.
    /// Visual scale: pixels per unit distance.
    pub scale: f32,
    pub bgscale: f32,
    pub show_extradebug: bool, // show extra debug ui like lines from anchors to tags
    pub use_second: HashSet<(usize, usize)>, // (anchorid1, anchorid2) -> use second solution when these 2 anchors are available
    pub use_lut: bool,
    pub lut: Vec<(f32, f32)>, // (measured, actual)
}

fn apply_lut(distance: f32, lut: &[(f32, f32)]) -> f32 {
    if lut.is_empty() {
        return distance;
    }
    if lut.len() == 1 {
        return lut[0].1;
    }

    // Assuming lut is sorted by measured distance (lut[i].0)
    if distance <= lut[0].0 {
        return lut[0].1;
    }
    let last = lut.len() - 1;
    if distance >= lut[last].0 {
        return lut[last].1;
    }

    for i in 0..last {
        let (m1, a1) = lut[i];
        let (m2, a2) = lut[i + 1];
        if distance >= m1 && distance <= m2 {
            let t = (distance - m1) / (m2 - m1);
            return a1 + t * (a2 - a1);
        }
    }
    distance
}

impl Default for TriangulationState {
    fn default() -> Self {
        let mut anchors = HashMap::new();

        // parking 1
        // anchors.insert(1, Vec2::new(0.0, -0.5));
        // anchors.insert(2, Vec2::new(32.8, 0.0));
        // anchors.insert(3, Vec2::new(39.7, 12.7));

        // gras 1
        anchors.insert(1, Vec2::new(11.8, 18.4));
        anchors.insert(2, Vec2::new(22.9, 0.0));
        anchors.insert(3, Vec2::new(0., 0.));

        let mut use_second_set = HashSet::new();
        use_second_set.insert((1, 2));
        // use_second_map.insert((2, 3));
        // use_second_set.insert((1, 3));

        Self {
            anchors,
            tagstates: HashMap::new(),
            scale: 10.0,
            bgscale: 0.05,
            use_second: use_second_set,
            show_extradebug: true,
            use_lut: false,
            lut: vec![(0.0, 0.0), (10.0, 10.0)],
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
            distances: HashMap::new(),
            solutions: None,
            estimated_position: None,
            show_radii: false,
            use_second_solution: false,
        });
        tagstate.distances.insert(ev.anchor_id, ev.distance);
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
fn trilaterate_2d(a: Vec2, inr1: f32, b: Vec2, inr2: f32) -> Option<(Vec2, Vec2)> {
    let d_vec = b - a;
    let d = d_vec.length();
    let mut r1 = inr1;
    let mut r2 = inr2;

    if d > r1 + r2 {
        let deficit = (d - (r1 + r2)) / 2. + 0.01;
        r1 += deficit;
        r2 += deficit;
    } else if d < (r1 - r2).abs() {
        return None;
    } else if d > r1 + r2 || d < (r1 - r2).abs() || d < 1e-9 {
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

/// Arbitrary number of anchors least squares approach.
/// Returns the estimated position using gradient descent to minimize
/// the sum of squared differences between measured and geometric distances.
fn multilaterate_least_squares(anchors: &[(Vec2, f32)]) -> Option<Vec2> {
    if anchors.is_empty() {
        return None;
    }
    if anchors.len() == 1 {
        return Some(anchors[0].0);
    }

    // Use trilateration exactly for 2 anchors, returning the midpoint of solutions or best guess
    // but a gradient descent works reasonably well to find a viable minimum anyway.

    // Initial guess: average of all anchor positions
    let mut pos = Vec2::ZERO;
    for (a, _) in anchors {
        pos += *a;
    }
    if !anchors.is_empty() {
        pos /= anchors.len() as f32;
    }

    let learning_rate = 0.5 / anchors.len() as f32;
    for _ in 0..200 {
        let mut grad = Vec2::ZERO;
        for (a, r) in anchors {
            let diff = pos - *a;
            let dist = diff.length();
            if dist > 0.001 {
                // error = dist - r
                // d(error^2)/dpos = 2 * (dist - r) * (diff / dist)
                grad += 2.0 * (dist - r) * (diff / dist);
            }
        }
        pos -= learning_rate * grad;
    }

    Some(pos)
}

// ---------------------------------------------------------------------------
// UI
// ---------------------------------------------------------------------------

/// egui window for editing anchor positions, distances, and provider selection.
fn triangulation_ui(
    mut contexts: EguiContexts,
    mut state: ResMut<TriangulationState>,
    mut provider: ResMut<ActiveDistanceProvider>,
    mut background: Query<&mut Transform, With<BackgroundImage>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let ss = state.use_second.clone();

    egui::Window::new("Calibration LUT")
        .default_width(250.0)
        .show(ctx, |ui| {
            ui.checkbox(&mut state.use_lut, "Enable Calibration LUT");
            ui.separator();

            let mut to_remove = None;
            for (i, sample) in state.lut.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label("Measured:");
                    ui.add(egui::DragValue::new(&mut sample.0).speed(0.1));
                    ui.label("Actual:");
                    ui.add(egui::DragValue::new(&mut sample.1).speed(0.1));
                    if ui.button("X").clicked() {
                        to_remove = Some(i);
                    }
                });
            }
            if let Some(i) = to_remove {
                state.lut.remove(i);
            }
            if ui.button("Add Sample").clicked() {
                let last_measured = state.lut.last().map(|s| s.0).unwrap_or(0.0);
                let last_actual = state.lut.last().map(|s| s.1).unwrap_or(0.0);
                state.lut.push((last_measured + 1.0, last_actual + 1.0));
            }

            // Keep it sorted by measured distance
            state
                .lut
                .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        });

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

            let mut to_remove = None;
            let mut anchors_sorted: Vec<_> = state.anchors.iter_mut().collect();
            anchors_sorted.sort_by_key(|(id, _)| **id);

            for (&id, pos) in anchors_sorted {
                ui.horizontal(|ui| {
                    ui.label(format!("Anchor {} x:", id));
                    ui.add(egui::DragValue::new(&mut pos.x).speed(0.05));
                    ui.label("y:");
                    ui.add(egui::DragValue::new(&mut pos.y).speed(0.05));
                    if ui.button("X").clicked() {
                        to_remove = Some(id);
                    }
                });
            }
            if let Some(id) = to_remove {
                state.anchors.remove(&id);
                for tagstate in state.tagstates.values_mut() {
                    tagstate.distances.remove(&id);
                }
            }

            if ui.button("Add Anchor").clicked() {
                let next_id = state.anchors.keys().max().copied().unwrap_or(0) + 1;
                state.anchors.insert(next_id, Vec2::new(0.0, 0.0));
            }

            ui.separator();

            // ----- Distances -----
            ui.heading("Distances");

            let manual = provider.kind == DistanceProviderKind::Manual;

            let anchor_keys: Vec<_> = state.anchors.keys().copied().collect();
            let anchors_clone = state.anchors.clone(); // Clone to avoid multiple borrow issues
            let use_lut = state.use_lut;
            let lut = state.lut.clone();

            for (tag_id, tagstate) in state.tagstates.iter_mut() {
                ui.label(format!("Tag {}", tag_id));
                ui.checkbox(&mut tagstate.show_radii, "Show radii");
                ui.checkbox(&mut tagstate.use_second_solution, "Use second solution");

                let mut distances_sorted: Vec<_> = tagstate.distances.iter_mut().collect();
                distances_sorted.sort_by_key(|(id, _)| **id);

                for (&anchor_id, opt_dist) in distances_sorted {
                    ui.horizontal(|ui| {
                        ui.label(format!("d(Anchor {} -> P):", anchor_id));
                        if manual {
                            let mut dist = opt_dist.unwrap_or(0.0);
                            let response = ui.add(
                                egui::DragValue::new(&mut dist)
                                    .speed(0.05)
                                    .range(0.0..=f32::MAX),
                            );
                            if response.changed() {
                                *opt_dist = Some(dist);
                            }
                        } else {
                            if let Some(dist) = opt_dist {
                                ui.label(format!("{:.4}", dist));
                            } else {
                                ui.colored_label(
                                    egui::Color32::from_rgb(255, 100, 100),
                                    "(out of range)",
                                );
                            }
                        }
                    });
                }

                // For manual mode, allow adding distances for newly added anchors
                if manual {
                    for &anchor_id in &anchor_keys {
                        if !tagstate.distances.contains_key(&anchor_id) {
                            if ui
                                .button(format!("Add distance for Anchor {}", anchor_id))
                                .clicked()
                            {
                                tagstate.distances.insert(anchor_id, Some(0.0));
                            }
                        }
                    }
                }

                // ----- Solve -----
                let dtr = 25.;

                // Collect available data for solving
                let mut valid_measurements = Vec::new();
                for (&anchor_id, &opt_dist) in tagstate.distances.iter() {
                    if let Some(mut dist) = opt_dist {
                        if use_lut {
                            dist = apply_lut(dist, &lut);
                        }
                        if let Some(&pos) = anchors_clone.get(&anchor_id) {
                            valid_measurements.push((pos, dist));
                        }
                    }
                }

                if valid_measurements.len() == 2 {
                    tagstate.solutions = trilaterate_2d(
                        valid_measurements[0].0,
                        valid_measurements[0].1,
                        valid_measurements[1].0,
                        valid_measurements[1].1,
                    );

                    let anchorsinrange: Vec<usize> = tagstate
                        .distances
                        .iter()
                        .filter_map(|(&k, v)| v.map(|_| k))
                        .collect();

                    let sorted_tuple = (anchorsinrange.len() == 2).then(|| {
                        let (a, b) = (anchorsinrange[0], anchorsinrange[1]);
                        (a.min(b), a.max(b))
                    });

                    tagstate.estimated_position = if let Some(uskey) = sorted_tuple
                        && ss.contains(&uskey)
                    {
                        tagstate.solutions.map(|s| s.1)
                    } else {
                        tagstate.solutions.map(|s| s.0)
                    };
                } else if valid_measurements.len() >= 2 {
                    tagstate.solutions = None;
                    tagstate.estimated_position = multilaterate_least_squares(&valid_measurements);
                } else {
                    tagstate.solutions = None;
                    tagstate.estimated_position = None;
                }

                if let Some(pos) = tagstate.estimated_position {
                    ui.label(format!("Estimated Position: ({:.3}, {:.3})", pos.x, pos.y));
                } else {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 100, 100),
                        "Not enough data or no intersection",
                    );
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

            ui.checkbox(&mut state.show_extradebug, "Show debug UI");

            if ui.button("Auto scale").clicked() {
                // Maximum distance between 2 points for auto scaling
                let mut maxdist = 1.0_f32; // Fallback
                for (&id1, &a1) in state.anchors.iter() {
                    for (&id2, &a2) in state.anchors.iter() {
                        if id1 != id2 {
                            maxdist = maxdist.max((a1 - a2).length());
                        }
                    }
                }

                for (_tag_id, tagstate) in &state.tagstates {
                    if let Some(pos) = tagstate.estimated_position {
                        for (&_id, &a) in state.anchors.iter() {
                            maxdist = maxdist.max((a - pos).length());
                        }
                    }
                }

                state.scale = TriangulationState::default().scale / (maxdist / 2.0);
            }

            for (mut transform) in &mut background {
                // background position and scale
                ui.horizontal(|ui| {
                    ui.label(format!("Background:"));
                    ui.add(egui::DragValue::new(&mut transform.translation.x).speed(0.05));
                    ui.label("y:");
                    ui.add(egui::DragValue::new(&mut transform.translation.y).speed(0.05));
                    ui.label("scale:");

                    ui.add(egui::DragValue::new(&mut state.bgscale).speed(0.001));
                    transform.scale.x = state.bgscale * state.scale;
                    transform.scale.y = state.bgscale * state.scale;
                    transform.scale.z = state.bgscale * state.scale;
                });
            }
        });
}

#[derive(Component)]
struct BackgroundImage; // Component for overlayed background image (map or sattelite pic of location)

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Sprite::from_image(asset_server.load("parking.png")),
        BackgroundImage,
        Transform::from_xyz(206., 120., 0.),
    ));
}

/// Draw anchors, distance circles, and the estimated position using gizmos.
fn draw_triangulation(state: Res<TriangulationState>, mut gizmos: Gizmos) {
    let s = state.scale;

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

    // --- Anchors ---
    for (&id, &pos) in state.anchors.iter() {
        let screen_pos = pos * s;
        // Generate a pseudo-random color based on ID
        let hue = ((id as f32) * 137.5) % 360.0;
        let color = Color::hsl(hue, 0.8, 0.5);
        gizmos.circle_2d(screen_pos, 8.0, color);
    }

    // // --- Lines between anchors ---
    // let anchor_ids: Vec<_> = state.anchors.keys().copied().collect();
    // for i in 0..anchor_ids.len() {
    //     for j in (i + 1)..anchor_ids.len() {
    //         let p1 = state.anchors[&anchor_ids[i]] * s;
    //         let p2 = state.anchors[&anchor_ids[j]] * s;
    //         gizmos.line_2d(p1, p2, Color::srgba(1.0, 1.0, 1.0, 0.15));
    //     }
    // }

    for (tag_id, tagstate) in state.tagstates.iter() {
        // radii if enabled
        if tagstate.show_radii || tagstate.estimated_position.is_none() {
            for (&anchor_id, &opt_dist) in tagstate.distances.iter() {
                if let Some(dist) = opt_dist {
                    if let Some(&anchor_pos) = state.anchors.get(&anchor_id) {
                        let anchor_screen = anchor_pos * s;
                        let hue = ((anchor_id as f32) * 137.5) % 360.0;
                        let color = Color::hsla(hue, 0.8, 0.5, 0.35);
                        gizmos.circle_2d(anchor_screen, dist * s, color);
                    }
                }
            }
        }

        // --- Solutions ---
        // For exactly 2 anchors we might have 2 exact solutions, dim one if we want
        if let Some((p1, p2)) = tagstate.solutions {
            let p1_screen = p1 * s;
            let p2_screen = p2 * s;

            let (_chosen_screen, other_screen) = if tagstate.use_second_solution {
                (p2_screen, p1_screen)
            } else {
                (p1_screen, p2_screen)
            };

            // Dimmed alternate solution
            gizmos.circle_2d(other_screen, 5.0, Color::srgba(1.0, 1.0, 0.0, 0.25));
        }

        // for one solution (>=3 anchors in range)
        if let Some(pos) = tagstate.estimated_position {
            let chosen_screen = pos * s;
            let hue = (((tag_id + 4) as f32) * 137.5) % 360.0;
            let color = Color::hsla(hue, 0.8, 0.5, 0.5);
            // Chosen estimated position (yellow)
            gizmos.circle_2d(chosen_screen, 7.0, color);

            // Cross-hair on chosen position
            let ch = 12.0;
            gizmos.line_2d(
                chosen_screen + Vec2::new(-ch, 0.0),
                chosen_screen + Vec2::new(ch, 0.0),
                color,
            );
            gizmos.line_2d(
                chosen_screen + Vec2::new(0.0, -ch),
                chosen_screen + Vec2::new(0.0, ch),
                color,
            );

            // Lines from anchors to chosen point
            if state.show_extradebug {
                for (&anchor_id, distance) in tagstate.distances.iter() {
                    if distance.is_none() {
                        continue;
                    }
                    if let Some(&anchor_pos) = state.anchors.get(&anchor_id) {
                        let anchor_screen = anchor_pos * s;
                        let hue = ((anchor_id as f32) * 137.5) % 360.0;
                        let color = Color::hsla(hue, 0.8, 0.5, 0.5);
                        gizmos.line_2d(anchor_screen, chosen_screen, color);
                    }
                }
            }
        }
    }
}
