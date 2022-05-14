mod collision_from_mesh;
mod plane;
mod player;
mod sun;
mod terrain;
mod window;

use bevy::prelude::*;
use bevy_prototype_debug_lines::*;
use heron::prelude::*;
use plane::{Plane, PlaneAssetLoader, PlaneCamera, PlaneDescriptor};
use player::Player;
use sun::SunLight;
use terrain::{HeightMap, TerrainChunks};

fn main() {
    App::new()
        // plugins
        .add_plugins(DefaultPlugins)
        .add_plugin(PhysicsPlugin::default())
        .add_plugin(DebugLinesPlugin::default())
        // assets
        .add_asset::<HeightMap>()
        .add_asset::<PlaneDescriptor>()
        .add_asset_loader(PlaneAssetLoader)
        // resources
        .init_resource::<TerrainChunks>()
        .insert_resource(Gravity::from(Vec3::new(0.0, -9.81, 0.0)))
        // startup systems
        .add_startup_system(setup)
        // systems
        .add_system(TerrainChunks::system)
        .add_system(Player::system)
        .add_system(Plane::enter_system)
        .add_system(Plane::flight_system)
        .add_system(Plane::debug_system)
        .add_system(PlaneCamera::system)
        .add_system(SunLight::system)
        .add_system(window::window_system)
        .add_system(collision_from_mesh::pending_colliders_system)
        // run
        .run();
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    asset_server.watch_for_changes().unwrap();

    materials.set_untracked(
        TerrainChunks::MATERIAL,
        StandardMaterial {
            ..Default::default()
        },
    );

    Player::default().spawn(&mut commands, Transform::from_xyz(0.0, 20.0, 0.0));
    Plane::default().spawn(
        &mut commands,
        &asset_server,
        Transform::from_xyz(0.0, 15.0, -4.0),
    );

    commands
        .spawn_bundle(DirectionalLightBundle {
            transform: Transform::identity().looking_at(Vec3::new(-1.0, -1.0, -1.0), Vec3::Y),
            directional_light: DirectionalLight {
                shadows_enabled: true,
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(SunLight);
}
