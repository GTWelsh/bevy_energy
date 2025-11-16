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

/*
 * IDEAS
 * - Walking alpha
 * - Breathing alpha
 *
 * */

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
            ((aim, breathing, weapon_sway, set_weapon_transform).chain(),),
        )
        .run();
}

#[derive(Component)]
struct Player;

#[derive(Component)]
enum WalkingStride {
    Left(f32),
    Right(f32),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum BreathDirection {
    In = 0,
    Out = 1,
}

#[derive(Component)]
struct BreathIntake {
    alpha: f32,
    direction: BreathDirection,
}

#[derive(Component)]
struct PlayerCamera;

#[derive(Component)]
struct PlayerWeapon;

// sway direction enum and a sway Vec3

struct WeaponSway {
    base: Vec3,
    new: Vec3,
}

fn breathing(time: Res<Time>, players_q: Query<&mut BreathIntake, With<Player>>) {
    let speed = 0.75 * time.delta_secs(); // easy thing to modify based on player state

    for mut breath in players_q {
        breath.alpha += speed;

        if breath.alpha >= 1.0 {
            breath.alpha = 0.0;
            breath.direction = if breath.direction == BreathDirection::In {
                BreathDirection::Out
            } else {
                BreathDirection::In
            };
        }
    }
}

fn weapon_sway(
    players_q: Query<(&BreathIntake, &Children), With<Player>>,
    camera_q: Query<(&PlayerCamera, &Children)>,
    mut sway_base: Local<Vec3>, //TODO: make this local to the player, not the system
    mut sway_new: Local<Vec3>,  //TODO: make this local to the player, not the system
    mut weapon_query: Query<&mut TranslationPipeline, (With<PlayerWeapon>, With<WeaponActive>)>,
) {
    let max_sway = 0.0005; // easy thing to modify based on player state

    for (breath, children) in players_q {
        let breath_alpha = breath.alpha;

        if breath_alpha >= 1.0 || breath_alpha == 0.0 {
            *sway_base = *sway_new;
        }

        let change_sway = *sway_base == *sway_new;
        let mut rng = rand::rng();

        if change_sway {
            *sway_new = Vec3::new(
                rng.random_range(-max_sway..=max_sway),
                rng.random_range(-max_sway..=max_sway),
                rng.random_range(-max_sway..=max_sway),
            );
        }

        let curve = EaseFunction::SmoothStep;
        let curve_alpha = EasingCurve::new(0.0, 1.0, curve)
            .sample(breath.alpha)
            .unwrap();

        // query the children -> player (here) -> camera -> weapon
        for &camera_entity in children {
            let camera = camera_q.get(camera_entity);

            if camera.is_err() {
                continue;
            }

            for &child in camera.unwrap().1 {
                if let Ok(mut position_pipe) = weapon_query.get_mut(child) {
                    let position = position_pipe.latest();
                    let new_base_sway_vec = position + *sway_base;
                    let new_target_sway_vec = position + *sway_new;
                    let sway_diff = new_target_sway_vec - new_base_sway_vec;
                    let new_position = *sway_base + (sway_diff * curve_alpha);

                    position_pipe.queue(new_position);
                }
            }
        }
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
        (&mut Transform, &mut TranslationPipeline),
        (With<PlayerWeapon>, With<WeaponActive>),
    >,
) {
    for (mut trans, mut current_translation) in &mut weapon_query {
        trans.translation = current_translation.apply();
    }
}

fn aim(
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut weapon_query: Query<
        (
            &mut TranslationPipeline,
            &DefaultTransform,
            &SightOffsetTransform,
            &mut AdsAlpha,
        ),
        (With<PlayerWeapon>, With<WeaponActive>),
    >,
) {
    for (mut current_transform, default_transform, aim_transform, mut ads_alpha) in
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

        current_transform.queue(aim_difference * curve_alpha);
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
struct TranslationPipeline {
    base_translation: Vec3,
    additive_translations: Vec<Vec3>,
}

impl TranslationPipeline {
    fn new(translation: Vec3) -> Self {
        Self {
            base_translation: translation,
            additive_translations: vec![],
        }
    }

    fn queue(&mut self, translation: Vec3) -> &Self {
        self.additive_translations.push(translation);
        self
    }

    fn latest(&mut self) -> Vec3 {
        let mut output = self.base_translation;

        for t in self.additive_translations.iter() {
            output += t;
        }

        output
    }

    fn apply(&mut self) -> Vec3 {
        let mut output = self.base_translation;

        while let Some(t) = self.additive_translations.pop() {
            output += t;
        }

        output
    }
}

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
            WalkingStride::Right(0.0),
            BreathIntake {
                alpha: 0.0,
                direction: BreathDirection::Out,
            },
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
                    let ads_position = Vec3::new(0.0, -0.07, -0.3);
                    let hip_position = Vec3::new(0.1, -0.1, -0.5);
                    parent_camera.spawn((
                        SceneRoot(
                            asset_server
                                .load(GltfAssetLabel::Scene(0).from_asset("weapons/mpx/main.glb")),
                        ),
                        Transform::from_xyz(hip_position.x, hip_position.y, hip_position.z)
                            .looking_to(Vec3::NEG_Z, Vec3::Y),
                        PlayerWeapon,
                        WeaponActive,
                        TranslationPipeline::new(hip_position),
                        DefaultTransform(hip_position),
                        SightOffsetTransform(ads_position),
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
