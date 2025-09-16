use avian3d::prelude::*;
use bevy::{
    pbr::CascadeShadowConfigBuilder,
    prelude::{light_consts::lux, *},
    window::{CursorGrabMode, PrimaryWindow},
};
use std::f32::consts::PI;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_floor, add_border, setup_atmos))
            .add_systems(Update, (hide_cursor, dynamic_scene))
            .insert_resource(FloorSize(100.0))
            .run();
    }
}

#[derive(Resource)]
struct FloorSize(f32);

#[derive(Component)]
struct Cube;

fn dynamic_scene(mut suns: Query<&mut Transform, With<DirectionalLight>>, time: Res<Time>) {
    suns.iter_mut()
        .for_each(|mut tf| tf.rotate_x(-time.delta_secs() * PI / 200.0));
}

fn setup_atmos(mut commands: Commands) {
    let cascade_shadow_config = CascadeShadowConfigBuilder { ..default() }.build();

    // Sun
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            illuminance: lux::DIRECT_SUNLIGHT,
            ..default()
        },
        Transform::from_xyz(3.0, 5.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        cascade_shadow_config,
    ));

    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 3000.0,
        ..default()
    });
}

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
    let floor_size_value: f32 = floor_size.0;
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(floor_size_value, 1., floor_size_value))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_xyz(0.0, 0.0, 0.0),
        RigidBody::Static,
        Collider::cuboid(floor_size_value, 1., floor_size_value),
    ));

    // ocean so we don't see the infinite blackness
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(5000.0, 5000.0))),
        MeshMaterial3d(materials.add(Color::Srgba(Srgba {
            red: 0.0,
            green: 0.45,
            blue: 0.6,
            alpha: 1.0,
        }))),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}

fn add_border(
    mut commands: Commands,
    floor_size_res: Res<FloorSize>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let floor_size = floor_size_res.0;
    const HEIGHT: f32 = 2.0;

    let wall_mesh_x = meshes.add(Cuboid::new(floor_size, HEIGHT, 1.0));
    let wall_mesh_y = meshes.add(Cuboid::new(1.0, HEIGHT, floor_size));

    let cube_mat = materials.add(Color::srgb_u8(124, 144, 255));

    let transform_t = Transform::from_xyz(0.0, 0.0, floor_size / 2.0);
    let transform_b = Transform::from_xyz(0.0, 0.0, -floor_size / 2.0);

    let transform_l = Transform::from_xyz(floor_size / 2., 0.0, 0.0);
    let transform_r = Transform::from_xyz(-floor_size / 2., 0.0, 0.0);

    commands.spawn((
        RigidBody::Static,
        Mesh3d(wall_mesh_x.clone()),
        MeshMaterial3d(cube_mat.clone()),
        transform_t,
        Cube,
        Collider::cuboid(floor_size, HEIGHT, 1.),
    ));

    commands.spawn((
        RigidBody::Static,
        Mesh3d(wall_mesh_x.clone()),
        MeshMaterial3d(cube_mat.clone()),
        transform_b,
        Cube,
        Collider::cuboid(floor_size, HEIGHT, 1.),
    ));

    commands.spawn((
        RigidBody::Static,
        Mesh3d(wall_mesh_y.clone()),
        MeshMaterial3d(cube_mat.clone()),
        transform_l,
        Cube,
        Collider::cuboid(1., HEIGHT, floor_size),
    ));

    commands.spawn((
        RigidBody::Static,
        Mesh3d(wall_mesh_y.clone()),
        MeshMaterial3d(cube_mat.clone()),
        transform_r,
        Cube,
        Collider::cuboid(1., HEIGHT, floor_size),
    ));
}
