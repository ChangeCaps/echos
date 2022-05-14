use std::f32::consts::FRAC_PI_2;

use bevy::{input::mouse::MouseMotion, prelude::*};

#[derive(Component, Clone, Debug)]
pub struct PlaneCamera {
    pub distance: f32,
    pub angles: Vec2,
}

impl Default for PlaneCamera {
    fn default() -> Self {
        Self {
            distance: 15.0,
            angles: Vec2::new(0.0, 0.4),
        }
    }
}

impl PlaneCamera {
    pub fn system(
        mut mouse_motion: EventReader<MouseMotion>,
        windows: Res<Windows>,
        mut query: Query<(&mut PlaneCamera, &mut Transform)>,
    ) {
        let window = windows.primary();

        let mut delta = Vec2::ZERO;

        if window.cursor_locked() {
            for event in mouse_motion.iter() {
                delta += event.delta;
            }

            delta /= 1000.0;
        }

        if let Ok((mut camera, mut transform)) = query.get_single_mut() {
            camera.angles += delta;

            let y = camera.angles.y.clamp(-FRAC_PI_2, FRAC_PI_2);
            camera.angles.y = y;

            transform.translation.x = camera.angles.x.sin() * camera.angles.y.cos();
            transform.translation.y = camera.angles.y.sin();
            transform.translation.z = -camera.angles.x.cos() * camera.angles.y.cos();

            transform.translation *= camera.distance;

            transform.look_at(Vec3::ZERO, Vec3::Y);
        }
    }
}
