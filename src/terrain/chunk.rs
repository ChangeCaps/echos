use std::collections::HashMap;

use bevy::{
    math::Vec3Swizzles, prelude::*, reflect::TypeUuid, render::mesh::Indices,
    tasks::AsyncComputeTaskPool, utils::HashSet,
};
use crossbeam::channel::{unbounded, Receiver, Sender};
use heron::prelude::*;
use noise::{NoiseFn, Perlin};
use serde::{Deserialize, Serialize};

fn noise(p: Vec2) -> f32 {
    let noise = Perlin::new();

    noise.get([p.x as f64, p.y as f64]) as f32
}

fn nnoise(p: Vec2) -> f32 {
    noise(p) * 0.5 + 0.5
}

fn height_function(p: Vec2) -> f32 {
    let mut h = 1.0 - noise(p / 200.0).abs().powf(1.1);

    h *= nnoise(p / 400.0);

    h * 30.0
}

#[derive(Component)]
pub struct TerrainCenter;

#[derive(Debug)]
pub struct TerrainChunks {
    pub chunk_size: i32,
    pub max_range: f32,
    pub detail: Vec<usize>,
    pub sender: Sender<ChunkUpdate>,
    pub updates: Receiver<ChunkUpdate>,
    pub queue: HashSet<IVec2>,
    pub chunks: HashMap<IVec2, TerrainChunk>,
}

impl Default for TerrainChunks {
    fn default() -> Self {
        let (sender, receiver) = unbounded();

        Self {
            chunk_size: 50,
            max_range: 2000.0,
            detail: vec![75, 50, 25, 15, 10, 5, 3],
            sender,
            updates: receiver,
            queue: HashSet::new(),
            chunks: HashMap::new(),
        }
    }
}

impl TerrainChunks {
    pub const MATERIAL: HandleUntyped =
        HandleUntyped::weak_from_u64(StandardMaterial::TYPE_UUID, 12738921);

    pub fn unload_chunks(&mut self, center: Vec2, commands: &mut Commands) {
        let chunk_size = self.chunk_size as f32;
        let p = Vec2::floor(center / chunk_size) * chunk_size;
        let center_chunk = Vec2::as_ivec2(&p);

        self.chunks.retain(|p_i, chunk| {
            let p = p_i.as_vec2(); // position
            let d = center_chunk.as_vec2().distance(p); // distance

            if d > self.max_range {
                commands.entity(chunk.entity).despawn_recursive();

                false
            } else {
                true
            }
        });
    }

    pub fn load_chunks(&mut self, center: Vec2, task_pool: &AsyncComputeTaskPool) {
        let first_load = self.chunks.is_empty();

        let chunk_size = self.chunk_size as f32;
        let p = Vec2::floor(center / chunk_size) * chunk_size;
        let center_chunk = Vec2::as_ivec2(&p);

        let chunk_range = f32::ceil(self.max_range / chunk_size) as i32;

        for x in -chunk_range..chunk_range {
            for z in -chunk_range..chunk_range {
                let p_i = center_chunk + IVec2::new(x * self.chunk_size, z * self.chunk_size);

                let p = p_i.as_vec2();
                let d = center_chunk.as_vec2().distance(p);

                let lod = (d / self.max_range * (self.detail.len() - 1) as f32).floor() as usize;

                if d > self.max_range || self.queue.contains(&p_i) {
                    continue;
                }

                if let Some(chunk) = self.chunks.get(&p_i) {
                    if chunk.lod == lod {
                        continue;
                    }
                }

                let detail = self.detail[lod];

                self.queue.insert(p_i);

                let sender = self.sender.clone();

                let load = move || {
                    let height_map = HeightMap::generate(p, chunk_size, detail, height_function);
                    let mesh = height_map.generate_mesh();

                    sender
                        .send(ChunkUpdate {
                            position: p_i,
                            height_map,
                            mesh,
                            lod,
                        })
                        .unwrap();
                };

                if first_load {
                    load();
                } else {
                    task_pool.spawn(async move { load() }).detach();
                }
            }
        }
    }

    pub fn update_chunks(
        &mut self,
        center: Vec2,
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        task_pool: &AsyncComputeTaskPool,
        query: &mut Query<(&mut Handle<Mesh>, &mut CollisionShape)>,
    ) {
        self.unload_chunks(center, commands);
        self.load_chunks(center, task_pool);

        for update in self.updates.try_iter() {
            let chunk_size = self.chunk_size as f32;
            let mesh = meshes.add(update.mesh);
            self.queue.remove(&update.position);

            if let Some(chunk) = self.chunks.get_mut(&update.position) {
                chunk.lod = update.lod;

                chunk.mesh = mesh.clone();

                let (mut mesh_handle, mut collision_shape) = query.get_mut(chunk.entity).unwrap();

                *mesh_handle = mesh;
                match *collision_shape {
                    CollisionShape::HeightField {
                        ref mut heights, ..
                    } => *heights = update.height_map.heights,
                    _ => unreachable!(),
                }
            } else {
                let entity = commands
                    .spawn_bundle(MaterialMeshBundle::<StandardMaterial> {
                        mesh: mesh.clone(),
                        material: Self::MATERIAL.typed(),
                        transform: Transform::from_xyz(
                            update.position.x as f32,
                            0.0,
                            update.position.y as f32,
                        ),
                        ..Default::default()
                    })
                    .insert(RigidBody::Static)
                    .insert(CollisionShape::HeightField {
                        size: Vec2::splat(chunk_size),
                        heights: update.height_map.heights,
                    })
                    .insert(PhysicMaterial {
                        restitution: 0.0,
                        ..Default::default()
                    })
                    .id();

                let chunk = TerrainChunk {
                    lod: update.lod,
                    mesh,
                    entity,
                };

                self.chunks.insert(update.position, chunk);
            }
        }
    }

