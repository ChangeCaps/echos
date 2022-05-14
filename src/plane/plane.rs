use bevy::{prelude::*, reflect::TypeUuid};
use bevy_prototype_debug_lines::DebugLines;
use heron::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    collision_from_mesh::PendingColliders,
    plane::{SurfaceForces, SurfaceInputState},
    player::Player,
    terrain::TerrainCenter,
};

use super::{PlaneCamera, PlaneSurface};

#[derive(Clone, Debug, Default, Serialize, Deserialize, TypeUuid)]
#[uuid = "c5b78858-4882-4dee-b860-87375369de15"]
pub struct PlaneDescriptor {
    pub max_speed: f32,
    pub mass: f32,
    pub center_of_mass: Vec3,
    pub surfaces: Vec<PlaneSurface>,
}

#[derive(Component, Clone, Debug, Default)]
pub struct Plane {
    pub speed: f32,
    pub descriptor: Handle<PlaneDescriptor>,
    pub entered: bool,
}

impl Plane {
    pub fn spawn(
        self,
        commands: &mut Commands,
        asset_server: &AssetServer,
        transform: Transform,
    ) -> Entity {
        let scene = asset_server.load("models/plane.glb#Scene0");
        let descriptor = asset_server.load("planes/basic.plane.ron");

        commands
            .spawn()
            .insert(transform)
            .insert(GlobalTransform::identity())
            .insert(RigidBody::Dynamic)
            .insert(Velocity::default())
            .insert(PendingColliders)
            .insert(Plane {
                descriptor,
                ..Default::default()
            })
            .with_children(|parent| {
                parent.spawn_scene(scene);
            })
            .id()
    }

    pub fn debug_system(
        mut lines: ResMut<DebugLines>,
        descriptors: Res<Assets<PlaneDescriptor>>,
        query: Query<(&GlobalTransform, &Plane)>,
    ) {
        if !cfg!(feature = "debug") {
            return;
        }

        for (transform, plane) in query.iter() {
            let descriptor = if let Some(d) = descriptors.get(&plane.descriptor) {
                d
            } else {
                return;
            };

            for surface in descriptor.surfaces.iter() {
                let position = *transform * surface.position;
                let rotation = transform.rotation * surface.rotation_quat();

                let local_z = rotation * Vec3::Z;
                let local_x = rotation * Vec3::X;

                let t = local_z * surface.chord / 2.0;
                let r = local_x * surface.span / 2.0;

                let f = surface.flap_fraction;

                let tr = t + r;
                let tl = t - r;
                let br = -t + r;
                let bl = -t - r;
                let r = Vec3::lerp(br, tr, f);
                let l = Vec3::lerp(bl, tl, f);

                lines.line_colored(position + tr, position + tl, 0.0, Color::BLUE);
                lines.line_colored(position + tl, position + l, 0.0, Color::BLUE);
                lines.line_colored(position + tr, position + r, 0.0, Color::BLUE);
                lines.line_colored(position + r, position + l, 0.0, Color::BLUE);
                lines.line_colored(position + br, position + bl, 0.0, Color::YELLOW_GREEN);
                lines.line_colored(position + bl, position + l, 0.0, Color::YELLOW_GREEN);
                lines.line_colored(position + br, position + r, 0.0, Color::YELLOW_GREEN);
            }
        }
    }

