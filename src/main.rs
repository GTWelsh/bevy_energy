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
                (
                    (lean_camera, rotate_player, rotate_camera),
                    calc_new_velocity,
                    move_player,
                )
                    .chain(),
                change_camera_keybind,
                player_shoot,
            ),
        )
        .add_observer(set_camera)
        .insert_resource(FloorSize(1000.0))
        .insert_resource(CameraView(CameraViewType::FirstPerson))
        .run();
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum CameraViewType {
    TopDown,
    ThirdPerson,
    FirstPerson,
}

#[derive(Event)]
struct SetCameraView(CameraViewType);

#[derive(Resource)]
struct FloorSize(f32);

#[derive(Resource)]
struct CameraView(CameraViewType);

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Velocity(Vec3);

#[derive(Component)]
struct PlayerCamera;

#[derive(Component)]
struct PlayerWeapon;

#[derive(Component)]
struct Cube;

#[derive(Component)]
struct AimPoint(Vec3);

#[derive(Component)]
struct Lean(f32);

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

//TODO: Y axis mouse look limits - perfect down and up cause weirdness with lean recentering
fn lean_camera(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut transform: Single<&mut Transform, With<PlayerCamera>>,
    player_transform: Single<&GlobalTransform, (With<Player>, Without<PlayerCamera>)>,
    mut player_lean: Single<&mut Lean, (With<Player>, Without<PlayerCamera>)>,
) {
    let player_position = player_transform.translation();
    let rotation_point = Vec3::new(0_f32, 0_f32, transform.translation.z);
    let max_lean = 30_f32;
    let curr_angle = transform.rotation.to_euler(EulerRot::XYZ).2.to_degrees();
    let leaning_right = player_lean.0 > 0_f32;
    let leaning_left = player_lean.0 < 0_f32;
    let trying_to_lean_left = keyboard_input.pressed(KeyCode::KeyQ);
    let trying_to_lean_right = keyboard_input.pressed(KeyCode::KeyE);
    let trying_to_lean = trying_to_lean_left || trying_to_lean_right;
    let player_lean_speed = 4_f32 * time.delta_secs();
    let will_lean_left = trying_to_lean_left && !leaning_right;
    let will_lean_right = trying_to_lean_right && !leaning_left;
    let will_lean = will_lean_left || will_lean_right;
    // let will_recenter = !will_lean && is_leaning;

    let mut working_lean = player_lean.0;

    // lean
    if will_lean_left {
        working_lean -= player_lean_speed;
    } else if will_lean_right {
        working_lean += player_lean_speed;
    } else {
        // auto return to center
        if leaning_right {
            working_lean -= player_lean_speed;
        } else if leaning_left {
            working_lean += player_lean_speed;
        }
    }

    working_lean = working_lean.clamp(-1_f32, 1_f32);

    if working_lean.abs() < player_lean_speed && !trying_to_lean {
        working_lean = 0_f32;
    }

    if keyboard_input.just_pressed(KeyCode::KeyI) {
        info!("{:?}", working_lean);
    }

    // let easing = if will_lean {
    //     EaseFunction::CubicInOut
    // } else if will_recenter {
    //     EaseFunction::CubicInOut
    // } else {
    //     EaseFunction::ExponentialInOut
    // };

    // let translation_curve = EasingCurve::new(0_f32, max_lean, EaseFunction::CubicInOut);
    let translation_curve = EasingCurve::new(0_f32, max_lean, EaseFunction::SineOut);

    let alpha = if leaning_left || will_lean_left {
        -working_lean
    } else {
        working_lean
    };

    let mut curved_lean = translation_curve.sample(alpha).unwrap_or(0_f32);

    if leaning_right {
        curved_lean = -curved_lean;
    }

    let rotation_step = (curr_angle - curved_lean).to_radians();

    let rotation = Quat::from_axis_angle(Vec3::NEG_Z, rotation_step);

    transform.rotate_around(rotation_point, rotation);

    player_lean.0 = working_lean;
}

fn rotate_camera(
    mouse_motion: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
    mut transform: Single<&mut Transform, With<PlayerCamera>>,
) {
    let rotation_speed: f32 = 10_f32;
    let rotation_amount_x = (-mouse_motion.delta.y * rotation_speed) * time.delta_secs();

    transform.rotate_x(rotation_amount_x.to_radians());
}

fn rotate_player(
    mouse_motion: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
    mut transform: Single<&mut Transform, With<Player>>,
) {
    let rotation_speed: f32 = 0.5;
    let rotation_amount_y = -mouse_motion.delta.x * rotation_speed;

    transform.rotate_y(rotation_amount_y * time.delta_secs());
}

fn move_player(mut player_query: Query<(&mut Transform, &mut Velocity), With<Player>>) {
    let (mut player, velocity) = player_query.single_mut();

    let Vec3 { x, y: _, z } = player.rotation.mul_vec3(velocity.0);
    player.translation += Vec3::new(x, 0.0, z);
}

