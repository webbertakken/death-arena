use crate::gameplay::player::Player;
use bevy::prelude::*;

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let player_handle = asset_server.load("textures/car1.png");

    commands
        .spawn_bundle(SpriteBundle {
            texture: player_handle,
            transform: Transform {
                translation: Vec3::new(0.0, 0.0, 5.0),
                rotation: Quat::from_rotation_z(f32::to_radians(0.0)),
                scale: Vec3::new(0.2, 0.2, 0.0),
            },
            ..default()
        })
        .insert(Player {
            movement_speed: 500.0,                  // metres per second
            rotation_speed: f32::to_radians(360.0), // degrees per second
        });
}