    pub fn flight_system(
        time: Res<Time>,
        key_input: Res<Input<KeyCode>>,
        descriptors: Res<Assets<PlaneDescriptor>>,
        mut lines: ResMut<DebugLines>,
        mut query: Query<(&mut Plane, &mut Velocity, &GlobalTransform)>,
    ) {
        for (mut plane, mut velocity, transform) in query.iter_mut() {
            let mut input = SurfaceInputState::default();

            let descriptor = if let Some(d) = descriptors.get(&plane.descriptor) {
                d
            } else {
                return;
            };

            if plane.entered {
                if key_input.pressed(KeyCode::LShift) {
                    plane.speed += descriptor.max_speed * 0.5 * time.delta_seconds();
                }

                if key_input.pressed(KeyCode::LControl) {
                    plane.speed -= descriptor.max_speed * 0.5 * time.delta_seconds();
                }

                plane.speed = plane.speed.clamp(0.0, descriptor.max_speed);

                if key_input.pressed(KeyCode::W) {
                    input.pitch += 1.0;
                }

                if key_input.pressed(KeyCode::S) {
                    input.pitch -= 1.0;
                }

                if key_input.pressed(KeyCode::A) {
                    input.yaw += 1.0;
                }

                if key_input.pressed(KeyCode::D) {
                    input.yaw -= 1.0;
                }

                if key_input.pressed(KeyCode::Q) {
                    input.roll += 1.0;
                }

                if key_input.pressed(KeyCode::E) {
                    input.roll -= 1.0;
                }
            }

            let angular_velocity: Vec3 = velocity.angular.into();
            let center_of_mass = *transform * descriptor.center_of_mass;

            let air_density = f32::clamp(1.0 - (center_of_mass.y / 1000.0), 0.0, 1.0);

            let mut forces = SurfaceForces::default();
            for surface in descriptor.surfaces.iter() {
                let position = *transform * surface.position;
                let relative_position = position - center_of_mass;
                let rotation = transform.rotation * surface.rotation_quat();

                let mut wind = transform.local_z() * -50.0;
                wind.y = 0.0;

                let air_density = f32::clamp(1.0 - (position.y / 1000.0), 0.0, 1.0);

                let flap_angle = surface.input_flap_angle(&input);
                let surface_forces = surface.calculate_forces(
                    -velocity.linear - Vec3::cross(angular_velocity, relative_position),
                    //wind,
                    air_density, // air density
                    relative_position,
                    position,
                    rotation,
                    flap_angle.to_radians(),
                    &mut lines,
                );

                forces.linear += surface_forces.linear;
                forces.angular += surface_forces.angular;
            }

            let mut sim_forces = SurfaceForces::default();
            for surface in descriptor.surfaces.iter() {
                let position = *transform * surface.position;
                let relative_position = position - center_of_mass;
                let rotation = transform.rotation * surface.rotation_quat();

                let aoa = 5.0f32.to_radians();

                let mut wind = transform.rotation * Vec3::new(0.0, -aoa.sin(), aoa.cos());
                wind *= -50.0;

                let air_density = f32::clamp(1.0 - (position.y / 1000.0), 0.0, 1.0);

                let flap_angle = surface.input_flap_angle(&input);
                let surface_forces = surface.calculate_forces(
                    wind,
                    air_density,
                    relative_position,
                    position,
                    rotation,
                    flap_angle.to_radians(),
                    &mut lines,
                );

                sim_forces.linear += surface_forces.linear;
                sim_forces.angular += surface_forces.angular;
            }

            let center_of_lift = center_of_mass
                + Vec3::cross(sim_forces.linear, sim_forces.angular)
                    / sim_forces.linear.length_squared();

            if cfg!(feature = "debug") {
                lines.line_colored(
                    center_of_mass - Vec3::Y * 3.0,
                    center_of_mass + Vec3::Y * 3.0,
                    0.0,
                    Color::YELLOW,
                );

                lines.line_colored(
                    center_of_lift - Vec3::Y * 3.0,
                    center_of_lift + Vec3::Y * 3.0,
                    0.0,
                    Color::ALICE_BLUE,
                );
            }

            velocity.linear += forces.linear * time.delta_seconds() / descriptor.mass;
            velocity.angular = From::from(
                angular_velocity + forces.angular * time.delta_seconds() / descriptor.mass,
            );

            velocity.linear +=
                transform.local_z() * plane.speed * air_density * time.delta_seconds();
        }
    }

    pub fn enter_system(
        mut commands: Commands,
        key_input: Res<Input<KeyCode>>,
        plane_camera_query: Query<Entity, With<PlaneCamera>>,
        mut plane_query: Query<(Entity, &mut Plane, &GlobalTransform)>,
        player_query: Query<(Entity, &Player, &GlobalTransform)>,
    ) {
        let (plane_entity, mut plane, plane_transform) =
            if let Ok(components) = plane_query.get_single_mut() {
                components
            } else {
                return;
            };

        if plane.entered {
            if key_input.just_pressed(KeyCode::Return) {
                commands.entity(plane_entity).remove::<TerrainCenter>();

                let mut translation = plane_transform.translation
                    + plane_transform.local_x() * -2.0
                    + plane_transform.local_z() * -2.0;

                translation.y = plane_transform.translation.y + 1.0;

                Player::default().spawn(&mut commands, Transform::from_translation(translation));

                plane.entered = false;

                let entity = plane_camera_query.single();

                commands.entity(entity).despawn_recursive();
            }
        } else {
            let (player_entity, _player, player_transform) = player_query.single();

            let distance = plane_transform
                .translation
                .distance(player_transform.translation);

            if distance < 4.0 && key_input.just_pressed(KeyCode::Return) {
                commands.entity(player_entity).despawn_recursive();

                plane.entered = true;

                commands
                    .entity(plane_entity)
                    .insert(TerrainCenter)
                    .with_children(|parent| {
                        parent
                            .spawn_bundle(PerspectiveCameraBundle::default())
                            .insert(PlaneCamera::default());
                    });
            }
        }
    }
}
