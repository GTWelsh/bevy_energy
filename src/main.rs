use std::f32::consts::TAU;

use bevy::pbr::NotShadowCaster;
use bevy::{
    core_pipeline::{bloom::Bloom, tonemapping::Tonemapping},
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, (setup_floor, setup_player, add_cubes))
        .add_systems(
            Update,
            (
                (rotate_player, move_player).chain(),
                move_camera,
                change_camera,
                player_shoot,
            ),
        )
        .insert_resource(FloorSize(1000.0))
        .insert_resource(CameraView(CameraViewType::TopDown))
        .run();
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum CameraViewType {
    TopDown,
    ThirdPerson,
    FirstPerson,
}

#[derive(Resource)]
struct FloorSize(f32);

#[derive(Resource)]
struct CameraView(CameraViewType);

#[derive(Component)]
struct Player;

#[derive(Component)]
struct PlayerCamera;

#[derive(Component)]
struct PlayerWeapon;

#[derive(Component)]
struct Cube;

#[derive(Component)]
struct AimPoint(Vec3);

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

fn player_shoot(
    mut commands: Commands,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    spawn_transform: Single<&GlobalTransform, With<PlayerWeapon>>,
) {
    if !mouse_input.just_pressed(MouseButton::Left) {
        return;
    }
    let Vec3 { x, y, z } = spawn_transform.translation();
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.05))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_xyz(x, y, z),
    ));
}

fn rotate_player(
    mouse_motion: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
    mut transform: Single<&mut Transform, With<Player>>,
) {
    let rotation_speed: f32 = 0.5;
    let rotation_amount = -mouse_motion.delta.x * rotation_speed;

    transform.rotate_y(rotation_amount * time.delta_secs());
}

fn move_player(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut player: Single<&mut Transform, With<Player>>,
    time: Res<Time>,
) {
    let speed = 5.0 * time.delta_secs();
    let mut movement = Vec3::ZERO;

    if keyboard_input.pressed(KeyCode::KeyW) {
        movement.z -= speed;
    }

    if keyboard_input.pressed(KeyCode::KeyA) {
        movement.x -= speed;
    }

    if keyboard_input.pressed(KeyCode::KeyD) {
        movement.x += speed;
    }

    if keyboard_input.pressed(KeyCode::KeyS) {
        movement.z += speed;
    }

    let new_movement = player.rotation.mul_vec3(movement);

    player.translation += new_movement;
}

fn change_camera(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut camera: Single<&mut Transform, With<PlayerCamera>>,
    aimpoint: Single<&AimPoint, With<Player>>,
    mut camera_view: ResMut<CameraView>,
) {
    if !keyboard_input.just_pressed(KeyCode::KeyV) {
        return;
    }

    let view_modes = [
        CameraViewType::TopDown,
        CameraViewType::ThirdPerson,
        CameraViewType::FirstPerson,
    ];
    let view_index = view_modes.iter().position(|v| *v == camera_view.0);

    let mut next_view_index = if let Some(i) = view_index { i + 1 } else { 0 };

    if next_view_index >= view_modes.len() {
        next_view_index = 0;
    }

    camera_view.0 = view_modes[next_view_index];

    if camera_view.0 == CameraViewType::TopDown {
        camera.translation.x = 0.0;
        camera.translation.y = 30.0;
        camera.translation.z = 0.0;
        camera.look_at(Vec3::NEG_Z, Vec3::Y);
    }

    if camera_view.0 == CameraViewType::ThirdPerson {
        camera.translation.x = 0.0;
        camera.translation.y = 1.0;
        camera.translation.z = 6.0;
        camera.look_at(Vec3::NEG_Z, Vec3::Y);
    }

    if camera_view.0 == CameraViewType::FirstPerson {
        camera.translation.x = 0.0;
        camera.translation.y = 0.85;
        camera.translation.z = -0.2;
        camera.look_at(aimpoint.0, Vec3::Y);
    }
}

fn move_camera(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut camera: Single<&mut Transform, With<PlayerCamera>>,
    time: Res<Time>,
) {
    let speed = 5.0 * time.delta_secs();
    if keyboard_input.pressed(KeyCode::KeyE) {
        camera.translation.y -= speed;
        camera.look_at(Vec3::NEG_Z, Vec3::Y);
    }

    if keyboard_input.pressed(KeyCode::KeyQ) {
        camera.translation.y += speed;
        camera.look_at(Vec3::NEG_Z, Vec3::Y);
    }
}

fn setup_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let height = 2.0;
    let radius = 0.5;
    let aimpoint = Vec3::new(0.0, 0.85, -10.0);

    commands
        .spawn((
            Mesh3d(meshes.add(Capsule3d::new(radius, height - radius * 2.0))),
            MeshMaterial3d(materials.add(Color::WHITE)),
            Transform::from_xyz(-1.5, height / 2.0, -1.0),
            Player,
            AimPoint(aimpoint),
        ))
        .with_children(|parent| {
            parent.spawn((
                Camera3d::default(),
                Camera {
                    hdr: true, // 1. HDR is required for bloom
                    ..default()
                },
                Tonemapping::TonyMcMapface, // 2. Using a tonemapper that desaturates to white is recommended
                Transform::from_xyz(0.0, 50.0, 0.0).looking_at(Vec3::NEG_Z, Vec3::Y),
                Bloom::NATURAL,
                PlayerCamera,
            ));

            parent.spawn((
                PointLight {
                    shadows_enabled: true,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.5, 0.0),
            ));

            let weapon_length = 1.0;
            let weapon_radius = 0.05;
            let actual_weapon_length = weapon_length - weapon_radius * 2.0;

            parent.spawn((
                Mesh3d(meshes.add(Cuboid::new(
                    weapon_radius,
                    weapon_radius,
                    actual_weapon_length,
                ))),
                MeshMaterial3d(materials.add(Color::WHITE)),
                Transform::from_xyz(0.2, 0.7, -0.5).looking_at(aimpoint, Vec3::Y),
                NotShadowCaster,
                PlayerWeapon,
            ));
        });
}

fn add_cubes(
    mut commands: Commands,
    floor_size: Res<FloorSize>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let size_mutli = 15.0;
    let density = 500.0 / size_mutli;
    let max_number_of_cubes = (floor_size.0.floor() * density) as i32;
    let actual_number_of_cubes = rand::random_range(1..max_number_of_cubes);
    let cube_type_range = 0..10;
    let mut cube_meshes: Vec<(f32, Handle<Mesh>)> = Vec::with_capacity(5);
    let upper_size = 0.35 * size_mutli;

    for _i in cube_type_range {
        let cube_size_x = rand::random_range(0.05..upper_size);
        let cube_size_y = rand::random_range(0.05..upper_size);
        cube_meshes.push((
            cube_size_y,
            meshes.add(Cuboid::new(cube_size_x, cube_size_y, cube_size_x)),
        ));
    }

    let cube_mat = materials.add(Color::srgb_u8(124, 144, 255));

    let range_of_all_cubes = 1..actual_number_of_cubes;

    for _i in range_of_all_cubes {
        let (cube_size, cube_mesh) = &cube_meshes[rand::random_range(0..10)];

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
