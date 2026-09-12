use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy_egui::EguiContexts;

use crate::MainCamera;
use crate::triangulation::{TriangulationState, TriangulationUiState};
use crate::ui::{DefaultGizmos, set_gizmo_renderlayer};

/// Specifies which corners are interactive for dragging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum CornerDragMode {
    /// Drag the (inner) scaled corners. The underlying image corners are updated accordingly.
    #[default]
    Scaled,
    /// Drag the original full-size image corners directly.
    Original,
    /// Allow dragging either the inner scaled corners or the outer original corners,
    /// depending on which handle is clicked/hovered.
    Both,
}

/// Identifies whether a handle belongs to the original (outer) or scaled (inner) corners.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum HandleKind {
    Original,
    Scaled,
}

/// Component attached to entities whose 2D image/mesh can be deformed, scaled, and rotated
/// by dragging its 4 corners.
#[derive(Component, Debug, Clone)]
pub struct DeformableImage {
    /// Local 2D positions of the 4 corners:
    /// [0] Top-Left
    /// [1] Top-Right
    /// [2] Bottom-Right
    /// [3] Bottom-Left
    pub corners: [Vec2; 4],
    /// Mesh grid subdivisions (e.g. 16 for a 16x16 vertex grid)
    pub subdivisions: usize,
    /// Handle to the underlying Mesh asset
    pub mesh_handle: Handle<Mesh>,
    /// Set to true whenever `corners` are modified to trigger mesh vertex updates
    pub is_dirty: bool,
    /// Pick radius for corner handles in world units
    pub handle_radius: f32,
    /// Original width & height of the image rect
    pub size: Vec2,
    /// Whether interaction and gizmos are active
    pub enabled: bool,
    /// Optional corner dragging mode override for this entity.
    /// If None, the global `CornerDragState::drag_mode` is used.
    pub drag_mode: Option<CornerDragMode>,
}

#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct DeformableGizmos;

impl DeformableImage {
    /// Creates a default rectangular `DeformableImage` centered at (0,0) and generates its mesh.
    pub fn new_rect(
        size: Vec2,
        subdivisions: usize,
        meshes: &mut Assets<Mesh>,
    ) -> (Self, Handle<Mesh>) {
        let half_w = size.x / 2.0;
        let half_h = size.y / 2.0;
        let corners = [
            Vec2::new(-half_w, half_h),  // TL
            Vec2::new(half_w, half_h),   // TR
            Vec2::new(half_w, -half_h),  // BR
            Vec2::new(-half_w, -half_h), // BL
        ];

        let mesh = generate_deformable_mesh(&corners, subdivisions);
        let mesh_handle = meshes.add(mesh);

        let deformable = Self {
            corners,
            subdivisions,
            mesh_handle: mesh_handle.clone(),
            is_dirty: false,
            handle_radius: 20.0,
            size,
            enabled: true,
            drag_mode: None,
        };

        (deformable, mesh_handle)
    }

    /// Resets the 4 corners back to a standard rectangle centered at (0,0).
    pub fn reset_rect(&mut self) {
        let half_w = self.size.x / 2.0;
        let half_h = self.size.y / 2.0;
        self.corners = [
            Vec2::new(-half_w, half_h),
            Vec2::new(half_w, half_h),
            Vec2::new(half_w, -half_h),
            Vec2::new(-half_w, -half_h),
        ];
        self.is_dirty = true;
    }
}

/// Tracks the mouse interaction state for corner dragging.
#[derive(Resource, Debug)]
pub struct CornerDragState {
    pub active_entity: Option<Entity>,
    pub dragged_corner: Option<(usize, HandleKind)>,
    pub hovered_corner: Option<(Entity, usize, HandleKind)>,
    pub drag_mode: CornerDragMode,
}

impl Default for CornerDragState {
    fn default() -> Self {
        Self {
            active_entity: None,
            dragged_corner: None,
            hovered_corner: None,
            drag_mode: CornerDragMode::Scaled,
        }
    }
}

