use crate::gameplay::carry_fatigue::carry_fatigue_speed_multiplier;
use crate::gameplay::chase_resolve::chase_resolve_speed_multiplier;
use crate::gameplay::combat::{VehicleIntegrity, WreckStuns, WreckSurges};
use crate::gameplay::comeback::comeback_speed_multiplier;
use crate::gameplay::ctf::{
    flag_carrier_speed_multiplier, CaptureScore, CtfFlag, CtfMatchResult, FlagCarryTimers, FlagTeam,
};
use crate::gameplay::escort_resolve::escort_resolve_speed_multiplier;
use crate::gameplay::flag_escort::flag_escort_speed_multiplier;
use crate::gameplay::flag_rally::flag_rally_speed_multiplier;
use crate::gameplay::front_runner::front_runner_speed_multiplier;
use crate::gameplay::main::{BOUNDS, TIME_STEP};
use crate::gameplay::pickup::{NitroBoosts, SabotageEffects};
use crate::gameplay::player::car::{FrontLeftWheel, FrontRightWheel};
use crate::gameplay::player::Player;
use crate::gameplay::slipstream::{slipstream_speed_multiplier, LeadingCar};
use crate::gameplay::virtual_player::VirtualPlayer;
use crate::gameplay::wall_scrape::wall_scrape_speed_multiplier;
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
    Option<Res<'w, CaptureScore>>,
    Option<Res<'w, FlagCarryTimers>>,
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
    let (
        nitro_boosts,
        integrity,
        wreck_stuns,
        wreck_surges,
        sabotage_effects,
        match_result,
        captures,
        flag_carry_timers,
    ) = context;
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
    let effect_multiplier = player_effect_multiplier(
        nitro_boosts.as_deref(),
        integrity.as_deref(),
        wreck_stuns.as_deref(),
        wreck_surges.as_deref(),
        sabotage_effects.as_deref(),
    );
    let carried_flag = flag_query
        .iter()
        .find(|flag| flag.holder == Some(player_entity));
    let carrying_flag = carried_flag.is_some();
    let carry_multiplier = flag_carrier_speed_multiplier(carrying_flag);
    // A carrier tires the longer it clings to the flag, just like the field, so a
    // long hold scrubs pace on top of the flat carry tax.
    let carry_fatigue_multiplier =
        player_carry_fatigue_multiplier(carried_flag, flag_carry_timers.as_deref());
    let draft_multiplier = player_draft_multiplier(carrying_flag, &transform, &other_car_query);
    // A car grinding the arena boundary bleeds speed, just like the field.
    let wall_scrape_multiplier =
        wall_scrape_speed_multiplier(transform.translation.xy(), BOUNDS / 2.0);
    // A trailing team's chasers earn a small catch-up urge, just like the field.
    let comeback_multiplier = player_comeback_multiplier(captures.as_deref(), carrying_flag);
    // A leading team's flag runner carries the front-runner's burden, just like the
    // field, so its run home is weighed down the further its side leads.
    let front_runner_multiplier =
        player_front_runner_multiplier(captures.as_deref(), carrying_flag);
    // The four flag-in-flight feel levers, each mirroring the field: while the human's
    // own flag is out, the defensive flag-recovery rally and its time-ramped chase
    // resolve; while the human hauls the enemy flag home, the offensive flag escort and
    // its time-ramped escort resolve. Folded together so the system reads the whole
    // flag-pressure stack in one call.
    let flag_feel_multiplier =
        player_flag_feel_multiplier(&flag_query, carrying_flag, flag_carry_timers.as_deref());
    let speed_multiplier = player.engine_max_speed_multiplier
        * effect_multiplier
        * carry_multiplier
        * carry_fatigue_multiplier
        * draft_multiplier
        * wall_scrape_multiplier
        * comeback_multiplier
        * front_runner_multiplier
        * flag_feel_multiplier;
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

