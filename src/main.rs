mod movement;

use avian3d::PhysicsPlugins;
use avian3d::math::Scalar;
use avian3d::prelude::{
    AngularVelocity, CoefficientCombine, Collider, Friction, GravityScale, Restitution, RigidBody,
};
use bevy::pbr::NotShadowCaster;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use bevy::{
    core_pipeline::{bloom::Bloom, tonemapping::Tonemapping},
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            PhysicsPlugins::default(),
            movement::CharacterControllerPlugin,
        ))
        .add_systems(Startup, (setup_floor, setup_player, add_cubes))
        .add_systems(
            Update,
            (
                hide_cursor,
                (lean_camera, rotate_horizontal, look_vertical).chain(),
                change_camera_keybind,
                player_shoot,
            ),
        )
        .add_observer(set_camera)
        .insert_resource(FloorSize(100.0))
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
struct PlayerCamera;

#[derive(Component)]
struct PlayerWeapon;

#[derive(Component)]
struct Cube;

#[derive(Component)]
struct AimPoint(Vec3);

#[derive(Component)]
struct Lean(f32);

fn hide_cursor(
    mut q_windows: Query<&mut Window, With<PrimaryWindow>>,
    mut lock_cursor: Local<bool>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    let Ok(mut window) = q_windows.single_mut() else {
        return;
    };

    if *lock_cursor {
        window.cursor_options.grab_mode = CursorGrabMode::Confined;
        window.cursor_options.visible = false;
    } else {
        window.cursor_options.grab_mode = CursorGrabMode::None;
        window.cursor_options.visible = true;
    }

    if keyboard_input.just_pressed(KeyCode::F1) {
        *lock_cursor = !*lock_cursor;
    }
}

fn setup_floor(
    mut commands: Commands,
    floor_size: Res<FloorSize>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(floor_size.0, 1., floor_size.0))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_xyz(0.0, -1., 0.0),
        RigidBody::Static,
        Collider::cuboid(floor_size.0, 1., floor_size.0),
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

fn lean_camera(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut transform: Single<&mut Transform, With<PlayerCamera>>,
    mut player_lean: Single<&mut Lean, (With<Player>, Without<PlayerCamera>)>,
) {
    let rotation_point = Vec3::new(0_f32, 0_f32, transform.translation.z);
    let max_lean = 30_f32;
    let curr_angle = transform.rotation.to_euler(EulerRot::XYZ).2.to_degrees();
    let leaning_right = player_lean.0 > 0_f32;
    let leaning_left = player_lean.0 < 0_f32;
    let trying_to_lean_left = keyboard_input.pressed(KeyCode::KeyQ);
    let trying_to_lean_right = keyboard_input.pressed(KeyCode::KeyE);
    let player_lean_speed = 4_f32 * time.delta_secs();
    let will_lean_left = trying_to_lean_left && !leaning_right;
    let will_lean_right = trying_to_lean_right && !leaning_left;
    let will_lean = will_lean_left || will_lean_right;

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

    let easing = if will_lean {
        EaseFunction::SineOut
    } else {
        EaseFunction::SineIn
    };

    let translation_curve = EasingCurve::new(0_f32, max_lean, easing);

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

fn look_vertical(
    mouse_motion: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
    mut transform: Single<&mut Transform, With<PlayerCamera>>,
) {
    const LIMIT: f32 = 45_f32;
    const ZERO: f32 = 0_f32;

    let rotation_speed: f32 = 10_f32;
    let rotation_amount_x = (-mouse_motion.delta.y * rotation_speed) * time.delta_secs();
    let positive_rot = rotation_amount_x > ZERO;
    let negative_rot = rotation_amount_x < ZERO;
    let current_rot = transform.rotation.to_euler(EulerRot::XYZ).0.to_degrees();
    let high = current_rot > LIMIT && positive_rot;
    let low = current_rot < -LIMIT && negative_rot;

    if high || low {
        return;
    }

    transform.rotate_x(rotation_amount_x.to_radians());
}

fn rotate_horizontal(
    mouse_motion: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
    mut transform: Single<&mut Transform, With<Player>>,
) {
    let rotation_speed: f32 = 0.5;
    let rotation_amount_y = -mouse_motion.delta.x * rotation_speed;

    transform.rotate_y(rotation_amount_y * time.delta_secs());
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

fn setup_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let height = 2.0;
    let radius = 0.5;
    let aimpoint = Vec3::new(0.0, 0.85, -10.0);
    let transform = Transform::from_translation(Vec3::new(radius, height / 2. + radius, radius));

    commands
        .spawn((
            Mesh3d(meshes.add(Capsule3d::new(radius, height))),
            MeshMaterial3d(materials.add(Color::WHITE)),
            Player,
            transform,
            AimPoint(aimpoint),
            Lean(0_f32),
            movement::CharacterControllerBundle::new(Collider::capsule(radius, height))
                .with_movement(25.0, 2., 0.85, 7.0, (30.0 as Scalar).to_radians()),
            Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
            Restitution::ZERO.with_combine_rule(CoefficientCombine::Min),
            GravityScale(2.0),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Camera3d::default(),
                    Projection::Perspective(PerspectiveProjection {
                        fov: 60_f32.to_radians(),
                        aspect_ratio: 1.,
                        near: 0.001,
                        far: 1000.,
                    }),
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
    let size_mutli = 10.0;
    let density = 5.0 / size_mutli;
    let max_number_of_cubes = (floor_size.0.floor() * density) as i32;
    let actual_number_of_cubes = rand::random_range(1..max_number_of_cubes);
    let cube_type_range = 0..10;
    let mut cube_meshes: Vec<Handle<Mesh>> = Vec::with_capacity(5);

    for _i in cube_type_range {
        let mesh = Cuboid::new(1., 1., 1.);
        cube_meshes.push(meshes.add(mesh));
    }

    let cube_mat = materials.add(Color::srgb_u8(124, 144, 255));

    let range_of_all_cubes = 1..actual_number_of_cubes;

    for _i in range_of_all_cubes {
        let cube_mesh = &cube_meshes[rand::random_range(0..10)];

        let half_floor_size = floor_size.0 / 2.0;

        let height = rand::random_range(1. ..5.);
        let width = rand::random_range(1. ..5.);
        let depth = rand::random_range(1. ..5.);

        let x: f32 =
            (rand::random_range(0.0..floor_size.0) - half_floor_size).clamp(0_f32, floor_size.0);
        let y: f32 = height / 2.;
        let z: f32 =
            (rand::random_range(0.0..floor_size.0) - half_floor_size).clamp(0_f32, floor_size.0);

        let transform = Transform::from_xyz(x, y + height, z)
            // .with_rotation(
            //     Quat::from_rotation_y(rand::random_range(0.0..TAU)))
            .with_scale(Vec3::new(width, height, depth));

        commands.spawn((
            RigidBody::Dynamic,
            Mesh3d(cube_mesh.clone()),
            MeshMaterial3d(cube_mat.clone()),
            transform,
            AngularVelocity(Vec3::new(width, height, depth)),
            Cube,
            Collider::cuboid(1., 1., 1.), //ColliderConstructorHierarchy::new(ColliderConstructor::ConvexDecompositionFromMesh),
        ));
    }
}