pub struct DeformableImagePlugin;

impl Plugin for DeformableImagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CornerDragState>()
            .add_systems(
                Update,
                (
                    handle_corner_drag,
                    update_deformable_mesh,
                    draw_corner_gizmos,
                )
                    .chain(),
            )
            .init_gizmo_group::<DeformableGizmos>();

        // Configure respective render layers
        let mut config_store = app.world_mut().resource_mut::<GizmoConfigStore>();

        let (config_a, _) = config_store.config_mut::<DeformableGizmos>();
        config_a.render_layers = RenderLayers::layer(1);
    }
}

/// Generates a grid mesh bilinearly interpolated from the 4 corner points.
fn generate_deformable_mesh(corners: &[Vec2; 4], subdivisions: usize) -> Mesh {
    let sub = subdivisions.max(1);
    let num_verts = (sub + 1) * (sub + 1);

    let mut positions = Vec::with_capacity(num_verts);
    let mut uvs = Vec::with_capacity(num_verts);
    let mut normals = Vec::with_capacity(num_verts);

    let c_tl = corners[0];
    let c_tr = corners[1];
    let c_br = corners[2];
    let c_bl = corners[3];

    for row in 0..=sub {
        let v = row as f32 / sub as f32; // 0.0 (top) to 1.0 (bottom)
        for col in 0..=sub {
            let u = col as f32 / sub as f32; // 0.0 (left) to 1.0 (right)

            // Bilinear quad interpolation formula
            let pos_2d = (1.0 - u) * (1.0 - v) * c_tl
                + u * (1.0 - v) * c_tr
                + u * v * c_br
                + (1.0 - u) * v * c_bl;

            positions.push([pos_2d.x, pos_2d.y, 0.0]);
            uvs.push([u, v]);
            normals.push([0.0, 0.0, 1.0]);
        }
    }

    let num_quads = sub * sub;
    let mut indices = Vec::with_capacity(num_quads * 6);

    for row in 0..sub {
        for col in 0..sub {
            let i0 = (row * (sub + 1) + col) as u32;
            let i1 = i0 + 1;
            let i2 = ((row + 1) * (sub + 1) + col) as u32;
            let i3 = i2 + 1;

            // Two triangles per quad cell
            indices.push(i0);
            indices.push(i1);
            indices.push(i3);

            indices.push(i0);
            indices.push(i3);
            indices.push(i2);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));

    mesh
}

/// Updates the mesh vertex positions whenever `is_dirty` is true.
fn update_deformable_mesh(
    mut deformable_query: Query<&mut DeformableImage>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for mut deformable in deformable_query.iter_mut() {
        if !deformable.is_dirty {
            continue;
        }

        if let Some(mut mesh) = meshes.get_mut(&deformable.mesh_handle) {
            let sub = deformable.subdivisions.max(1);
            let num_verts = (sub + 1) * (sub + 1);
            let mut positions = Vec::with_capacity(num_verts);

            let c_tl = deformable.corners[0];
            let c_tr = deformable.corners[1];
            let c_br = deformable.corners[2];
            let c_bl = deformable.corners[3];

            for row in 0..=sub {
                let v = row as f32 / sub as f32;
                for col in 0..=sub {
                    let u = col as f32 / sub as f32;
                    let pos_2d = (1.0 - u) * (1.0 - v) * c_tl
                        + u * (1.0 - v) * c_tr
                        + u * v * c_br
                        + (1.0 - u) * v * c_bl;
                    positions.push([pos_2d.x, pos_2d.y, 0.0]);
                }
            }

            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        }

        deformable.is_dirty = false;
    }
}

