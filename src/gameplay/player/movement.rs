use crate::gameplay::main::BOUNDS;
use crate::gameplay::main::TIME_STEP;
use crate::gameplay::player::car::{FrontLeftWheel, FrontRightWheel};
use crate::gameplay::player::Player;
use crate::{App, Input, KeyCode, Plugin, Query, Res, Transform, Vec3};
use bevy::prelude::*;
use bevy::{math::Vec3Swizzles, time::FixedTimestep};

type FilterFrontLeftWheel = (Without<Player>, Without<FrontRightWheel>);
type FilterFrontRightWheel = (Without<Player>, Without<FrontLeftWheel>);

/// Demonstrates applying rotation and movement based on keyboard input.
pub fn car_movement_system(
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<(&Player, &mut Transform)>,
    mut front_left_wheel_query: Query<(&FrontLeftWheel, &mut Transform), FilterFrontLeftWheel>,
    mut front_right_wheel_query: Query<(&FrontRightWheel, &mut Transform), FilterFrontRightWheel>,
) {
    let (player, mut transform) = query.single_mut();
    let (front_left_wheel, mut front_left_wheel_transform) = front_left_wheel_query.single_mut();
    let (front_right_wheel, mut front_right_wheel_transform) = front_right_wheel_query.single_mut();

    // Acceleration - 0.5 (slowest car) to 1.0 (fastest car)
    let engine_max_speed_multiplier = 0.5;
    let forward_max_speed = 0.825 * engine_max_speed_multiplier;
    let backward_max_speed = 0.25 * engine_max_speed_multiplier;

    // Turning
    let wheels_turning_multiplier = 0.91;
    let forward_turning_speed = forward_max_speed * wheels_turning_multiplier;
    let backward_turning_speed = backward_max_speed * wheels_turning_multiplier;

    let mut rotation_factor = 0.0;
    let mut movement_factor = 0.0;

    let mut steer_reverse = false;

    if keyboard_input.any_pressed([KeyCode::Up, KeyCode::W]) {
        movement_factor += forward_max_speed;
        if keyboard_input.any_pressed([KeyCode::Left, KeyCode::A]) {
            rotation_factor += forward_turning_speed;
        }

        if keyboard_input.any_pressed([KeyCode::Right, KeyCode::D]) {
            rotation_factor -= forward_turning_speed;
        }
    } else if keyboard_input.any_pressed([KeyCode::Down, KeyCode::S]) {
        movement_factor -= backward_max_speed;
        steer_reverse = true;

        if keyboard_input.any_pressed([KeyCode::Left, KeyCode::A]) {
            rotation_factor -= backward_turning_speed;
        }

        if keyboard_input.any_pressed([KeyCode::Right, KeyCode::D]) {
            rotation_factor += backward_turning_speed;
        }
    }

    // update the car rotation around the Z axis (perpendicular to the 2D plane of the screen)
    transform.rotate_z(rotation_factor * player.rotation_speed * TIME_STEP);

    // Wheels just keep on turning
    let steering_multiplier = if steer_reverse { -1.0 } else { 1.0 };
    let front_wheel_rotation = Quat::from_rotation_z(f32::to_radians(
        rotation_factor * 30.0 * steering_multiplier,
    ));
    front_left_wheel_transform.rotation = front_wheel_rotation;
    front_right_wheel_transform.rotation = front_wheel_rotation;

    // get the car's forward vector by applying the current rotation to the cars initial facing vector
    let movement_direction = transform.rotation * Vec3::Y;
    // get the distance the car will move based on direction, the car's movement speed and delta time
    let movement_distance = movement_factor * player.movement_speed * TIME_STEP;
    // create the change in translation using the new movement direction and distance
    let translation_delta = movement_direction * movement_distance;
    // update the car translation with our new translation delta
    transform.translation += translation_delta;

    // bound the car within the invisible level bounds
    let extents = Vec3::from((BOUNDS / 2.0, 0.0));
    transform.translation = transform.translation.min(extents).max(-extents);
    transform.translation.z = 5.0;
}