/// Combined timed-effect speed multiplier the player team carries this frame: the
/// nitro boost, engine integrity, wreck stun, wreck surge and engine sabotage
/// folded together. The player-side mirror of the field's `team_movement_multiplier`,
/// so the human reads the same stack of timed effects the AI does; an absent
/// resource (no match in progress) contributes a neutral `1.0`.
fn player_effect_multiplier(
    nitro_boosts: Option<&NitroBoosts>,
    integrity: Option<&VehicleIntegrity>,
    wreck_stuns: Option<&WreckStuns>,
    wreck_surges: Option<&WreckSurges>,
    sabotage_effects: Option<&SabotageEffects>,
) -> f32 {
    let nitro = nitro_boosts.map_or(1.0, NitroBoosts::player_multiplier);
    let integrity = integrity.map_or(1.0, |integrity| integrity.player_multiplier());
    let stun = wreck_stuns.map_or(1.0, |stuns| stuns.player_multiplier());
    let surge = wreck_surges.map_or(1.0, |surges| surges.player_multiplier());
    let sabotage = sabotage_effects.map_or(1.0, SabotageEffects::player_multiplier);
    nitro * integrity * stun * surge * sabotage
}

/// Slipstream tow the human earns from the cars ahead this frame, or `1.0` when it
/// is carrying a flag (the bulky flag spoils the tow, so the slipstream can never
/// speed a flag run home, mirroring the field in the drive system). Leaders are
/// every other car's tail line.
fn player_draft_multiplier(
    carrying_flag: bool,
    transform: &Transform,
    other_car_query: &Query<&Transform, OtherCarTransform>,
) -> f32 {
    if carrying_flag {
        return 1.0;
    }
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
}

/// Fatigue the human carrier earns from clinging to its stolen flag, or `1.0` when
/// it carries nothing or no match is in progress. Reads the carried flag's
/// continuous-carry frame count, mirroring the field's
/// [`crate::gameplay::virtual_player`] drive, so the human tires on the identical
/// terms: a long hold scrubs pace on top of the flat carry tax.
fn player_carry_fatigue_multiplier(
    carried_flag: Option<&CtfFlag>,
    carry_timers: Option<&FlagCarryTimers>,
) -> f32 {
    match (carried_flag, carry_timers) {
        (Some(flag), Some(timers)) => carry_fatigue_speed_multiplier(timers.frames_for(flag.team)),
        _ => 1.0,
    }
}

/// Catch-up urge the human earns while its side trails on captures, or `1.0` with
/// no match in progress. The human is the player side, so its deficit is the
/// opponents' capture lead; a flag carrier earns none, mirroring the field, so the
/// catch-up never speeds a flag run home.
fn player_comeback_multiplier(captures: Option<&CaptureScore>, carrying_flag: bool) -> f32 {
    captures.map_or(1.0, |score| {
        comeback_speed_multiplier(score.player, score.opponents, carrying_flag)
    })
}

/// Front-runner's burden the human carrier carries while its side leads on
/// captures, or `1.0` with no match in progress. The human is the player side, so
/// its lead is the margin over the opponents' captures; only a flag carrier carries
/// the burden, mirroring the field, so a leading side's chasers stay unhindered and
/// the drag only ever weighs down a flag run home.
fn player_front_runner_multiplier(captures: Option<&CaptureScore>, carrying_flag: bool) -> f32 {
    captures.map_or(1.0, |score| {
        front_runner_speed_multiplier(score.player, score.opponents, carrying_flag)
    })
}

/// Flag-recovery rally the human earns while its own flag is in enemy hands, or
/// `1.0` when its flag is safe. The human is the player (blue) side, so its flag is
/// the blue flag, stolen exactly when that flag has a holder; only an empty-handed
/// car rallies, mirroring the field, so a double-steal carrier earns none and the
/// urge never speeds a flag run home.
fn player_flag_rally_multiplier(flag_query: &Query<&CtfFlag>, carrying_flag: bool) -> f32 {
    let own_flag_stolen = flag_query
        .iter()
        .any(|flag| flag.team == FlagTeam::Blue && flag.holder.is_some());
    flag_rally_speed_multiplier(own_flag_stolen, carrying_flag)
}