/// Computes the 4 corners scaled toward `scale_center(corners)` by `scale`.
pub fn compute_scaled_corners(corners: [Vec2; 4], scale: f32) -> [Vec2; 4] {
    let scalecenter = scale_center(corners);
    [
        corners[0] * scale + scalecenter * (1.0 - scale),
        corners[1] * scale + scalecenter * (1.0 - scale),
        corners[2] * scale + scalecenter * (1.0 - scale),
        corners[3] * scale + scalecenter * (1.0 - scale),
    ]
}

/// Solves for the original quad corner position `corners[corner_idx]` such that
/// scaling the quad toward `scale_center(corners)` by `scale` puts the scaled
/// corner at `target`.
pub fn solve_corner_for_scaled_target(
    mut corners: [Vec2; 4],
    corner_idx: usize,
    scale: f32,
    target: Vec2,
) -> Vec2 {
    let scale = scale.clamp(0.01, 10.0);
    if (scale - 1.0).abs() < 1e-6 {
        return target;
    }

    let base_gamma = 1.0 / (scale + (1.0 - scale) * 0.25);
    let mut c = corners[corner_idx];
    let mut best_c = c;
    let mut best_err = f32::MAX;

    for _ in 0..40 {
        corners[corner_idx] = c;
        let current = compute_scaled_corners(corners, scale)[corner_idx];
        let res = target - current;
        let err = res.length();
        if err < best_err {
            best_err = err;
            best_c = c;
        }
        if err < 1e-4 {
            return c;
        }

        let mut step_gamma = base_gamma;
        let mut next_c = c + res * step_gamma;
        corners[corner_idx] = next_c;
        let mut next_err = (target - compute_scaled_corners(corners, scale)[corner_idx]).length();

        while next_err > err && step_gamma > base_gamma * 0.05 {
            step_gamma *= 0.5;
            next_c = c + res * step_gamma;
            corners[corner_idx] = next_c;
            next_err = (target - compute_scaled_corners(corners, scale)[corner_idx]).length();
        }

        c = next_c;
    }

    best_c
}

