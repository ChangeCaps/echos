use bevy::{input::mouse::MouseMotion, prelude::*};
use heron::prelude::*;

use crate::terrain::TerrainCenter;

#[derive(Component, Clone, Debug)]
pub struct Player {
    pub movement_speed: f32,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            movement_speed: 3.0,
        }
    }
}

#[derive(Component, Clone, Debug, Default)]
pub struct PlayerCamera;

impl Player {
    pub fn spawn(self, commands: &mut Commands, transform: Transform) -> Entity {
        commands
            .spawn()
            .insert(self)
            .insert(transform)
            .insert(GlobalTransform::identity())
            .insert(RigidBody::Dynamic)
            .insert(CollisionShape::Capsule {
                half_segment: 0.5,
                radius: 0.25,
            })
            .insert(Velocity::default())
            .insert(RotationConstraints::lock())
            .insert(PhysicMaterial {
                restitution: 0.0,
                ..Default::default()
            })
            .insert(TerrainCenter)
            .with_children(|parent| {
                parent
                    .spawn_bundle(PerspectiveCameraBundle {
                        transform: Transform::from_xyz(0.0, 0.5, 0.0),
                        ..Default::default()
                    })
                    .insert(PlayerCamera);
            })
            .id()
    }

    pub fn system(
        mut mouse_motion: EventReader<MouseMotion>,
        key_input: Res<Input<KeyCode>>,
        windows: Res<Windows>,
        mut player_query: Query<
            (&Player, &mut Velocity, &mut Transform, &GlobalTransform),
            Without<PlayerCamera>,
        >,
        mut camera_query: Query<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
    ) {
        let window = windows.primary();

        let mut delta = Vec2::ZERO;

        if window.cursor_locked() {
            for event in mouse_motion.iter() {
                delta -= event.delta;
            }

            delta /= 1000.0;
        }

        if let Ok(mut transform) = camera_query.get_single_mut() {
            transform.rotate(Quat::from_rotation_x(delta.y));
        }

        if let Ok((player, mut velocity, mut transform, global_transform)) =
            player_query.get_single_mut()
        {
            let mut movement = Vec3::ZERO;

            transform.rotate(Quat::from_rotation_y(delta.x));

            if key_input.pressed(KeyCode::W) {
                movement -= global_transform.local_z();
            }

            if key_input.pressed(KeyCode::S) {
                movement += global_transform.local_z();
            }

            if key_input.pressed(KeyCode::A) {
                movement -= global_transform.local_x();
            }

            if key_input.pressed(KeyCode::D) {
                movement += global_transform.local_x();
            }

            movement = movement.normalize_or_zero();

            velocity.linear.x = movement.x * player.movement_speed;
            velocity.linear.z = movement.z * player.movement_speed;
        }
    }
}
