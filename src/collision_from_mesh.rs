use std::collections::LinkedList;

use bevy::{prelude::*, render::mesh::VertexAttributeValues};
use heron::CollisionShape;

#[derive(Component, Clone, Debug, Default)]
pub struct PendingColliders;

pub fn pending_colliders_system(
    mut commands: Commands,
    added_scenes: Query<(Entity, &Children), With<PendingColliders>>,
    scene_elements: Query<&Children, Without<PendingColliders>>,
    transforms: Query<&Transform>,
    mesh_handles: Query<&Handle<Mesh>>,
    meshes: Option<Res<Assets<Mesh>>>,
) {
    let meshes = match meshes {
        Some(m) => m,
        None => return,
    };

    for (scene, children) in added_scenes.iter() {
        let children = recursive_scene_children(
            children,
            Transform::identity(),
            &scene_elements,
            &transforms,
        );

        let mut scene_commands = commands.entity(scene);

        for (child, transform) in children.iter().cloned() {
            if let Ok(handle) = mesh_handles.get(child) {
                let mesh = meshes.get(handle).unwrap();

                let aabb = mesh.compute_aabb().unwrap();

                if (aabb.max() - aabb.min()).max_element() < 0.25 {
                    continue;
                }

                let vertices = match mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap() {
                    VertexAttributeValues::Float32x3(vertices) => vertices,
                    _ => unreachable!(),
                };

                let mut points = Vec::with_capacity(vertices.len());
                for vertex in vertices {
                    points.push(Vec3::from(*vertex));
                }

                scene_commands.with_children(|parent| {
                    parent
                        .spawn()
                        .insert(transform)
                        .insert(GlobalTransform::identity())
                        .insert(CollisionShape::ConvexHull {
                            points,
                            border_radius: None,
                        });
                });
            }
        }

        if !children.is_empty() {
            scene_commands.remove::<PendingColliders>();
        }
    }
}

fn recursive_scene_children(
    children: &Children,
    transform: Transform,
    scene_elements: &Query<&Children, Without<PendingColliders>>,
    transforms: &Query<&Transform>,
) -> LinkedList<(Entity, Transform)> {
    let mut all_children = LinkedList::new();

    for child in children.iter() {
        let child_transform = transform * *transforms.get(*child).unwrap();

        if let Ok(children) = scene_elements.get(*child) {
            let mut children =
                recursive_scene_children(children, child_transform, scene_elements, transforms);
            all_children.append(&mut children);
        }

        all_children.push_back((*child, child_transform));
    }

    all_children
}