/// System for mouse hover hit testing and corner dragging.
fn handle_corner_drag(
    mut drag_state: ResMut<CornerDragState>,
    mut deformable_query: Query<(Entity, &mut DeformableImage, &GlobalTransform)>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    windows: Query<&Window>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut egui_contexts: EguiContexts,
    tristate: Option<Res<TriangulationState>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok((camera, camera_gt)) = camera_query.single() else {
        return;
    };

    // Check if egui is currently taking mouse interaction
    let egui_wants_pointer = if let Ok(ctx) = egui_contexts.ctx_mut() {
        ctx.is_pointer_over_egui()
    } else {
        false
    };

    let Ok(world_pos) = camera.viewport_to_world_2d(camera_gt, cursor_pos) else {
        return;
    };

    let scale = tristate.as_ref().map_or(1.0, |t| t.deformable_scale);

    // Handle active drag in progress
    if let (Some(entity), Some((corner_idx, handle_kind))) =
        (drag_state.active_entity, drag_state.dragged_corner)
    {
        if mouse_button.pressed(MouseButton::Left) {
            if let Ok((_, mut deformable, entity_gt)) = deformable_query.get_mut(entity) {
                // Convert world mouse position into entity local space
                let inv_affine = entity_gt.affine().inverse();
                let local_pos = inv_affine.transform_point3(world_pos.extend(0.0)).xy();

                match handle_kind {
                    HandleKind::Original => {
                        deformable.corners[corner_idx] = local_pos;
                    }
                    HandleKind::Scaled => {
                        deformable.corners[corner_idx] = solve_corner_for_scaled_target(
                            deformable.corners,
                            corner_idx,
                            scale,
                            local_pos,
                        );
                    }
                }
                deformable.is_dirty = true;
            }
            return;
        } else {
            // Mouse released
            drag_state.active_entity = None;
            drag_state.dragged_corner = None;
        }
    }

    if egui_wants_pointer {
        drag_state.hovered_corner = None;
        return;
    }

    // Hit test corners to find hovered handle
    let mut closest_hover: Option<(Entity, usize, HandleKind, f32)> = None;

    for (entity, deformable, entity_gt) in deformable_query.iter() {
        if !deformable.enabled {
            continue;
        }

        let mode = deformable.drag_mode.unwrap_or(drag_state.drag_mode);

        let corners_world_orig: [Vec2; 4] = [
            entity_gt.transform_point(deformable.corners[0].extend(0.0)).xy(),
            entity_gt.transform_point(deformable.corners[1].extend(0.0)).xy(),
            entity_gt.transform_point(deformable.corners[2].extend(0.0)).xy(),
            entity_gt.transform_point(deformable.corners[3].extend(0.0)).xy(),
        ];

        let corners_world_scaled = compute_scaled_corners(corners_world_orig, scale);

        // Check scaled corners if mode is Scaled or Both
        if matches!(mode, CornerDragMode::Scaled | CornerDragMode::Both) {
            for (idx, &corner_world) in corners_world_scaled.iter().enumerate() {
                let dist = corner_world.distance(world_pos);
                if dist <= deformable.handle_radius {
                    if closest_hover.map_or(true, |(_, _, _, min_d)| dist < min_d) {
                        closest_hover = Some((entity, idx, HandleKind::Scaled, dist));
                    }
                }
            }
        }

        // Check original corners if mode is Original or Both
        if matches!(mode, CornerDragMode::Original | CornerDragMode::Both) {
            for (idx, &corner_world) in corners_world_orig.iter().enumerate() {
                let dist = corner_world.distance(world_pos);
                if dist <= deformable.handle_radius {
                    if closest_hover.map_or(true, |(_, _, _, min_d)| dist < min_d) {
                        closest_hover = Some((entity, idx, HandleKind::Original, dist));
                    }
                }
            }
        }
    }

    drag_state.hovered_corner = closest_hover.map(|(e, i, k, _)| (e, i, k));

    // Handle mouse click to start drag
    if mouse_button.just_pressed(MouseButton::Left) {
        if let Some((entity, corner_idx, handle_kind)) = drag_state.hovered_corner {
            drag_state.active_entity = Some(entity);
            drag_state.dragged_corner = Some((corner_idx, handle_kind));
        }
    }
}

/// Visualizes corner handles and bounding quad using Bevy Gizmos.
fn draw_corner_gizmos(
    drag_state: Res<CornerDragState>,
    tristate: Option<Res<TriangulationState>>,
    deformable_query: Query<(Entity, &DeformableImage, &GlobalTransform)>,
    mut gizmos: Gizmos<DefaultGizmos>,
) {
    let scale = tristate.as_ref().map_or(1.0, |t| t.deformable_scale);

    for (entity, deformable, entity_gt) in deformable_query.iter() {
        if !deformable.enabled {
            continue;
        }

        let mode = deformable.drag_mode.unwrap_or(drag_state.drag_mode);

        let corners_world_orig: [Vec2; 4] = [
            entity_gt
                .transform_point(deformable.corners[0].extend(0.0))
                .xy(),
            entity_gt
                .transform_point(deformable.corners[1].extend(0.0))
                .xy(),
            entity_gt
                .transform_point(deformable.corners[2].extend(0.0))
                .xy(),
            entity_gt
                .transform_point(deformable.corners[3].extend(0.0))
                .xy(),
        ];

        let corners_world_scaled = compute_scaled_corners(corners_world_orig, scale);
        let has_scale_offset = (scale - 1.0).abs() >= 1e-4;

        let frame_color_outer = Color::srgba(0.2, 0.8, 1.0, 0.35);
        let frame_color_inner = Color::srgba(0.2, 0.8, 1.0, 0.6);

        // 1. Draw outer quad lines
        draw_quad_lines(&mut gizmos, corners_world_orig, frame_color_outer);

        // 2. Draw inner quad lines if scaled
        if has_scale_offset {
            draw_quad_lines(&mut gizmos, corners_world_scaled, frame_color_inner);
        }

        // 3. Draw outer handles
        let orig_interactive = matches!(mode, CornerDragMode::Original | CornerDragMode::Both);
        draw_quad_handles(
            &mut gizmos,
            entity,
            &drag_state,
            corners_world_orig,
            HandleKind::Original,
            orig_interactive,
        );

        // 4. Draw inner handles if scaled
        if has_scale_offset {
            let scaled_interactive = matches!(mode, CornerDragMode::Scaled | CornerDragMode::Both);
            draw_quad_handles(
                &mut gizmos,
                entity,
                &drag_state,
                corners_world_scaled,
                HandleKind::Scaled,
                scaled_interactive,
            );
        }
    }
}

