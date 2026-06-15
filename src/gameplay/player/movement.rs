use crate::gameplay::combat::{VehicleIntegrity, WreckStuns, WreckSurges};
use crate::gameplay::ctf::{flag_carrier_speed_multiplier, CtfFlag, CtfMatchResult};
use crate::gameplay::main::{BOUNDS, TIME_STEP};
use crate::gameplay::pickup::{NitroBoosts, SabotageEffects};
use crate::gameplay::player::car::{FrontLeftWheel, FrontRightWheel};
use crate::gameplay::player::Player;
use crate::gameplay::slipstream::{slipstream_speed_multiplier, LeadingCar};
use crate::gameplay::virtual_player::VirtualPlayer;
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;

type FilterFrontLeftWheel = (Without<Player>, Without<FrontRightWheel>);
type FilterFrontRightWheel = (Without<Player>, Without<FrontLeftWheel>);
/// Read-only filter for the other cars whose wake the human can draft. The
/// `Without` bounds make the query provably disjoint from the mutable `Transform`
/// access of the player and wheel queries, so the borrow checker is satisfied.
type OtherCarTransform = (
    With<VirtualPlayer>,
    Without<Player>,
    Without<FrontLeftWheel>,
    Without<FrontRightWheel>,
);

/// Optional per-match resources the player movement system reads, bundled into
/// one system parameter to keep the signature legible (mirrors the CTF systems).
type PlayerMovementContext<'w> = (
    Option<Res<'w, NitroBoosts>>,
    Option<Res<'w, VehicleIntegrity>>,
    Option<Res<'w, WreckStuns>>,
    Option<Res<'w, WreckSurges>>,
    Option<Res<'w, SabotageEffects>>,
    Option<Res<'w, CtfMatchResult>>,
);

/// Demonstrates applying rotation and movement based on keyboard input.
pub fn car_movement_system(
    keyboard_input: Res<Input<KeyCode>>,
    context: PlayerMovementContext,
    mut query: Query<(Entity, &Player, &mut Transform)>,
    flag_query: Query<&CtfFlag>,
    other_car_query: Query<&Transform, OtherCarTransform>,
    mut front_left_wheel_query: Query<(&FrontLeftWheel, &mut Transform), FilterFrontLeftWheel>,
    mut front_right_wheel_query: Query<(&FrontRightWheel, &mut Transform), FilterFrontRightWheel>,
) {
    let (nitro_boosts, integrity, wreck_stuns, wreck_surges, sabotage_effects, match_result) =
        context;
    if match_result
        .as_ref()
        .is_some_and(|result| result.winner.is_some())
    {
        return;
    }

    let Ok((player_entity, player, mut transform)) = query.get_single_mut() else {
        return;
    };
    let Ok((_, mut front_left_wheel_transform)) = front_left_wheel_query.get_single_mut() else {
        return;
    };
    let Ok((_, mut front_right_wheel_transform)) = front_right_wheel_query.get_single_mut() else {
        return;
    };

    // Acceleration
    let nitro_multiplier = nitro_boosts
        .as_ref()
        .map_or(1.0, |boosts| boosts.player_multiplier());
    let integrity_multiplier = integrity
        .as_ref()
        .map_or(1.0, |integrity| integrity.player_multiplier());
    let stun_multiplier = wreck_stuns
        .as_ref()
        .map_or(1.0, |stuns| stuns.player_multiplier());
    let surge_multiplier = wreck_surges
        .as_ref()
        .map_or(1.0, |surges| surges.player_multiplier());
    let sabotage_multiplier = sabotage_effects
        .as_ref()
        .map_or(1.0, |effects| effects.player_multiplier());
    let carrying_flag = flag_query
        .iter()
        .any(|flag| flag.holder == Some(player_entity));
    let carry_multiplier = flag_carrier_speed_multiplier(carrying_flag);
    // A flag carrier never drafts: the bulky flag spoils the tow, so the slipstream
    // can never speed a flag run home, mirroring the field in the drive system.
    let draft_multiplier = if carrying_flag {
        1.0
    } else {
        let leaders: Vec<LeadingCar> = other_car_query
            .iter()
            .map(|other| LeadingCar {
                position: other.translation.xy(),
                heading: (other.rotation * Vec3::Y).xy(),
            })
            .collect();
        slipstream_speed_multiplier(
            transform.translation.xy(),
            (transform.rotation * Vec3::Y).xy(),
            &leaders,
        )
    };
    let speed_multiplier = player.engine_max_speed_multiplier
        * nitro_multiplier
        * integrity_multiplier
        * stun_multiplier
        * surge_multiplier
        * sabotage_multiplier
        * carry_multiplier
        * draft_multiplier;
    let forward_max_speed = player.forward_max_speed_base * speed_multiplier;
    let backward_max_speed = player.backward_max_speed_base * speed_multiplier;

    // Turning
    let forward_turning_speed = forward_max_speed * player.wheels_turning_multiplier;
    let backward_turning_speed = backward_max_speed * player.wheels_turning_multiplier;

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
        movement_factor * 30.0 * steering_multiplier,
    ));
    front_left_wheel_transform.rotation = front_wheel_rotation;
    front_right_wheel_transform.rotation = front_wheel_rotation;

    // get the car's forward vector by applying the current rotation to the car's initial facing vector
    let movement_direction = transform.rotation * Vec3::Y;
    // get the distance the car will move based on direction, the car's movement speed and delta time
    let movement_distance = movement_factor * player.movement_speed * TIME_STEP;
    // create the change in translation using the new movement direction and distance
    let translation_delta = movement_direction * movement_distance;
    // update the car translation with our new translation delta
    transform.translation += translation_delta;

    // bound the car within the invisible level bounds
    let extents = Vec3::new(BOUNDS.x / 2.0, BOUNDS.y / 2.0, 0.0);
    transform.translation.x = transform.translation.x.clamp(-extents.x, extents.x);
    transform.translation.y = transform.translation.y.clamp(-extents.y, extents.y);
    transform.translation.z = 5.0;
}