    pub fn system(
        mut commands: Commands,
        mut chunks: ResMut<TerrainChunks>,
        mut meshes: ResMut<Assets<Mesh>>,
        task_pool: Res<AsyncComputeTaskPool>,
        mut mesh_query: Query<(&mut Handle<Mesh>, &mut CollisionShape)>,
        query: Query<&GlobalTransform, With<TerrainCenter>>,
    ) {
        if let Ok(transform) = query.get_single() {
            let center = transform.translation.xz();

            chunks.update_chunks(
                center,
                &mut commands,
                &mut meshes,
                &task_pool,
                &mut mesh_query,
            );
        } else {
            debug!("player doesn't exist");
        }
    }
}

pub struct ChunkUpdate {
    pub position: IVec2,
    pub height_map: HeightMap,
    pub mesh: Mesh,
    pub lod: usize,
}

#[derive(Debug)]
pub struct TerrainChunk {
    pub lod: usize,
    pub mesh: Handle<Mesh>,
    pub entity: Entity,
}

#[derive(Debug, Clone, Serialize, Deserialize, TypeUuid)]
#[uuid = "2f3080db-425d-41c6-bd2a-d1bafb349752"]
pub struct HeightMap {
    pub size: f32,
    pub row_size: usize,
    pub heights: Vec<Vec<f32>>,
    pub normals: Vec<Vec<[f32; 3]>>,
}

impl HeightMap {
    pub fn generate(
        offset: Vec2,
        size: f32,
        row_size: usize,
        mut f: impl FnMut(Vec2) -> f32,
    ) -> Self {
        const EPSILON: f32 = 0.05;

        let mut heights = Vec::with_capacity(row_size);
        let mut normals = Vec::with_capacity(row_size);
        // pre-compute factor
        let factor = 1.0 / (row_size - 1) as f32 * size as f32;
        let half_size = size / 2.0;

        for x_i in 0..row_size {
            let x = x_i as f32 * factor - half_size;

            let mut row = Vec::with_capacity(row_size);
            let mut row_normals = Vec::with_capacity(row_size);

            for z_i in 0..row_size {
                let z = z_i as f32 * factor - half_size;

                let p = offset + Vec2::new(x, z);
                let height = f(p);

                let normal = Vec3::new(
                    height - f(p + Vec2::new(EPSILON, 0.0)),
                    EPSILON,
                    height - f(p + Vec2::new(0.0, EPSILON)),
                )
                .normalize_or_zero();

                row.push(height);
                row_normals.push(normal.into());
            }

            heights.push(row);
            normals.push(row_normals);
        }

        Self {
            size,
            row_size,
            heights,
            normals,
        }
    }

    pub fn generate_mesh(&self) -> Mesh {
        let mut positions = Vec::<[f32; 3]>::with_capacity(self.row_size * self.row_size);
        let mut normals = Vec::<[f32; 3]>::with_capacity(self.row_size * self.row_size);
        let mut uvs = Vec::<[f32; 2]>::with_capacity(self.row_size * self.row_size);
        let mut indices = Vec::<u32>::new();

        let factor = 1.0 / (self.row_size - 1) as f32 * self.size as f32;
        let half_size = self.size / 2.0;

        for x_i in 0..self.row_size {
            let x = x_i as f32 * factor - half_size;

            for z_i in 0..self.row_size {
                let z = z_i as f32 * factor - half_size;

                let i = z_i * self.row_size + x_i;

                positions.push([x, self.heights[x_i][z_i], z]);
                normals.push(self.normals[x_i][z_i]);
                uvs.push([x, z]);

                if x_i > 0 && z_i > 0 {
                    let j = i as u32;
                    let i = j - self.row_size as u32;

                    indices.push(i - 1);
                    indices.push(j);
                    indices.push(j - 1);

                    indices.push(i - 1);
                    indices.push(i);
                    indices.push(j);
                }
            }
        }

        let mut mesh = Mesh::new(bevy::render::mesh::PrimitiveTopology::TriangleList);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.set_indices(Some(Indices::U32(indices)));

        mesh
    }
}