fn draw_quad_lines(gizmos: &mut Gizmos<DefaultGizmos>, corners: [Vec2; 4], color: Color) {
    gizmos.line_2d(corners[0], corners[1], color);
    gizmos.line_2d(corners[1], corners[2], color);
    gizmos.line_2d(corners[2], corners[3], color);
    gizmos.line_2d(corners[3], corners[0], color);
}

fn draw_quad_handles(
    gizmos: &mut Gizmos<DefaultGizmos>,
    entity: Entity,
    drag_state: &CornerDragState,
    corners: [Vec2; 4],
    kind: HandleKind,
    interactive: bool,
) {
    for (idx, &corner_world) in corners.iter().enumerate() {
        let is_dragged = drag_state.active_entity == Some(entity)
            && drag_state.dragged_corner == Some((idx, kind));
        let is_hovered = drag_state.hovered_corner == Some((entity, idx, kind));

        let (color, radius) = if is_dragged {
            (Color::srgb(0.0, 1.0, 0.4), 10.0)
        } else if is_hovered {
            (Color::srgb(1.0, 0.9, 0.2), 9.0)
        } else if interactive {
            (Color::srgb(0.2, 0.8, 1.0), 7.0)
        } else {
            (Color::srgba(0.2, 0.8, 1.0, 0.4), 4.5)
        };

        gizmos.circle_2d(corner_world, radius, color);
    }
}

/// Finds a point `p` such that scaling the quadrilateral toward `p` by any
/// factor k in (0, 1) produces a polygon strictly nested inside the original.
///
/// Works for any simple (non-self-intersecting) quadrilateral, convex or
/// concave. The vertex mean only works for the convex case — for a concave
/// "dart" quad it can land in the notch cut out by the reflex vertex,
/// i.e. outside the polygon, which breaks nesting.
pub fn scale_center(poly: [Vec2; 4]) -> Vec2 {
    let mut pts = poly.to_vec();
    if signed_area(&pts) < 0.0 {
        pts.reverse(); // normalize to CCW
    }

    let mut kernel = pts.clone();
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        kernel = clip_half_plane(&kernel, a, b);
        if kernel.is_empty() {
            break;
        }
    }

    polygon_centroid(&kernel).unwrap_or_else(|| {
        // Degenerate fallback: near-zero-area kernel (e.g. three
        // vertices almost collinear). Just average whatever survived.
        let fallback = if kernel.is_empty() { &pts } else { &kernel };
        fallback.iter().fold(Vec2::ZERO, |a, &b| a + b) / fallback.len() as f32
    })
}

