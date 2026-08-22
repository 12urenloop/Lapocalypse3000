use bevy::prelude::*;
use std::f32::consts::PI;

use crate::triangulation::{
    ActiveDistanceProvider, DistanceMeasurement, DistanceProviderKind, TriangulationState,
};

pub struct SimulatedDistanceProviderPlugin;

impl Plugin for SimulatedDistanceProviderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_simulated_provider)
            .add_systems(Update, emit_simulated_distances);
    }
}

fn setup_simulated_provider(mut provider: ResMut<ActiveDistanceProvider>) {
    // Register ourselves in the provider list so the UI shows "Simulated".
    if !provider
        .available
        .contains(&DistanceProviderKind::Simulated)
    {
        provider.available.push(DistanceProviderKind::Simulated);
    }
}

fn emit_simulated_distances(
    provider: Res<ActiveDistanceProvider>,
    triangulation: Res<TriangulationState>,
    mut events: MessageWriter<DistanceMeasurement>,
    time: Res<Time>,
    mut last_update: Local<f32>,
) {
    if provider.kind != DistanceProviderKind::Simulated {
        return;
    }

    let t = time.elapsed_secs();
    if t - *last_update < 0.1 {
        return;
    }
    *last_update = t;

    let period = 10.0;

    // Tag 1 revolves with a period of 10 seconds
    let theta1 = (t % period) / period * 2.0 * PI;

    // Tag 2 revolves exactly on the opposite side of the circle
    let theta2 = theta1 + PI;

    // A circle roughly in the middle of the default 3 anchors
    let center = Vec2::new(11.0, 9.0);
    let radius = 7.0;

    let tag1_pos = center + Vec2::new(theta1.cos() * radius, theta1.sin() * radius);
    let tag2_pos = center + Vec2::new(theta2.cos() * radius, theta2.sin() * radius);

    let tags = [(1, tag1_pos), (2, tag2_pos)];

    // Send distances to 2 tags from 3 anchors
    for (tag_id, tag_pos) in tags.iter() {
        for anchor_id in 1..=3 {
            // Get the current anchor positions directly from TriangulationState
            if let Some(anchor_pos) = triangulation.anchors.get(&anchor_id) {
                let distance = tag_pos.distance(*anchor_pos);

                events.write(DistanceMeasurement {
                    anchor_id,
                    tag_id: *tag_id,
                    distance: Some(distance),
                });
            }
        }
    }
}
