use bevy::prelude::*;

use crate::{App, Input, KeyCode, Plugin, Query, Res, Transform, Vec3};
use bevy::{math::Vec3Swizzles, time::FixedTimestep};

use crate::gameplay::main::{BOUNDS, TIME_STEP};
use crate::gameplay::player;

mod movement;

#[derive(Component)]
pub struct Player {
    /// linear speed in meters per second
    movement_speed: f32,
    /// rotation speed in radians per second
    rotation_speed: f32,
}

#[derive(Default)]
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SpawnTimer(Timer::from_seconds(2.0, true)))
            .add_startup_system(setup)
            .add_system_set(
                SystemSet::new()
                    .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                    .with_system(movement::player_movement_system)
                    .with_system(snap_to_player_system)
                    .with_system(rotate_to_player_system),
            )
            .add_system(bevy::window::close_on_esc);
    }
}

pub struct SpawnTimer(Timer);

/// snap to player ship behavior
#[derive(Component)]
pub struct SnapToPlayer;

/// rotate to face player ship behavior
#[derive(Component)]
pub struct RotateToPlayer {
    /// rotation speed in radians per second
    rotation_speed: f32,
}

#[derive(Component)]
pub struct Name(String);

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let player_handle = asset_server.load("textures/car1.png");
    // let enemy_handle = asset_server.load("textures/bevy.png");
    let rock_handle = asset_server.load("textures/rock.png");

    let horizontal_margin = BOUNDS.x / 4.0;
    let vertical_margin = BOUNDS.y / 4.0;

    // player controlled ship
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

    // enemy that snaps to face the player spawns on the bottom and left
    commands
        .spawn_bundle(SpriteBundle {
            texture: rock_handle.clone(),
            transform: Transform::from_xyz(0.0 - horizontal_margin, 0.0, 2.0),
            ..default()
        })
        .insert(SnapToPlayer);
    commands
        .spawn_bundle(SpriteBundle {
            texture: rock_handle.clone(),
            transform: Transform::from_xyz(0.0, 0.0 - vertical_margin, 2.0),
            ..default()
        })
        .insert(SnapToPlayer);

    // enemy that rotates to face the player enemy spawns on the top and right
    commands
        .spawn_bundle(SpriteBundle {
            texture: rock_handle.clone(),
            transform: Transform::from_xyz(0.0 + horizontal_margin, 0.0, 2.0),
            ..default()
        })
        .insert(RotateToPlayer {
            rotation_speed: f32::to_radians(45.0), // degrees per second
        });
    commands
        .spawn_bundle(SpriteBundle {
            texture: rock_handle,
            transform: Transform::from_xyz(0.0, 0.0 + vertical_margin, 2.0),
            ..default()
        })
        .insert(RotateToPlayer {
            rotation_speed: f32::to_radians(90.0), // degrees per second
        });
}

/// Demonstrates snapping the enemy ship to face the player ship immediately.
pub fn snap_to_player_system(
    mut query: Query<&mut Transform, (With<SnapToPlayer>, Without<Player>)>,
    player_query: Query<&Transform, With<Player>>,
) {
    let player_transform = player_query.single();
    // get the player translation in 2D
    let player_translation = player_transform.translation.xy();

    for mut enemy_transform in &mut query {
        // get the vector from the enemy ship to the player ship in 2D and normalize it.
        let to_player = (player_translation - enemy_transform.translation.xy()).normalize();

        // get the quaternion to rotate from the initial enemy facing direction to the direction
        // facing the player
        let rotate_to_player = Quat::from_rotation_arc(Vec3::Y, to_player.extend(0.));

        // rotate the enemy to face the player
        enemy_transform.rotation = rotate_to_player;
    }
}

/// Demonstrates rotating an enemy ship to face the player ship at a given rotation speed.
///
/// This method uses the vector dot product to determine if the enemy is facing the player and
/// if not, which way to rotate to face the player. The dot product on two unit length vectors
/// will return a value between -1.0 and +1.0 which tells us the following about the two vectors:
///
/// * If the result is 1.0 the vectors are pointing in the same direction, the angle between them
///   is 0 degrees.
/// * If the result is 0.0 the vectors are perpendicular, the angle between them is 90 degrees.
/// * If the result is -1.0 the vectors are parallel but pointing in opposite directions, the angle
///   between them is 180 degrees.
/// * If the result is positive the vectors are pointing in roughly the same direction, the angle
///   between them is greater than 0 and less than 90 degrees.
/// * If the result is negative the vectors are pointing in roughly opposite directions, the angle
///   between them is greater than 90 and less than 180 degrees.
///
/// It is possible to get the angle by taking the arc cosine (`acos`) of the dot product. It is
/// often unnecessary to do this though. Beware than `acos` will return `NaN` if the input is less
/// than -1.0 or greater than 1.0. This can happen even when working with unit vectors due to
/// floating point precision loss, so it pays to clamp your dot product value before calling
/// `acos`.
pub fn rotate_to_player_system(
    mut query: Query<(&RotateToPlayer, &mut Transform), Without<Player>>,
    player_query: Query<&Transform, With<Player>>,
) {
    let player_transform = player_query.single();
    // get the player translation in 2D
    let player_translation = player_transform.translation.xy();

    for (config, mut enemy_transform) in &mut query {
        // get the enemy ship forward vector in 2D (already unit length)
        let enemy_forward = (enemy_transform.rotation * Vec3::Y).xy();

        // get the vector from the enemy ship to the player ship in 2D and normalize it.
        let to_player = (player_translation - enemy_transform.translation.xy()).normalize();

        // get the dot product between the enemy forward vector and the direction to the player.
        let forward_dot_player = enemy_forward.dot(to_player);

        // if the dot product is approximately 1.0 then the enemy is already facing the player and
        // we can early out.
        if (forward_dot_player - 1.0).abs() < f32::EPSILON {
            continue;
        }

        // get the right vector of the enemy ship in 2D (already unit length)
        let enemy_right = (enemy_transform.rotation * Vec3::X).xy();

        // get the dot product of the enemy right vector and the direction to the player ship.
        // if the dot product is negative them we need to rotate counter clockwise, if it is
        // positive we need to rotate clockwise. Note that `copysign` will still return 1.0 if the
        // dot product is 0.0 (because the player is directly behind the enemy, so perpendicular
        // with the right vector).
        let right_dot_player = enemy_right.dot(to_player);

        // determine the sign of rotation from the right dot player. We need to negate the sign
        // here as the 2D bevy co-ordinate system rotates around +Z, which is pointing out of the
        // screen. Due to the right hand rule, positive rotation around +Z is counter clockwise and
        // negative is clockwise.
        let rotation_sign = -f32::copysign(1.0, right_dot_player);

        // limit rotation so we don't overshoot the target. We need to convert our dot product to
        // an angle here so we can get an angle of rotation to clamp against.
        let max_angle = forward_dot_player.clamp(-1.0, 1.0).acos(); // clamp acos for safety

        // calculate angle of rotation with limit
        let rotation_angle = rotation_sign * (config.rotation_speed * TIME_STEP).min(max_angle);

        // rotate the enemy to face the player
        enemy_transform.rotate_z(rotation_angle);
    }
}
