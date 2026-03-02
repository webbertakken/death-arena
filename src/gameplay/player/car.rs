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
    let player_handle: Handle<Image> = asset_server.load("textures/car1/chassis1.png");
    let wheel_handle: Handle<Image> = asset_server.load("textures/car1/wheel1.png");
    let spikes_handle: Handle<Image> = asset_server.load("textures/car1/spikes1.png");

    // Wheel2 needs approx position of -132, 135 (front) -188 (rear), and scale 0.7

    commands
        .spawn((
            Name::new("Player"),
            Player {
                movement_speed: 500.0,                  // metres per second
                rotation_speed: f32::to_radians(360.0), // degrees per second
            },
            Sprite {
                image: player_handle,
                ..default()
            },
            Transform {
                translation: Vec3::new(-430.0, 0.0, 3.0),
                rotation: Quat::from_rotation_z(f32::to_radians(8.0)),
                scale: Vec3::new(0.2, 0.2, 0.2),
            },
            RigidBody::Dynamic,
            Velocity::zero(),
            Collider::ball(350.0),
            Restitution::coefficient(0.7),
            Ccd::enabled(),
            Damping {
                linear_damping: 12.5,
                angular_damping: 12.5,
            },
        ))
        .with_children(|parent| {
            // Front left wheel
            parent.spawn((
                Name::new("Wheel (FL)"),
                FrontLeftWheel,
                Sprite {
                    image: wheel_handle.clone(),
                    ..default()
                },
                Transform {
                    translation: Vec3::new(-117.0, 128.0, 0.0),
                    rotation: Quat::from_rotation_z(f32::to_radians(0.0)),
                    scale: Vec3::new(1.1, 1.1, 1.1),
                },
            ));

            // Front right wheel
            parent.spawn((
                Name::new("Wheel (FR)"),
                FrontRightWheel,
                Sprite {
                    image: wheel_handle.clone(),
                    ..default()
                },
                Transform {
                    translation: Vec3::new(114.0, 128.0, 0.0),
                    rotation: Quat::from_rotation_z(f32::to_radians(0.0)),
                    scale: Vec3::new(1.1, 1.1, 1.01),
                },
            ));

            // Rear left wheel
            parent.spawn((
                Name::new("Wheel (RL)"),
                RearLeftWheel,
                Sprite {
                    image: wheel_handle.clone(),
                    ..default()
                },
                Transform {
                    translation: Vec3::new(-115.0, -167.5, 0.0),
                    rotation: Quat::from_rotation_z(f32::to_radians(0.0)),
                    scale: Vec3::new(1.2, 1.2, 1.2),
                },
            ));

            // Rear right wheel
            parent.spawn((
                Name::new("Wheel (RR)"),
                RearRightWheel,
                Sprite {
                    image: wheel_handle.clone(),
                    ..default()
                },
                Transform {
                    translation: Vec3::new(115.0, -167.5, 0.0),
                    rotation: Quat::from_rotation_z(f32::to_radians(0.0)),
                    scale: Vec3::new(1.2, 1.2, 1.2),
                },
            ));

            // Spikes
            parent.spawn((
                Name::new("Spikes"),
                SpikesUpgrade,
                Sprite {
                    image: spikes_handle,
                    ..default()
                },
                Transform {
                    translation: Vec3::new(0.0, 225.0, 0.0),
                    rotation: Quat::from_rotation_z(f32::to_radians(0.0)),
                    scale: Vec3::new(1.0, 1.0, 1.0),
                },
            ));
        });
}
