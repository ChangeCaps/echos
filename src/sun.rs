use bevy::prelude::*;

use crate::terrain::TerrainCenter;

#[derive(Component, Clone, Debug, Default)]
pub struct SunLight;

impl SunLight {
    pub fn system(
        mut light_query: Query<(&GlobalTransform, &mut DirectionalLight), With<SunLight>>,
        terrain_center_query: Query<&GlobalTransform, With<TerrainCenter>>,
    ) {
        if let Ok(center_transform) = terrain_center_query.get_single() {
            for (transform, mut light) in light_query.iter_mut() {
                let direction = transform.forward();
                let view = Mat4::look_at_rh(Vec3::ZERO, direction, Vec3::Y);
                let center = view.transform_point3(center_transform.translation);

                light.shadow_projection.left = center.x - 25.0;
                light.shadow_projection.right = center.x + 25.0;
                light.shadow_projection.bottom = center.y - 25.0;
                light.shadow_projection.top = center.y + 25.0;
                light.shadow_projection.near = -center.z - 25.0;
                light.shadow_projection.far = -center.z + 1000.0;
            }
        }
    }
}
