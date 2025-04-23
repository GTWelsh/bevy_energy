use std::f32::consts::TAU;

use bevy::{
    core_pipeline::{bloom::Bloom, tonemapping::Tonemapping},
    prelude::*,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(
            Startup,
            (setup_demo_scene, setup_floor, setup_player, add_cubes),
        )
        .insert_resource(FloorSize(100.0))
        .run();
}

#[derive(Resource)]
struct FloorSize(f32);

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Cube;

fn setup_floor(
    mut commands: Commands,
    floor_size: Res<FloorSize>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(
            meshes.add(
                Plane3d::default()
                    .mesh()
                    .size(floor_size.0, floor_size.0)
                    .subdivisions((floor_size.0 / 5.0).floor() as u32),
            ),
        ),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}

fn setup_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let height = 2.0;
    let radius = 0.5;

    commands.spawn((
        Mesh3d(meshes.add(Capsule3d::new(radius, height - radius * 2.0))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_xyz(-1.5, height / 2.0, -1.0),
        Player,
    ));
}

fn add_cubes(
    mut commands: Commands,
    floor_size: Res<FloorSize>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let max_number_of_cubes = (floor_size.0.floor() * 500.0) as i32;
    let actual_number_of_cubes = rand::random_range(1..max_number_of_cubes);
    let cube_type_range = 0..5;
    let mut cube_meshes: Vec<(f32, Handle<Mesh>)> = Vec::with_capacity(5);

    for _i in cube_type_range {
        let cube_size = rand::random_range(0.05..0.35);
        cube_meshes.push((
            cube_size,
            meshes.add(Cuboid::new(cube_size, cube_size, cube_size)),
        ));
    }

    let cube_mat = materials.add(Color::srgb_u8(124, 144, 255));

    let range_of_all_cubes = 1..actual_number_of_cubes;

    for _i in range_of_all_cubes {
        let (cube_size, cube_mesh) = &cube_meshes[rand::random_range(0..5)];

        let half_floor_size = floor_size.0 / 2.0;

        let x: f32 = rand::random_range(0.0..floor_size.0) - half_floor_size;
        let y: f32 = cube_size / 2.0;
        let z: f32 = rand::random_range(0.0..floor_size.0) - half_floor_size;

        commands.spawn((
            Mesh3d(cube_mesh.clone()),
            MeshMaterial3d(cube_mat.clone()),
            Transform::from_xyz(x, y, z)
                .with_rotation(Quat::from_rotation_y(rand::random_range(0.0..TAU))),
            Cube,
        ));
    }
}

/// set up a simple 3D scene
fn setup_demo_scene(mut commands: Commands) {
    // light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
    // camera
    commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: true, // 1. HDR is required for bloom
            ..default()
        },
        Tonemapping::TonyMcMapface, // 2. Using a tonemapper that desaturates to white is recommended
        Transform::from_xyz(-2.5, 2.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
        Bloom::NATURAL,
    ));
}