/// Escort urge the human earns while its side is hauling the enemy flag home, or
/// `1.0` when no blue car holds it. The human is the player (blue) side, so the enemy
/// flag is the red flag, held exactly when that flag has a holder (a flag is only ever
/// carried by the opposing side); only an empty-handed car escorts, mirroring the
/// field, so the carrier being shepherded earns none and the urge never speeds a flag
/// run home.
fn player_flag_escort_multiplier(flag_query: &Query<&CtfFlag>, carrying_flag: bool) -> f32 {
    let we_hold_enemy_flag = flag_query
        .iter()
        .any(|flag| flag.team == FlagTeam::Red && flag.holder.is_some());
    flag_escort_speed_multiplier(we_hold_enemy_flag, carrying_flag)
}

/// Chase resolve the human's empty-handed chasers build while its own flag is in
/// enemy hands, hardening the longer the steal drags on, or `1.0` when its flag is
/// safe (or no match is in progress, so the timers are absent). The human is the
/// player (blue) side, so its flag is the blue flag, stolen exactly when that flag
/// has a holder, and the resolve reads the blue flag's continuous-carry frame count;
/// only an empty-handed car digs in, mirroring the field, so a double-steal carrier
/// finds none and the urge never speeds a flag run home.
fn player_chase_resolve_multiplier(
    flag_query: &Query<&CtfFlag>,
    carrying_flag: bool,
    carry_timers: Option<&FlagCarryTimers>,
) -> f32 {
    let own_flag_stolen = flag_query
        .iter()
        .any(|flag| flag.team == FlagTeam::Blue && flag.holder.is_some());
    let carry_frames = carry_timers.map_or(0, |timers| timers.frames_for(FlagTeam::Blue));
    chase_resolve_speed_multiplier(own_flag_stolen, carrying_flag, carry_frames)
}

/// Escort resolve the human's empty-handed escorts build while its side hauls the
/// enemy flag home, hardening the longer the run drags on, or `1.0` when no blue car
/// holds it (or no match is in progress, so the timers are absent). The human is the
/// player (blue) side, so the enemy flag is the red flag, held exactly when that flag
/// has a holder, and the resolve reads the red flag's continuous-carry frame count
/// (the same count the carrier's own fatigue reads); only an empty-handed car digs in,
/// mirroring the field, so the shepherded carrier finds none and the urge never speeds
/// a flag run home.
fn player_escort_resolve_multiplier(
    flag_query: &Query<&CtfFlag>,
    carrying_flag: bool,
    carry_timers: Option<&FlagCarryTimers>,
) -> f32 {
    let we_hold_enemy_flag = flag_query
        .iter()
        .any(|flag| flag.team == FlagTeam::Red && flag.holder.is_some());
    let carry_frames = carry_timers.map_or(0, |timers| timers.frames_for(FlagTeam::Red));
    escort_resolve_speed_multiplier(we_hold_enemy_flag, carrying_flag, carry_frames)
}

/// The combined flag-in-flight feel multiplier the human earns from the four levers a
/// flag in motion arms, mirroring the field's `team_standing_multiplier` fold: the
/// defensive flag-recovery rally and its time-ramped chase resolve while the human's
/// own flag is in enemy hands, and the offensive flag escort and its time-ramped
/// escort resolve while the human hauls the enemy flag home. Each lever excludes the
/// carrier itself, so none ever speeds a flag run home; folded here so the movement
/// system reads the whole flag-pressure stack in a single call.
fn player_flag_feel_multiplier(
    flag_query: &Query<&CtfFlag>,
    carrying_flag: bool,
    carry_timers: Option<&FlagCarryTimers>,
) -> f32 {
    player_flag_rally_multiplier(flag_query, carrying_flag)
        * player_chase_resolve_multiplier(flag_query, carrying_flag, carry_timers)
        * player_flag_escort_multiplier(flag_query, carrying_flag)
        * player_escort_resolve_multiplier(flag_query, carrying_flag, carry_timers)
}
