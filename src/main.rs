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
            (
                (rotate_horizontal, look_vertical).chain(),
                player_shoot,
                player_breath_alter,
            ),
        )
        .add_systems(
            FixedUpdate,
            ((aim, player_breath, weapon_sway, set_weapon_transform).chain(),),
        )
        .run();
}

#[derive(Component)]
struct Player;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum BreathDirection {
    In = 0,
    Out = 1,
}

#[derive(Component, Debug)]
struct Breath {
    speed: f32,
    alpha: f32,
    amount: f32,
    depth: f32,
    direction: BreathDirection,
}

impl Breath {
    const MAX_SPEED: f32 = 10.0;
    const MAX_DEPTH: f32 = 5.0;

    fn breath(&mut self, delta: f32) {
        self.speed = self.speed.clamp(0.0, Self::MAX_SPEED);
        self.depth = self.depth.clamp(0.0, Self::MAX_DEPTH);

        // clamp to max breathing speed to ensure shallow breaths (<1.0) at max breath effort doesnt
        // create insane breathing rates
        let breathing_rate = (self.speed / self.depth).clamp(0.0, Self::MAX_SPEED);

        // increase alpha slower for deeper breaths
        self.alpha += breathing_rate * delta;

        let change_breath = self.alpha >= 1.0 || self.alpha <= 0.0;

        if change_breath {
            self.alpha = 0.0;
            self.direction = if self.direction == BreathDirection::In {
                BreathDirection::Out
            } else {
                BreathDirection::In
            };
        }

        self.amount = EasingCurve::new(0.0, self.depth, EaseFunction::SmoothStep)
            .sample(self.alpha)
            .unwrap_or_else(|| panic!("breath alpha not between 0 + {}", self.depth));
    }
}

#[derive(Component)]
struct PlayerCamera;

#[derive(Component)]
struct PlayerWeapon;

fn player_breath(time: Res<Time>, players_q: Query<&mut Breath, With<Player>>) {
    for mut breath in players_q {
        breath.breath(time.delta_secs());
    }
}

fn player_breath_alter(
    players_q: Query<&mut Breath, With<Player>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    for mut breath in players_q {
        if keys.pressed(KeyCode::BracketRight) {
            breath.depth += 0.1;
        }

        if keys.pressed(KeyCode::BracketLeft) {
            breath.depth -= 0.1;
        }

        if keys.pressed(KeyCode::PageUp) {
            breath.speed += 0.1;
        }

        if keys.pressed(KeyCode::PageDown) {
            breath.speed -= 0.1;
        }
    }
}

#[derive(Component, Default)]
struct WeaponSway {
    max_sway: f32,
    base: Vec3,
    next: Vec3,
}

impl WeaponSway {
    fn new(max_sway: f32) -> Self {
        Self {
            max_sway,
            ..default()
        }
    }

    fn renew(&mut self) {
        self.base = self.next;
    }

    fn change(&mut self, breath: &Breath) {
        let mut rng = rand::rng();

        let effective_sway = self.max_sway * breath.depth;
        let half_sway = effective_sway / 2.0;

        let sway_in = if breath.direction == BreathDirection::In {
            effective_sway
        } else {
            0.0
        };

        let sway_out = if breath.direction == BreathDirection::Out {
            effective_sway
        } else {
            0.0
        };

        let x_range = -half_sway..=half_sway;
        let y_range = -sway_in..=sway_out;
        let z_range = -effective_sway..=effective_sway;

        if x_range.is_empty() || y_range.is_empty() || z_range.is_empty() {
            return;
        }

        self.next = Vec3::new(
            // smaller half-sway in the X
            rng.random_range(-half_sway..=half_sway),
            // flip-flop up and down full sway for Y
            rng.random_range(-sway_in..=sway_out),
            // full sway range in the Z
            rng.random_range(-effective_sway..=effective_sway),
        );
    }

    fn is_complete(&self) -> bool {
        self.base == self.next
    }

    /// Return the vector from base to next relative to the origin
    ///
    /// This gives us a way of swaying from one sway location to another without having to revisit
    /// the centre
    fn diff_from(&self, origin: Vec3) -> Vec3 {
        let base_from_origin = origin + self.base;
        let next_from_origin = origin + self.next;
        next_from_origin - base_from_origin
    }

    /// Lerp from the old sway target (base) to the new sway target (next)
    fn lerp_from(&self, origin: Vec3, alpha: f32) -> Vec3 {
        self.base + self.diff_from(origin) * alpha
    }
}

fn weapon_sway(
    players_q: Query<(&Breath, &mut WeaponSway, &Children), With<Player>>,
    camera_q: Query<(&PlayerCamera, &Children)>,
    mut weapon_query: Query<&mut TranslationPipeline, (With<PlayerWeapon>, With<WeaponActive>)>,
) {
    for (breath, mut weapon_sway, children) in players_q {
        let breath_alpha = breath.alpha;

        if breath_alpha >= 1.0 || breath_alpha == 0.0 {
            weapon_sway.renew();
        }

        let change_sway = weapon_sway.is_complete();

        if change_sway {
            weapon_sway.change(breath);
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
                    position_pipe.queue(weapon_sway.lerp_from(position, curve_alpha));
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
            &PlayerWeaponTransformConfig,
            &mut AdsAlpha,
        ),
        (With<PlayerWeapon>, With<WeaponActive>),
    >,
) {
    for (mut current_transform, transform_config, mut ads_alpha) in &mut weapon_query {
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

        current_transform.queue(transform_config.aim_difference() * curve_alpha);
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
struct PlayerWeaponTransformConfig {
    hip: Vec3,
    aim: Vec3,
}

impl PlayerWeaponTransformConfig {
    fn new(hip: Vec3, aim: Vec3) -> Self {
        Self { hip, aim }
    }

    fn aim_difference(&self) -> Vec3 {
        self.aim - self.hip
    }
}

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
            Breath {
                amount: 0.0,
                speed: 0.75,
                depth: 1.0,
                alpha: 0.0,
                direction: BreathDirection::Out,
            },
            WeaponSway::new(0.0005),
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
                    let aim_position = Vec3::new(0.0, -0.07, -0.3);
                    let hip_position = Vec3::new(0.1, -0.1, -0.5);
                    let transform_config =
                        PlayerWeaponTransformConfig::new(hip_position, aim_position);

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
                        transform_config,
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
