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
use rand::Rng;

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
            ((rotate_horizontal, look_vertical).chain(), player_shoot),
        )
        .add_systems(
            FixedUpdate,
            ((aim, weapon_sway, set_weapon_transform).chain(),),
        )
        .run();
}

#[derive(Component)]
struct Player;

#[derive(Component)]
struct PlayerCamera;

#[derive(Component)]
struct PlayerWeapon;

fn weapon_sway(
    time: Res<Time>,
    mut alpha: Local<f32>,
    mut sway_right: Local<bool>,
    mut sway_to: Local<Vec3>,
    mut weapon_query: Query<&mut CurrentTranslation, (With<PlayerWeapon>, With<WeaponActive>)>,
) {
    let speed = 0.0075; // easy thing to modify based on player state
    let max_sway = 0.05; // easy thing to modify based on player state
    let mut change_sway = false;

    if *alpha >= 1.0 {
        *sway_right = false;
    } else if *alpha <= 0.0 {
        *sway_right = true;
        change_sway = true; // signal we are restarting the sway animation, change it up a bit
    }

    let mut rng = rand::rng();

    // make a random sway target vector to go towards
    if change_sway {
        *sway_to = Vec3::new(
            rng.random_range(-max_sway..=max_sway),
            rng.random_range(-max_sway..=max_sway),
            rng.random_range(-max_sway..=max_sway),
        );
    }

    if !*sway_right {
        *alpha -= speed;
    } else {
        *alpha += speed;
    }

    *alpha = alpha.clamp(0.0, 1.0);

    let curve = EaseFunction::SmoothStep;
    let curve_alpha = EasingCurve::new(0.0, 1.0, curve).sample(*alpha).unwrap();

    for mut current_translation in &mut weapon_query {
        current_translation.0 += *sway_to * curve_alpha * time.delta_secs();
    }
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
        RigidBody::Dynamic,
        Collider::sphere(0.05),
    ));
}

fn set_weapon_transform(
    mut weapon_query: Query<
        (&mut Transform, &CurrentTranslation),
        (With<PlayerWeapon>, With<WeaponActive>),
    >,
) {
    for (mut trans, current_translation) in &mut weapon_query {
        trans.translation = current_translation.0;
    }
}

fn aim(
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut weapon_query: Query<
        (
            &mut Transform,
            &mut CurrentTranslation,
            &DefaultTransform,
            &SightOffsetTransform,
            &mut AdsAlpha,
        ),
        (With<PlayerWeapon>, With<WeaponActive>),
    >,
) {
    for (mut trans, mut current_transform, default_transform, aim_transform, mut ads_alpha) in
        &mut weapon_query
    {
        // these values could come from some kind of config and or multipliers
        const AIM_TIME: f32 = 0.05;
        const UN_AIM_TIME: f32 = 0.05;

        let step = if mouse_input.pressed(MouseButton::Right) {
            AIM_TIME
        } else {
            -UN_AIM_TIME
        };

        let ease = if step > 0.0 {
            EaseFunction::QuarticOut
        } else {
            EaseFunction::QuinticInOut
        };

        let new_ads_alpha = ads_alpha.0 + step;
        ads_alpha.0 = new_ads_alpha.clamp(0.0, 1.0);

        let curve_alpha = EasingCurve::new(0.0, 1.0, ease)
            .sample(ads_alpha.0)
            .unwrap_or(0.0);

        let aim_difference = aim_transform.0 - default_transform.0;

        current_transform.0 = default_transform.0 + (aim_difference * curve_alpha);
    }
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
struct CurrentTranslation(Vec3);

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
                    let current_transform = Vec3::new(0.1, -0.1, -0.5);
                    parent_camera.spawn((
                        SceneRoot(
                            asset_server
                                .load(GltfAssetLabel::Scene(0).from_asset("weapons/mpx/main.glb")),
                        ),
                        Transform::from_xyz(
                            current_transform.x,
                            current_transform.y,
                            current_transform.z,
                        )
                        .looking_to(Vec3::NEG_Z, Vec3::Y),
                        PlayerWeapon,
                        WeaponActive,
                        CurrentTranslation(current_transform),
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
