use crate::gameplay::player::Player;
use bevy::prelude::*;

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

    // Wheel2 needs approx position of -132, 135 (front) -188 (rear), and scale 0.7

    commands
        .spawn_empty()
        .insert(Name::new("Player"))
        .insert(Player {
            movement_speed: 500.0,                  // metres per second
            rotation_speed: f32::to_radians(360.0), // degrees per second
        })
        .insert(SpriteBundle {
            texture: player_handle,
            transform: Transform {
                translation: Vec3::new(-430.0, 0.0, 3.0),
                rotation: Quat::from_rotation_z(f32::to_radians(8.0)),
                scale: Vec3::new(0.2, 0.2, 0.2),
            },
            ..default()
        })
        .insert(RigidBody::Dynamic)
        .insert(Velocity::zero())
        .insert(Collider::ball(350.0))
        .insert(Restitution::coefficient(0.7))
        .insert(Ccd::enabled())
        .with_children(|parent| {
            // Front left wheel
            parent
                .spawn_empty()
                .insert(Name::new("Wheel (FL)"))
                .insert(FrontLeftWheel)
                .insert(SpriteBundle {
                    texture: wheel_handle.clone(),
                    transform: Transform {
                        translation: Vec3::new(-117.0, 128.0, 0.0),
                        rotation: Quat::from_rotation_z(f32::to_radians(0.0)),
                        scale: Vec3::new(1.1, 1.1, 1.1),
                    },
                    ..default()
                });

            // Front right wheel
            parent
                .spawn_empty()
                .insert(Name::new("Wheel (FR)"))
                .insert(FrontRightWheel)
                .insert(SpriteBundle {
                    texture: wheel_handle.clone(),
                    transform: Transform {
                        translation: Vec3::new(114.0, 128.0, 0.0),
                        rotation: Quat::from_rotation_z(f32::to_radians(0.0)),
                        scale: Vec3::new(1.1, 1.1, 1.01),
                    },
                    ..default()
                });

            // Rear left wheel
            parent
                .spawn_empty()
                .insert(Name::new("Wheel (RL)"))
                .insert(RearLeftWheel)
                .insert(SpriteBundle {
                    texture: wheel_handle.clone(),
                    transform: Transform {
                        translation: Vec3::new(-115.0, -167.5, 0.0),
                        rotation: Quat::from_rotation_z(f32::to_radians(0.0)),
                        scale: Vec3::new(1.2, 1.2, 1.2),
                    },
                    ..default()
                });

            // Rear right wheel
            parent
                .spawn_empty()
                .insert(Name::new("Wheel (RR)"))
                .insert(RearRightWheel)
                .insert(SpriteBundle {
                    texture: wheel_handle.clone(),
                    transform: Transform {
                        translation: Vec3::new(115.0, -167.5, 0.0),
                        rotation: Quat::from_rotation_z(f32::to_radians(0.0)),
                        scale: Vec3::new(1.2, 1.2, 1.2),
                    },
                    ..default()
                });

            // Spikes
            parent
                .spawn_empty()
                .insert(Name::new("Spikes"))
                .insert(SpikesUpgrade)
                .insert(SpriteBundle {
                    texture: spikes_handle,
                    transform: Transform {
                        translation: Vec3::new(0.0, 225.0, 0.0),
                        rotation: Quat::from_rotation_z(f32::to_radians(0.0)),
                        scale: Vec3::new(1.0, 1.0, 1.0),
                    },
                    ..default()
                });
        });
}
