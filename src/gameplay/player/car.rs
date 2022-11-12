use crate::gameplay::player::Player;
use bevy::prelude::*;
use bevy_inspector_egui::Inspectable;
use bevy_rapier2d::prelude::*;

#[derive(Component)]
pub struct FrontLeftWheel;
#[derive(Component)]
pub struct FrontRightWheel;
#[derive(Component)]
pub struct RearLeftWheel;
#[derive(Component)]
pub struct RearRightWheel;
#[derive(Component)]
pub struct SpikesUpgrade;

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let player_handle = asset_server.load("textures/car1/chassis1.png");
    let wheel_handle = asset_server.load("textures/car1/wheel1.png");
    let spikes_handle = asset_server.load("textures/car1/spikes1.png");

    // Todo - remove me
    commands
        .spawn()
        .insert(Collider::cuboid(500.0, 50.0))
        .insert_bundle(TransformBundle::from(Transform::from_xyz(0.0, -100.0, 0.0)));

    // Player car
    commands
        .spawn()
        .insert(Name::new("Player"))
        .insert(Player {
            movement_speed: 500.0,                  // metres per second
            rotation_speed: f32::to_radians(360.0), // degrees per second
        })
        .insert_bundle(SpriteBundle {
            texture: player_handle,
            transform: Transform {
                translation: Vec3::new(-430.0, 200.0, 5.0),
                rotation: Quat::from_rotation_z(f32::to_radians(8.0)),
                scale: Vec3::new(0.2, 0.2, 0.2),
            },
            ..default()
        })
        .insert(Velocity::zero())
        .insert(RigidBody::Dynamic)
        .insert(Collider::ball(350.0))
        .insert(Restitution::coefficient(0.7))
        .insert(Ccd::enabled())
        .with_children(|parent| {
            // Front left wheel
            parent
                .spawn()
                .insert(Name::new("Wheel (FL)"))
                .insert_bundle(SpriteBundle {
                    texture: wheel_handle.clone(),
                    transform: Transform {
                        translation: Vec3::new(-117.0, 128.0, 4.9),
                        rotation: Quat::from_rotation_z(f32::to_radians(0.0)),
                        scale: Vec3::new(1.1, 1.1, 0.0),
                    },
                    ..default()
                })
                .insert(Collider::ball(50.0))
                .insert(FrontLeftWheel);

            // Front right wheel
            parent
                .spawn()
                .insert(Name::new("Wheel (FR)"))
                .insert_bundle(SpriteBundle {
                    texture: wheel_handle.clone(),
                    transform: Transform {
                        translation: Vec3::new(114.0, 128.0, 4.9),
                        rotation: Quat::from_rotation_z(f32::to_radians(0.0)),
                        scale: Vec3::new(1.1, 1.1, 0.0),
                    },
                    ..default()
                })
                .insert(Collider::ball(50.0))
                .insert(FrontRightWheel);

            // Rear left wheel
            parent
                .spawn()
                .insert(Name::new("Wheel (RL)"))
                .insert_bundle(SpriteBundle {
                    texture: wheel_handle.clone(),
                    transform: Transform {
                        translation: Vec3::new(-115.0, -167.5, 4.9),
                        rotation: Quat::from_rotation_z(f32::to_radians(0.0)),
                        scale: Vec3::new(1.2, 1.2, 0.0),
                    },
                    ..default()
                })
                .insert(Collider::ball(50.0))
                .insert(RearLeftWheel);

            // Rear right wheel
            parent
                .spawn()
                .insert(Name::new("Wheel (RR)"))
                .insert_bundle(SpriteBundle {
                    texture: wheel_handle.clone(),
                    transform: Transform {
                        translation: Vec3::new(115.0, -167.5, 4.9),
                        rotation: Quat::from_rotation_z(f32::to_radians(0.0)),
                        scale: Vec3::new(1.2, 1.2, 0.0),
                    },
                    ..default()
                })
                .insert(Collider::ball(50.0))
                .insert(RearRightWheel);

            // Spikes
            parent
                .spawn()
                .insert(Name::new("Spikes"))
                .insert_bundle(SpriteBundle {
                    texture: spikes_handle,
                    transform: Transform {
                        translation: Vec3::new(0.0, 225.0, 5.2),
                        rotation: Quat::from_rotation_z(f32::to_radians(0.0)),
                        scale: Vec3::new(1.0, 1.0, 0.0),
                    },
                    ..default()
                })
                .insert(Collider::ball(50.0))
                .insert(SpikesUpgrade);
        });
}
