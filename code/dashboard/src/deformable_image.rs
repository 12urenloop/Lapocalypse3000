use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy_egui::EguiContexts;

use crate::MainCamera;
use crate::ui::set_gizmo_renderlayer;

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
}

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
#[derive(Resource, Default, Debug)]
pub struct CornerDragState {
    pub active_entity: Option<Entity>,
    pub dragged_corner: Option<usize>,
    pub hovered_corner: Option<(Entity, usize)>,
}

pub struct DeformableImagePlugin;

impl Plugin for DeformableImagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CornerDragState>().add_systems(
            Update,
            (
                handle_corner_drag,
                update_deformable_mesh,
                // draw_corner_gizmos,
            )
                .chain(),
        );
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

/// System for mouse hover hit testing and corner dragging.
fn handle_corner_drag(
    mut drag_state: ResMut<CornerDragState>,
    mut deformable_query: Query<(Entity, &mut DeformableImage, &GlobalTransform)>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    windows: Query<&Window>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut egui_contexts: EguiContexts,
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

    // Handle active drag in progress
    if let (Some(entity), Some(corner_idx)) = (drag_state.active_entity, drag_state.dragged_corner)
    {
        if mouse_button.pressed(MouseButton::Left) {
            if let Ok((_, mut deformable, entity_gt)) = deformable_query.get_mut(entity) {
                // Convert world mouse position into entity local space
                let inv_affine = entity_gt.affine().inverse();
                let local_pos = inv_affine.transform_point3(world_pos.extend(0.0)).xy();

                deformable.corners[corner_idx] = local_pos;
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
    let mut closest_hover: Option<(Entity, usize, f32)> = None;

    for (entity, deformable, entity_gt) in deformable_query.iter() {
        if !deformable.enabled {
            continue;
        }

        for (idx, &corner_local) in deformable.corners.iter().enumerate() {
            let corner_world = entity_gt.transform_point(corner_local.extend(0.0)).xy();
            let dist = corner_world.distance(world_pos);

            if dist <= deformable.handle_radius {
                if closest_hover.map_or(true, |(_, _, min_d)| dist < min_d) {
                    closest_hover = Some((entity, idx, dist));
                }
            }
        }
    }

    drag_state.hovered_corner = closest_hover.map(|(e, i, _)| (e, i));

    // Handle mouse click to start drag
    if mouse_button.just_pressed(MouseButton::Left) {
        if let Some((entity, corner_idx)) = drag_state.hovered_corner {
            drag_state.active_entity = Some(entity);
            drag_state.dragged_corner = Some(corner_idx);
        }
    }
}

/// Visualizes corner handles and bounding quad using Bevy Gizmos.
fn draw_corner_gizmos(
    drag_state: Res<CornerDragState>,
    deformable_query: Query<(Entity, &DeformableImage, &GlobalTransform)>,
    mut params: ParamSet<(ResMut<GizmoConfigStore>, Gizmos)>,
) {
    set_gizmo_renderlayer(1, params.p0());

    let mut gizmos = params.p1();

    for (entity, deformable, entity_gt) in deformable_query.iter() {
        if !deformable.enabled {
            continue;
        }

        let corners_world: [Vec2; 4] = [
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

        let frame_color = Color::srgba(0.2, 0.8, 1.0, 0.6);

        // Draw bounding quadrilateral lines
        gizmos.line_2d(corners_world[0], corners_world[1], frame_color);
        gizmos.line_2d(corners_world[1], corners_world[2], frame_color);
        gizmos.line_2d(corners_world[2], corners_world[3], frame_color);
        gizmos.line_2d(corners_world[3], corners_world[0], frame_color);

        // Draw corner handle circles
        for (idx, &corner_world) in corners_world.iter().enumerate() {
            let is_dragged =
                drag_state.active_entity == Some(entity) && drag_state.dragged_corner == Some(idx);
            let is_hovered = drag_state.hovered_corner == Some((entity, idx));

            let (color, radius) = if is_dragged {
                (Color::srgb(0.0, 1.0, 0.4), 10.0)
            } else if is_hovered {
                (Color::srgb(1.0, 0.9, 0.2), 9.0)
            } else {
                (Color::srgb(0.2, 0.8, 1.0), 7.0)
            };

            gizmos.circle_2d(corner_world, radius, color);
        }
    }
}
