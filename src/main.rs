#![allow(clippy::type_complexity)]

mod movement;
mod scene;

use avian3d::PhysicsPlugins;
use avian3d::math::Scalar;
use avian3d::prelude::{
    CoefficientCombine, Collider, Friction, GravityScale, Restitution, RigidBody,
};
use bevy::camera::Exposure;
use bevy::pbr::Atmosphere;
use bevy::post_process::bloom::Bloom;
use bevy::{
    core_pipeline::tonemapping::Tonemapping, input::mouse::AccumulatedMouseMotion, prelude::*,
};
use bevy_dev_tools::fps_overlay::FpsOverlayPlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            FpsOverlayPlugin::default(),
            PhysicsPlugins::default(),
            scene::ScenePlugin,
            movement::CharacterControllerPlugin,
        ))
        .add_systems(Startup, setup_player)
        .add_systems(
            Update,
            (
                (lean_camera, rotate_horizontal, look_vertical).chain(),
                player_shoot,
                aim,
            ),
        )
        .run();
}

#[derive(Component)]
struct Player;

#[derive(Component)]
struct PlayerCamera;

#[derive(Component)]
struct PlayerWeapon;

#[derive(Component)]
struct Lean(f32);

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
        RigidBody::Dynamic,
        Collider::sphere(0.05),
    ));
}

fn aim(
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut weapon_query: Query<
        (
            &mut Transform,
            &DefaultTransform,
            &SightOffsetTransform,
            &mut AdsAlpha,
        ),
        (With<PlayerWeapon>, With<WeaponActive>),
    >,
) {
    for (mut trans, def_trans, sight_trans, mut ads_alpha) in &mut weapon_query {
        const AIM_SPEED: f32 = 0.03;

        let step = if mouse_input.pressed(MouseButton::Right) {
            1.0
        } else {
            -1.0
        } * AIM_SPEED;

        let ease = if step > 0.0 {
            EaseFunction::ExponentialOut
        } else {
            EaseFunction::CircularInOut
        };

        let new_ads_alpha = ads_alpha.0 + step;
        ads_alpha.0 = new_ads_alpha.clamp(0.0, 1.0);

        let curve_alpha = EasingCurve::new(0.0, 1.0, ease)
            .sample(ads_alpha.0)
            .unwrap_or(0.0);

        let diff = sight_trans.0 - def_trans.0;

        trans.translation = def_trans.0 + (diff * curve_alpha);
    }
}

fn lean_camera(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut transform: Single<&mut Transform, With<PlayerCamera>>,
    mut player_lean: Single<&mut Lean, (With<Player>, Without<PlayerCamera>)>,
) {
    let rotation_point = Vec3::new(
        transform.translation.x,
        transform.translation.y - 2.0,
        transform.translation.z,
    );
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

    let rotation_speed: f32 = 6_f32;
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
    let rotation_speed: f32 = 0.2;
    let rotation_amount_y = -mouse_motion.delta.x * rotation_speed;

    transform.rotate_y(rotation_amount_y * time.delta_secs());
}

#[derive(Component)]
struct AdsAlpha(f32);

#[derive(Component)]
struct SightOffsetTransform(Vec3);

#[derive(Component)]
struct DefaultTransform(Vec3);

#[derive(Component)]
struct WeaponActive;

fn setup_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let height = 2.0;
    let radius = 0.5;
    let transform = Transform::from_translation(Vec3::new(radius, height / 2. + radius, radius));

    commands
        .spawn((
            Mesh3d(meshes.add(Capsule3d::new(radius, height))),
            MeshMaterial3d(materials.add(Color::WHITE)),
            Player,
            transform,
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
                        fov: 36_f32.to_radians(),
                        aspect_ratio: 16. / 9.,
                        near: 0.001,
                        far: 1000.,
                    }),
                    Camera { ..default() },
                    Atmosphere::EARTH,
                    Exposure::SUNLIGHT,
                    Tonemapping::AcesFitted,
                    Transform::from_xyz(0.0, 0.85, -0.51).looking_to(Vec3::NEG_Z, Vec3::Y),
                    Bloom::NATURAL,
                    PlayerCamera,
                ))
                .with_children(|parent_camera| {
                    let hip_pos = Vec3::new(0.1, -0.1, -0.5);
                    let ads_pos = Vec3::new(0.0, -0.07, -0.3);
                    parent_camera.spawn((
                        SceneRoot(
                            asset_server
                                .load(GltfAssetLabel::Scene(0).from_asset("weapons/mpx/main.glb")),
                        ),
                        Transform::from_xyz(0.1, -0.1, -0.5).looking_to(Vec3::NEG_Z, Vec3::Y),
                        PlayerWeapon,
                        WeaponActive,
                        DefaultTransform(hip_pos),
                        SightOffsetTransform(ads_pos),
                        AdsAlpha(0.0),
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
}