fn calc_new_velocity(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut player_velocity_query: Query<&mut Velocity, With<Player>>,
    time: Res<Time>,
) {
    // configurables

    // how much force the object uses to move
    const ACCELERATION: f32 = 1.0;

    // inverse drag - higher = less drag
    const DRAG_COF_INV: f32 = 5.0;

    let mut velocity = player_velocity_query.single_mut();
    let vel = &mut velocity.0;
    let delta = time.delta_secs();
    let (speed_x, speed_z) = (vel.x.abs(), vel.z.abs());
    let drag_x = add_drag(vel.x, speed_x / DRAG_COF_INV);
    let drag_z = add_drag(vel.z, speed_z / DRAG_COF_INV);

    vel.x = if speed_x < drag_x {
        0.0
    } else {
        vel.x + drag_x
    };

    vel.z = if speed_z < drag_z {
        0.0
    } else {
        vel.z + drag_z
    };

    let mut new_vel = Vec3::ZERO;
    if keyboard_input.pressed(KeyCode::KeyW) {
        new_vel.z = -1f32;
    }

    if keyboard_input.pressed(KeyCode::KeyA) {
        new_vel.x = -1f32;
    }

    if keyboard_input.pressed(KeyCode::KeyD) {
        new_vel.x = 1f32;
    }

    if keyboard_input.pressed(KeyCode::KeyS) {
        new_vel.z = 1f32;
    }

    if new_vel == Vec3::ZERO {
        return;
    }

    let norm = new_vel.normalize();

    vel.x += norm.x * ACCELERATION * delta;
    vel.z += norm.z * ACCELERATION * delta;
}

fn add_drag(vel: f32, drag: f32) -> f32 {
    match vel.partial_cmp(&0.0) {
        Some(std::cmp::Ordering::Greater) => -drag,
        Some(std::cmp::Ordering::Less) => drag,
        Some(std::cmp::Ordering::Equal) => 0.0,
        None => 0.0,
    }
}

fn change_camera_keybind(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
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

    commands.trigger(SetCameraView(camera_view.0));
}

fn set_camera(
    trigger: Trigger<SetCameraView>,
    mut camera: Single<&mut Transform, With<PlayerCamera>>,
    aimpoint: Single<&AimPoint, With<Player>>,
) {
    let view = trigger.event().0;

    if view == CameraViewType::TopDown {
        camera.translation.x = 0.0;
        camera.translation.y = 30.0;
        camera.translation.z = 0.0;
        camera.look_at(Vec3::NEG_Z, Vec3::Y);
    }

    if view == CameraViewType::ThirdPerson {
        camera.translation.x = 0.0;
        camera.translation.y = 1.0;
        camera.translation.z = 6.0;
        camera.look_at(Vec3::NEG_Z, Vec3::Y);
    }

    if view == CameraViewType::FirstPerson {
        camera.translation.x = 0.0;
        camera.translation.y = 0.85;
        camera.translation.z = -0.51;
        camera.look_at(aimpoint.0, Vec3::Y);
    }
}

// fn move_camera(
//     keyboard_input: Res<ButtonInput<KeyCode>>,
//     mut camera: Single<&mut Transform, With<PlayerCamera>>,
//     time: Res<Time>,
// ) {
//     let speed = 55.0 * time.delta_secs();
//     if keyboard_input.pressed(KeyCode::KeyE) {
//         camera.translation.y -= speed;
//         camera.look_at(Vec3::NEG_Z, Vec3::Y);
//     }
//
//     if keyboard_input.pressed(KeyCode::KeyQ) {
//         camera.translation.y += speed;
//         camera.look_at(Vec3::NEG_Z, Vec3::Y);
//     }
// }

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
            Velocity(Vec3::ZERO),
            AimPoint(aimpoint),
            Lean(0_f32),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Camera3d::default(),
                    PerspectiveProjection {
                        fov: 90.0_f32.to_radians(),
                        ..default()
                    },
                    Camera {
                        hdr: true, // 1. HDR is required for bloom
                        ..default()
                    },
                    Tonemapping::TonyMcMapface, // 2. Using a tonemapper that desaturates to white is recommended
                    Transform::from_xyz(0.0, 50.0, 0.0).looking_at(Vec3::NEG_Z, Vec3::Y),
                    Bloom::NATURAL,
                    PlayerCamera,
                ))
                .with_children(|parent_camera| {
                    let weapon_length = 1.0;
                    let weapon_radius = 0.05;
                    let actual_weapon_length = weapon_length - weapon_radius * 2.0;

                    parent_camera.spawn((
                        Mesh3d(meshes.add(Cuboid::new(
                            weapon_radius,
                            weapon_radius,
                            actual_weapon_length,
                        ))),
                        MeshMaterial3d(materials.add(Color::WHITE)),
                        Transform::from_xyz(0.05, -0.1, 0.0).looking_at(aimpoint, Vec3::Y),
                        NotShadowCaster,
                        PlayerWeapon,
                    ));
                });

            parent.spawn((
                PointLight {
                    shadows_enabled: true,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.5, 0.0),
            ));
        });

    commands.trigger(SetCameraView(CameraViewType::FirstPerson));
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