/// Sutherland–Hodgman clip: keeps the part of `poly` on the interior
/// (left) side of the directed line through `a -> b`.
fn clip_half_plane(poly: &[Vec2], a: Vec2, b: Vec2) -> Vec<Vec2> {
    let edge = b - a;
    let side = |p: Vec2| edge.perp_dot(p - a);

    let mut out = Vec::with_capacity(poly.len() + 1);
    let n = poly.len();
    for i in 0..n {
        let cur = poly[i];
        let next = poly[(i + 1) % n];
        let s_cur = side(cur);
        let s_next = side(next);
        let cur_in = s_cur >= -EPS;
        let next_in = s_next >= -EPS;

        if cur_in {
            out.push(cur);
        }
        if cur_in != next_in {
            let denom = s_cur - s_next;
            if denom.abs() > EPS {
                let t = (s_cur / denom).clamp(0.0, 1.0);
                let p = cur + (next - cur) * t;
                // avoid duplicate points when the crossing lands on a
                // vertex that's already on the clip line
                if out
                    .last()
                    .map_or(true, |&last| last.distance_squared(p) > EPS * EPS)
                {
                    out.push(p);
                }
            }
        }
    }
    out
}
const EPS: f32 = 1e-6;
/// Area-weighted centroid of a convex polygon. `None` if near-zero area.
fn polygon_centroid(poly: &[Vec2]) -> Option<Vec2> {
    if poly.len() < 3 {
        return None;
    }
    let mut area = 0.0;
    let mut c = Vec2::ZERO;
    for i in 0..poly.len() {
        let p0 = poly[i];
        let p1 = poly[(i + 1) % poly.len()];
        let cross = p0.x * p1.y - p1.x * p0.y;
        area += cross;
        c += (p0 + p1) * cross;
    }
    area *= 0.5;
    (area.abs() >= EPS).then(|| c / (6.0 * area))
}

fn signed_area(pts: &[Vec2]) -> f32 {
    let mut sum = 0.0;
    let n = pts.len();
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        sum += a.x * b.y - b.x * a.y;
    }
    sum * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solver_comprehensive() {
        let original_corners = [
            Vec2::new(-100.0, 50.0),
            Vec2::new(100.0, 50.0),
            Vec2::new(100.0, -50.0),
            Vec2::new(-100.0, -50.0),
        ];

        for &scale in &[0.1, 0.25, 0.5, 0.75, 0.9, 1.0] {
            for corner_idx in 0..4 {
                let mut current_corners = original_corners;
                for step in 1..=5 {
                    let scaled_now = compute_scaled_corners(current_corners, scale)[corner_idx];
                    let mouse_target = scaled_now + Vec2::new(step as f32 * 2.0, -(step as f32) * 1.5);
                    let new_c = solve_corner_for_scaled_target(current_corners, corner_idx, scale, mouse_target);
                    let actual_scaled = compute_scaled_corners({
                        let mut c = current_corners;
                        c[corner_idx] = new_c;
                        c
                    }, scale)[corner_idx];
                    current_corners[corner_idx] = new_c;
                    assert!(
                        (actual_scaled - mouse_target).length() < 1e-2,
                        "scale={scale}, corner={corner_idx}, step={step}: err={}",
                        (actual_scaled - mouse_target).length()
                    );
                }
            }
        }
    }

    #[test]
    fn test_compute_scaled_corners() {
        let original_corners = [
            Vec2::new(-100.0, 50.0),
            Vec2::new(100.0, 50.0),
            Vec2::new(100.0, -50.0),
            Vec2::new(-100.0, -50.0),
        ];

        // Scale 1.0 should return identical corners
        let at_1 = compute_scaled_corners(original_corners, 1.0);
        for i in 0..4 {
            assert!((at_1[i] - original_corners[i]).length() < 1e-4);
        }

        // Scale 0.5 should halve the dimensions around center (0, 0)
        let at_half = compute_scaled_corners(original_corners, 0.5);
        assert!((at_half[0] - Vec2::new(-50.0, 25.0)).length() < 1e-4);
        assert!((at_half[1] - Vec2::new(50.0, 25.0)).length() < 1e-4);
        assert!((at_half[2] - Vec2::new(50.0, -25.0)).length() < 1e-4);
        assert!((at_half[3] - Vec2::new(-50.0, -25.0)).length() < 1e-4);
    }

    #[test]
    fn test_modes() {
        assert_eq!(CornerDragMode::default(), CornerDragMode::Scaled);
        let drag_state = CornerDragState::default();
        assert_eq!(drag_state.drag_mode, CornerDragMode::Scaled);
    }
}
