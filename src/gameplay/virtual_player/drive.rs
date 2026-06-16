use crate::gameplay::carry_fatigue::carry_fatigue_speed_multiplier;
use crate::gameplay::combat::{VehicleIntegrity, WreckStuns, WreckSurges, RAM_RADIUS};
use crate::gameplay::comeback::comeback_speed_multiplier;
use crate::gameplay::ctf::{
    flag_carrier_speed_multiplier, CaptureScore, CtfFlag, CtfMatchResult, FlagCarryTimers,
    FlagTeam, MatchClock,
};
use crate::gameplay::front_runner::front_runner_speed_multiplier;
use crate::gameplay::main::{BOUNDS, TIME_STEP};
use crate::gameplay::pickup::{NitroBoosts, Pickup, PickupKind, SabotageEffects};
use crate::gameplay::player::Player;
use crate::gameplay::slipstream::{draft_seeking_aim, slipstream_speed_multiplier, LeadingCar};
use crate::gameplay::virtual_player::ai::{
    choose_capture_the_flag_target, choose_driving_target, compare_positions, compute_steering,
    draft_seek_cone, finish_off_aim, finish_off_car, lead_defence_car, next_waypoint,
    pincer_partner, pit_retreat_car, pit_retreat_home_run_aim, AiTeam, DrivingChoices,
    DrivingTarget, FinishOffCandidate, FlagTarget, LeadDefenceCandidate, PickupTarget,
    PitRetreatCandidate, SteeringIntent, ThreatTarget, MIN_THROTTLE,
};
use crate::gameplay::virtual_player::discipline::{
    closing_time_pickup_discipline, lead_protection, ClosingTimePickupDiscipline,
};
use crate::gameplay::virtual_player::VirtualPlayer;
use crate::gameplay::wall_scrape::wall_scrape_speed_multiplier;
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;

/// Distance (world units) at which a virtual player considers a waypoint
/// reached and advances to the next one.
const WAYPOINT_ARRIVE_RADIUS: f32 = 80.0;
/// Baseline arrive radius for a target a car means to *ram*: chasing the human
/// player or hunting a reeling enemy down, as committed by a driver cornering on
/// the neutral [`MIN_THROTTLE`] floor.
///
/// The wide [`WAYPOINT_ARRIVE_RADIUS`] sits outside true ram range
/// ([`RAM_RADIUS`]), so a car that idles the instant it reaches it coasts to a
/// halt short of contact and the chase stutters. Kept well inside ram range
/// instead, so a hunter drives all the way through to a hard hit and shoves its
/// victim, the aggressive Death Rally run-down rather than a polite stop. Each
/// driver then flexes this baseline by its cornering commitment (see
/// [`pursuit_arrive_radius`]).
const PURSUIT_ARRIVE_RADIUS: f32 = 30.0;
/// How far a driver's run-down depth shifts per unit of cornering commitment away
/// from the neutral [`MIN_THROTTLE`] baseline.
///
/// A reckless driver (a higher [`VirtualPlayer::corner_throttle`]) commits a
/// *deeper* run-down: a tighter arrive radius, so it noses further through to
/// contact before it idles. A disciplined driver eases off a touch sooner. The
/// run-down depth is thus the chase mirror of the same commitment axis that sets
/// how hard a driver stays on the gas through a corner, so a keen hunter presses
/// a kill as relentlessly as it barrels a bend.
const PURSUIT_COMMITMENT_DEPTH_GAIN: f32 = 40.0;
/// Tightest run-down the keenest driver ever commits to.
const PURSUIT_ARRIVE_RADIUS_MIN: f32 = 18.0;
/// Shallowest run-down the most disciplined driver eases off to.
const PURSUIT_ARRIVE_RADIUS_MAX: f32 = 42.0;
/// A ram run-down must commit deeper than a waypoint stop across the whole
/// commitment band, enforced at compile time.
const _: () = assert!(PURSUIT_ARRIVE_RADIUS_MAX < WAYPOINT_ARRIVE_RADIUS);
/// A ram run-down must close well inside true ram range even at its shallowest,
/// enforced at compile time, so the car is genuinely trading paint before it ever
/// idles.
const _: () = assert!(PURSUIT_ARRIVE_RADIUS_MAX < RAM_RADIUS);
/// The keenest run-down must still be a positive distance, enforced at compile
/// time, so even a relentless hunter idles on contact rather than driving through
/// its victim forever.
const _: () = assert!(PURSUIT_ARRIVE_RADIUS_MIN > 0.0);
/// The neutral baseline must sit inside the commitment band, enforced at compile
/// time, so a reckless rival flexes it deeper and a disciplined one shallower.
const _: () = assert!(
    PURSUIT_ARRIVE_RADIUS_MIN < PURSUIT_ARRIVE_RADIUS
        && PURSUIT_ARRIVE_RADIUS < PURSUIT_ARRIVE_RADIUS_MAX
);
/// The human player's pickup-scavenging reach, used when deciding whether the
/// human has a better claim on a bag than a blue teammate. Each virtual player
/// scavenges at its own personality-driven [`VirtualPlayer::pickup_pursuit_radius`];
/// the human has no driving personality, so it mirrors the all-rounder baseline.
const PLAYER_PICKUP_PURSUIT_RADIUS: f32 = 450.0;
const HOME_LANE_GUARD_DISTANCE: f32 = 220.0;
const MIDFIELD_LANE_GUARD_FACTOR: f32 = 0.5;
const TEAMMATE_SPACING_RADIUS: f32 = 90.0;

type HumanPlayerTransform = (With<Player>, Without<VirtualPlayer>);

/// The human player's tracked velocity, so virtual players can lead and intercept
/// the human exactly as they already lead virtual enemies.
///
/// The one car the AI could never lead was the human: the threat list and the kill
/// press both fed it a zero velocity, so every defensive body-block met the spot it
/// had already left and every run-down tail-chased it. This resource closes that
/// gap. [`track_player_velocity_system`] refreshes it each fixed frame from the
/// human's movement, and the drive system reads it into the human's threat entry so
/// the existing lead machinery (the ring-breach and interception solves in
/// [`crate::gameplay::virtual_player::ai`]) finally applies to the human too: a red
/// defender cuts off a juking human thief, and a kill press heads a fleeing human
/// carrier off at the pass.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct PlayerVelocity {
    /// The human's position last fixed frame, or `None` before the first sample (or
    /// while the human is absent), so the first frame estimates a neutral zero.
    pub previous_position: Option<Vec2>,
    /// The human's velocity estimated across the most recent fixed frame.
    pub velocity: Vec2,
}

/// Estimates the human player's instantaneous velocity from its movement across a
/// single fixed frame: the position delta over [`TIME_STEP`].
///
/// The first frame (or any frame the human was absent) has no previous position to
/// difference against, so the estimate is a neutral zero, exactly the body-block
/// fallback the lead logic already handles.
#[must_use]
fn player_velocity_estimate(previous_position: Option<Vec2>, current_position: Vec2) -> Vec2 {
    previous_position.map_or(Vec2::ZERO, |previous| {
        (current_position - previous) / TIME_STEP
    })
}

/// The human's tracked velocity, or a neutral zero when the tracker is absent (e.g.
/// a combat-light test app), which the lead logic resolves to a plain body-block.
#[must_use]
fn human_velocity(player_velocity: Option<&PlayerVelocity>) -> Vec2 {
    player_velocity.map_or(Vec2::ZERO, |tracker| tracker.velocity)
}

/// Refreshes [`PlayerVelocity`] from the human player's movement each fixed frame.
///
/// Runs before [`virtual_player_drive_system`] so the drive reads a velocity
/// sampled this very frame. With no human present the estimate resets to a neutral
/// zero and the previous position is cleared, so a human that despawns and respawns
/// never registers a phantom teleport velocity on its return.
pub fn track_player_velocity_system(
    mut tracker: ResMut<PlayerVelocity>,
    player_query: Query<&Transform, HumanPlayerTransform>,
) {
    let Ok(transform) = player_query.get_single() else {
        tracker.previous_position = None;
        tracker.velocity = Vec2::ZERO;
        return;
    };

    let position = transform.translation.xy();
    tracker.velocity = player_velocity_estimate(tracker.previous_position, position);
    tracker.previous_position = Some(position);
}

/// Optional per-match resources the drive system reads, bundled into one system
/// parameter to keep the signature under clippy's argument limit (mirrors the
/// player movement system's [`crate::gameplay::player`] context tuple).
type VirtualPlayerDriveContext<'w> = (
    Option<Res<'w, NitroBoosts>>,
    Option<Res<'w, VehicleIntegrity>>,
    Option<Res<'w, WreckStuns>>,
    Option<Res<'w, WreckSurges>>,
    Option<Res<'w, CtfMatchResult>>,
    Option<Res<'w, CaptureScore>>,
    Option<Res<'w, MatchClock>>,
);

/// Drives every [`VirtualPlayer`] towards its current patrol waypoint, applying
/// the same movement/rotation model the human player uses.
///
/// `feel_effects` bundles the two resources that ride outside the
/// [`VirtualPlayerDriveContext`] tuple: the engine sabotage and the carry-fatigue
/// timers. Both stay out of the main tuple so its destructure keeps under clippy's
/// line gate, and travel together in one parameter to keep the signature inside
/// clippy's argument limit.
pub fn virtual_player_drive_system(
    mut query: Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
    player_query: Query<(Entity, &Transform), HumanPlayerTransform>,
    pickup_query: Query<(&Transform, &Pickup), Without<VirtualPlayer>>,
    flag_query: Query<(&Transform, &CtfFlag), Without<VirtualPlayer>>,
    feel_effects: (Option<Res<SabotageEffects>>, Option<Res<FlagCarryTimers>>),
    player_velocity: Option<Res<PlayerVelocity>>,
    context: VirtualPlayerDriveContext,
) {
    let (nitro_boosts, integrity, wreck_stuns, wreck_surges, match_result, captures, match_clock) =
        context;
    let (sabotage_effects, carry_timers) = feel_effects;
    if match_result
        .as_ref()
        .is_some_and(|result| result.winner.is_some())
    {
        return;
    }
    let captures = captures.as_deref().copied().unwrap_or_default();
    let carry_timers = carry_timers.as_deref().copied().unwrap_or_default();
    // Closing-time discipline: in the final stretch of a round every team leaves
    // cash bags on the track, whether it is committing to attack (not ahead) or
    // protecting a lead (ahead). Only a real edge is worth breaking off for.
    let discipline = closing_time_pickup_discipline(match_clock.as_deref(), captures);

    let player = player_query
        .get_single()
        .ok()
        .map(|(entity, transform)| (entity, transform.translation.xy()));
    let player_position = player.map(|(_, position)| position);
    // The human's tracked velocity feeds the threat list (and the kill press), so
    // the human is led exactly as a virtual car is.
    let tracked_velocity = human_velocity(player_velocity.as_deref());
    let threats = threat_targets(&query, player_position, tracked_velocity);
    let visible_pickups = arena_pickups(&pickup_query);
    let holder_positions = holder_positions(&query, player);
    let flags = flag_targets(&flag_query, &holder_positions);
    let flag_stolen = flag_stolen_state(&flags);
    let assigned_ctf_targets = assigned_ctf_targets(&query, &flags, &threats);
    let overlays = overlay_targets(
        &query,
        &flags,
        &threats,
        integrity.as_deref(),
        captures,
        match_clock.as_deref(),
    );
    let teammate_positions = virtual_player_positions(&query);
    // Every car's tail line (its wake) this frame, so a driver can catch a leader's slipstream.
    let wakes = car_draft_lines(&query, &player_query);
    let claimed_pickups = claimed_pickups_for_virtual_players(
        &query,
        &assigned_ctf_targets,
        &visible_pickups,
        integrity.as_deref(),
        player_position,
        discipline,
        flag_stolen,
    );

    for (entity, mut ai, mut transform) in &mut query {
        let position = transform.translation.xy();
        let forward = (transform.rotation * Vec3::Y).xy();
        let ctf_target = resolve_overlay_target(entity, &overlays, &assigned_ctf_targets);
        let entity_pickups = pickups_claimed_by(entity, &claimed_pickups);
        let Some(target) = choose_driving_target(
            position,
            DrivingChoices {
                waypoints: &ai.waypoints,
                current_waypoint: ai.current_waypoint,
                ctf_target,
                pickups: &entity_pickups,
                pickup_pursuit_radius: ai.pickup_pursuit_radius,
                player_position: player_position_for_team(ai.team, player_position),
                player_pursuit_radius: ai.player_pursuit_radius,
                closing_time_discipline: discipline.for_team(ai.team),
            },
        ) else {
            continue;
        };

        let spacing_target = matches!(target, DrivingTarget::PatrolWaypoint(_))
            .then(|| teammate_spacing_target(entity, ai.team, position, &teammate_positions))
            .flatten();
        let target_position = spacing_target.unwrap_or_else(|| target.position());
        let arrive_radius = arrive_radius_for_target(target, ai.corner_throttle);
        let draft_cone = car_draft_cone(entity, ai.team, ai.pickup_pursuit_radius, &flags);
        let Some(intent) = steering_for_car(
            position,
            forward,
            target_position,
            arrive_radius,
            ai.corner_throttle,
            draft_cone,
            &draft_leaders(entity, &wakes),
        ) else {
            if matches!(target, DrivingTarget::PatrolWaypoint(_)) {
                ai.current_waypoint = next_waypoint(ai.current_waypoint, ai.waypoints.len());
            }
            continue;
        };

        // Rotation: positive steer turns left (counter-clockwise).
        transform.rotate_z(intent.steer * ai.rotation_speed * TIME_STEP);

        // Translation along the (rotated) forward vector.
        let movement_direction = transform.rotation * Vec3::Y;
        let car_speed = car_speed_multiplier(entity, ai.team, position, forward, &flags, &wakes)
            * team_standing_multiplier(ai.team, captures, entity, &flags, carry_timers);
        let team_effects = team_movement_multiplier(
            ai.team,
            nitro_boosts.as_deref(),
            integrity.as_deref(),
            wreck_stuns.as_deref(),
            wreck_surges.as_deref(),
            sabotage_effects.as_deref(),
        );
        let movement_distance =
            intent.throttle * ai.movement_speed * team_effects * car_speed * TIME_STEP;
        transform.translation += movement_direction * movement_distance;

        // Keep opponents inside the arena, just like the player.
        confine_to_arena(&mut transform.translation);
    }
}

/// Clamps a car's translation back inside the arena bounds and pins it to the
/// opponent render layer, mirroring the bound the human player is held to.
fn confine_to_arena(translation: &mut Vec3) {
    let extents = Vec3::new(BOUNDS.x / 2.0, BOUNDS.y / 2.0, 0.0);
    translation.x = translation.x.clamp(-extents.x, extents.x);
    translation.y = translation.y.clamp(-extents.y, extents.y);
    translation.z = 4.0;
}

/// Gathers every per-entity driving overlay for the frame, in precedence order.
///
/// Survival comes first (a battered team's [`pit_retreat_targets`]), then offence
/// (a healthier team's [`finish_off_targets`]), then closing-time lead defence
/// (a leading team's [`lead_defence_targets`]). [`resolve_overlay_target`] takes
/// the first entry that claims a given entity, so an earlier overlay wins when
/// several would lay claim to the same car. Each overlay overrides the CTF role
/// the car would otherwise take.
fn overlay_targets(
    query: &Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
    flags: &[FlagTarget],
    threats: &[ThreatTarget],
    integrity: Option<&VehicleIntegrity>,
    captures: CaptureScore,
    clock: Option<&MatchClock>,
) -> Vec<(Entity, DrivingTarget)> {
    pit_retreat_targets(query, flags, threats, integrity, captures, clock)
        .into_iter()
        .chain(finish_off_targets(
            query, flags, threats, integrity, captures, clock,
        ))
        .chain(lead_defence_targets(query, flags, clock, captures))
        .collect()
}

/// Resolves the highest-priority driving target an entity has this frame.
///
/// The [`overlay_targets`] are listed in precedence order, so the first overlay
/// that claims this entity wins; the assigned CTF role is used only when no
/// overlay does.
fn resolve_overlay_target(
    entity: Entity,
    overlays: &[(Entity, DrivingTarget)],
    assigned_ctf_targets: &[(Entity, Option<DrivingTarget>)],
) -> Option<DrivingTarget> {
    overlays
        .iter()
        .find(|(candidate, _)| *candidate == entity)
        .map(|(_, target)| *target)
        .or_else(|| {
            assigned_ctf_targets
                .iter()
                .find(|(assigned_entity, _)| *assigned_entity == entity)
                .and_then(|(_, target)| *target)
        })
}

fn virtual_player_positions(
    query: &Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
) -> Vec<(Entity, AiTeam, Vec2)> {
    query
        .iter()
        .map(|(entity, virtual_player, transform)| {
            (entity, virtual_player.team, transform.translation.xy())
        })
        .collect()
}

/// Every car's tail line (position and heading) this frame: the field plus the
/// human, so any driver can catch the slipstream of a car ahead of it regardless
/// of team. Gathered immutably before the drive loop, mirroring
/// [`virtual_player_positions`].
fn car_draft_lines(
    query: &Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
    player_query: &Query<(Entity, &Transform), HumanPlayerTransform>,
) -> Vec<(Entity, Vec2, Vec2)> {
    let mut lines: Vec<(Entity, Vec2, Vec2)> = query
        .iter()
        .map(|(entity, _, transform)| {
            (
                entity,
                transform.translation.xy(),
                (transform.rotation * Vec3::Y).xy(),
            )
        })
        .collect();
    if let Ok((entity, transform)) = player_query.get_single() {
        lines.push((
            entity,
            transform.translation.xy(),
            (transform.rotation * Vec3::Y).xy(),
        ));
    }
    lines
}

/// Combined per-car speed multiplier beyond the team's shared timed effects: the
/// flag-carry tax, any slipstream tow, and the wall scrape a car bleeds for
/// grinding the arena boundary. The carrier state is computed once; a flag carrier
/// pays the carry tax and earns no tow, so the slipstream can never speed a flag
/// run home, and a car jammed against a wall scrubs speed exactly as the human does.
fn car_speed_multiplier(
    entity: Entity,
    team: AiTeam,
    position: Vec2,
    heading: Vec2,
    flags: &[FlagTarget],
    draft_lines: &[(Entity, Vec2, Vec2)],
) -> f32 {
    let is_carrier = carries_enemy_flag(entity, team, flags);
    flag_carrier_speed_multiplier(is_carrier)
        * car_draft_multiplier(is_carrier, entity, position, heading, draft_lines)
        * wall_scrape_speed_multiplier(position, BOUNDS / 2.0)
}

/// The three per-car factors keyed on the capture standing and how the car sits in
/// the round, folded together: the catch-up urge a trailing side earns, the fatigue
/// a carrier sheds for clinging to the flag, and the front-runner's burden a leading
/// side's carrier carries. Read as a separate factor beside the per-car
/// [`car_speed_multiplier`], mirroring the human.
///
/// The catch-up urges a trailing team's *chasers* (never its flag runner, so it
/// can never speed a flag run home, mirroring the slipstream); a car level or
/// ahead earns none. The other two bite the *carrier* alone: fatigue scrubs more
/// pace the longer it holds the enemy flag, and the front-runner's burden (see
/// [`front_runner_speed_multiplier`]) drags it back the further its team leads on
/// captures, the anti-snowball mirror of the trailing side's catch-up. So a chaser
/// is only ever urged on and a carrier only ever weighed down, never both: the
/// catch-up and the carrier penalties move mutually exclusive cars, and a leading,
/// tired carrier simply stacks the two carrier drags. Every one of the three can
/// only help the chase, never speed a flag run home.
fn team_standing_multiplier(
    team: AiTeam,
    captures: CaptureScore,
    entity: Entity,
    flags: &[FlagTarget],
    carry_timers: FlagCarryTimers,
) -> f32 {
    let is_carrier = carries_enemy_flag(entity, team, flags);
    let (own, enemy) = captures.standings(FlagTeam::from(team));
    let comeback = comeback_speed_multiplier(own, enemy, is_carrier);
    let fatigue = if is_carrier {
        carry_fatigue_speed_multiplier(carry_timers.frames_for(FlagTeam::from(team).enemy()))
    } else {
        1.0
    };
    let front_runner = front_runner_speed_multiplier(own, enemy, is_carrier);
    comeback * fatigue * front_runner
}

/// Slipstream tow `entity` earns from the cars ahead of it this frame, or `1.0`
/// when it is a flag carrier (the flag spoils the draft, keeping the tow off a
/// flag run home). Leaders are every car's tail line bar its own.
fn car_draft_multiplier(
    is_carrier: bool,
    entity: Entity,
    position: Vec2,
    heading: Vec2,
    draft_lines: &[(Entity, Vec2, Vec2)],
) -> f32 {
    if is_carrier {
        return 1.0;
    }
    slipstream_speed_multiplier(position, heading, &draft_leaders(entity, draft_lines))
}

/// The pickups claimed for `entity` this frame, pulled from the shared per-team
/// claim list ([`claimed_pickups_for_virtual_players`]).
fn pickups_claimed_by(entity: Entity, claimed: &[(Entity, PickupTarget)]) -> Vec<PickupTarget> {
    claimed
        .iter()
        .filter_map(|(assigned, pickup)| (*assigned == entity).then_some(*pickup))
        .collect()
}

/// Every car's wake bar `entity`'s own: the leaders a driver might draft, read both
/// as the passive tow it earns ([`car_draft_multiplier`]) and the wake it actively
/// steers to tuck into ([`draft_seeking_aim`]).
fn draft_leaders(entity: Entity, draft_lines: &[(Entity, Vec2, Vec2)]) -> Vec<LeadingCar> {
    draft_lines
        .iter()
        .filter(|(candidate, _, _)| *candidate != entity)
        .map(|(_, leader_position, leader_heading)| LeadingCar {
            position: *leader_position,
            heading: *leader_heading,
        })
        .collect()
}

/// The greed-scaled active-drafting cone a car drives with this frame, or `None` for
/// a flag carrier (which earns no tow and commits straight to its target).
///
/// Folds the carrier check and the personality-greed lookup ([`draft_seek_cone`]) so
/// the drive loop reads a car's drafting keenness in a single call: a greedier driver
/// tucks into a wake even when it pulls further off its objective line, the off-line
/// mirror of the same greed axis that widens its pickup detours.
fn car_draft_cone(
    entity: Entity,
    team: AiTeam,
    pickup_pursuit_radius: f32,
    flags: &[FlagTarget],
) -> Option<f32> {
    (!carries_enemy_flag(entity, team, flags)).then(|| draft_seek_cone(pickup_pursuit_radius))
}

/// The steering a car wants this frame toward `target_position`, with active
/// drafting folded in, or `None` when it has arrived and should idle (the caller
/// then advances a patrol waypoint and skips the car).
///
/// `draft_cone` carries both whether this car drafts and how keenly: `Some(cone)` for
/// a non-carrier (the greed-scaled deflection cone it tucks into a wake with, see
/// [`draft_seek_cone`]), or `None` for a flag carrier, which earns no tow and commits
/// straight to its target. When it drafts, the car steers toward the wake-seeking aim
/// ([`draft_seeking_aim`]) lying on its way, catching the tow as it goes; the
/// arrive/idle decision stays keyed on the real target, so seeking only ever
/// redirects an already-driving car and falls back to the straight line whenever
/// tucking in would stall it.
fn steering_for_car(
    position: Vec2,
    forward: Vec2,
    target_position: Vec2,
    arrive_radius: f32,
    corner_throttle: f32,
    draft_cone: Option<f32>,
    leaders: &[LeadingCar],
) -> Option<SteeringIntent> {
    let straight = compute_steering(
        position,
        forward,
        target_position,
        arrive_radius,
        corner_throttle,
    );
    if straight == SteeringIntent::IDLE {
        return None;
    }
    let Some(cone) = draft_cone else {
        return Some(straight);
    };
    let aim = draft_seeking_aim(position, target_position, leaders, cone);
    let drafted = compute_steering(position, forward, aim, arrive_radius, corner_throttle);
    Some(if drafted == SteeringIntent::IDLE {
        straight
    } else {
        drafted
    })
}

fn teammate_spacing_target(
    entity: Entity,
    team: AiTeam,
    position: Vec2,
    virtual_players: &[(Entity, AiTeam, Vec2)],
) -> Option<Vec2> {
    let spacing_radius_sq = TEAMMATE_SPACING_RADIUS * TEAMMATE_SPACING_RADIUS;
    virtual_players
        .iter()
        .copied()
        .filter(|(other_entity, other_team, _)| *other_entity != entity && *other_team == team)
        .filter_map(|(other_entity, _, other_position)| {
            let offset = position - other_position;
            let distance_sq = offset.length_squared();
            (distance_sq <= spacing_radius_sq).then_some((other_entity, offset, distance_sq))
        })
        .min_by(|(a_entity, _, a_dist), (b_entity, _, b_dist)| {
            a_dist
                .partial_cmp(b_dist)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a_entity.index().cmp(&b_entity.index()))
        })
        .map(|(other_entity, offset, _)| {
            let direction = offset
                .try_normalize()
                .unwrap_or_else(|| deterministic_spacing_direction(entity, other_entity));
            position + direction * TEAMMATE_SPACING_RADIUS
        })
}

const fn deterministic_spacing_direction(entity: Entity, other_entity: Entity) -> Vec2 {
    if entity.index() <= other_entity.index() {
        Vec2::NEG_X
    } else {
        Vec2::X
    }
}

fn threat_targets(
    query: &Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
    player_position: Option<Vec2>,
    player_velocity: Vec2,
) -> Vec<ThreatTarget> {
    let mut threats: Vec<ThreatTarget> = query
        .iter()
        .map(|(_, virtual_player, transform)| ThreatTarget {
            team: virtual_player.team,
            position: transform.translation.xy(),
            // Heading times top speed: the same instantaneous velocity estimate the
            // kill press uses, so a home-flag defender can lead an approaching thief
            // to where it will breach the defensive ring.
            velocity: (transform.rotation * Vec3::Y).xy() * virtual_player.movement_speed,
        })
        .collect();

    if let Some(position) = player_position {
        // The human carries its tracked velocity (see [`PlayerVelocity`]), so a
        // defence against a human thief leads it to the ring crossing exactly as it
        // would a virtual one; a stale or absent track is a neutral zero, which the
        // lead logic resolves to the plain body-block fallback.
        threats.push(ThreatTarget {
            team: AiTeam::Blue,
            position,
            velocity: player_velocity,
        });
    }

    threats
}

/// A trackside pickup visible to virtual players, tagged with its kind so each
/// team can price it against its *own* wear when deciding whether to chase it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ArenaPickup {
    position: Vec2,
    kind: PickupKind,
}

fn arena_pickups(
    pickup_query: &Query<(&Transform, &Pickup), Without<VirtualPlayer>>,
) -> Vec<ArenaPickup> {
    pickup_query
        .iter()
        .map(|(transform, pickup)| ArenaPickup {
            position: transform.translation.xy(),
            kind: pickup.kind,
        })
        .collect()
}

/// Per-team record of which flags are in flight, the two situations that lift a
/// pickup above its own-wear value: is an enemy currently hauling this team's flag
/// away?
///
/// A live steal turns a sabotage into a carrier-chase tool (slow the thief so a
/// defender catches it), so the pricing reads this alongside durability. It does
/// double duty for the getaway case too: because a flag is only ever held by an
/// enemy, the *enemy* team's flag being stolen means *this* team is the one
/// hauling it home, so `for_team(team.enemy())` reads "we hold the enemy flag" and
/// turns a sabotage into getaway cover. Default (no flag in flight) leaves every
/// pickup at its integrity-scaled price.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct FlagStolen {
    blue: bool,
    red: bool,
}

impl FlagStolen {
    const fn for_team(self, team: AiTeam) -> bool {
        match team {
            AiTeam::Blue => self.blue,
            AiTeam::Red => self.red,
        }
    }
}

/// Reads which teams have an enemy carrying their flag this frame.
///
/// A flag is only ever held by an enemy (a car touching its own loose flag
/// returns it), so a [`FlagTarget`] with a holder marks that team's flag stolen.
fn flag_stolen_state(flags: &[FlagTarget]) -> FlagStolen {
    let stolen = |team: AiTeam| {
        flags
            .iter()
            .any(|flag| flag.team == team && flag.holder.is_some())
    };
    FlagStolen {
        blue: stolen(AiTeam::Blue),
        red: stolen(AiTeam::Red),
    }
}

/// Prices a pickup from `team`'s perspective: repairs and shields scale with that
/// team's own durability, a sabotage jumps when an enemy is hauling that team's
/// flag, and every other kind keeps its flat priority. Without an integrity
/// resource a team is treated as pristine, matching the unstarted-match default.
fn price_pickup_for_team(
    pickup: ArenaPickup,
    integrity: Option<&VehicleIntegrity>,
    team: AiTeam,
    flag_stolen: FlagStolen,
) -> PickupTarget {
    let fraction = integrity.map_or(1.0, |integrity| integrity.fraction_for_team(team));
    // A flag is only ever held by an enemy, so "we hold the enemy flag" is exactly
    // "the enemy team's flag is stolen": this team has a car running it home and
    // values a sabotage as getaway cover for that run.
    PickupTarget {
        position: pickup.position,
        priority: pickup.kind.virtual_player_priority_for_context(
            fraction,
            flag_stolen.for_team(team),
            flag_stolen.for_team(team.enemy()),
        ),
    }
}

fn claimed_pickups_for_virtual_players(
    query: &Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
    assigned_ctf_targets: &[(Entity, Option<DrivingTarget>)],
    pickups: &[ArenaPickup],
    integrity: Option<&VehicleIntegrity>,
    player_position: Option<Vec2>,
    discipline: ClosingTimePickupDiscipline,
    flag_stolen: FlagStolen,
) -> Vec<(Entity, PickupTarget)> {
    let mut ordered_pickups = pickups.to_vec();
    ordered_pickups.sort_by(|a, b| compare_pickup_claim_priority(*a, *b, integrity, flag_stolen));

    let mut claimed_entities = Vec::new();
    ordered_pickups
        .iter()
        .copied()
        .filter_map(|pickup| {
            let (entity, team) = closest_eligible_pickup_claimant(
                query,
                assigned_ctf_targets,
                pickup,
                integrity,
                player_position,
                &claimed_entities,
                discipline,
                flag_stolen,
            )?;
            claimed_entities.push(entity);
            Some((
                entity,
                price_pickup_for_team(pickup, integrity, team, flag_stolen),
            ))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn closest_eligible_pickup_claimant(
    query: &Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
    assigned_ctf_targets: &[(Entity, Option<DrivingTarget>)],
    pickup: ArenaPickup,
    integrity: Option<&VehicleIntegrity>,
    player_position: Option<Vec2>,
    claimed_entities: &[Entity],
    discipline: ClosingTimePickupDiscipline,
    flag_stolen: FlagStolen,
) -> Option<(Entity, AiTeam)> {
    let (entity, team, _) = query
        .iter()
        .filter(|(entity, _, _)| !claimed_entities.contains(entity))
        .filter_map(|(entity, ai, transform)| {
            let ctf_target = assigned_ctf_targets
                .iter()
                .find(|(assigned_entity, _)| *assigned_entity == entity)
                .and_then(|(_, target)| *target);
            let position = transform.translation.xy();
            let pickup_candidates = [price_pickup_for_team(
                pickup,
                integrity,
                ai.team,
                flag_stolen,
            )];
            let target = choose_driving_target(
                position,
                DrivingChoices {
                    waypoints: &ai.waypoints,
                    current_waypoint: ai.current_waypoint,
                    ctf_target,
                    pickups: &pickup_candidates,
                    pickup_pursuit_radius: ai.pickup_pursuit_radius,
                    player_position: player_position_for_team(ai.team, player_position),
                    player_pursuit_radius: ai.player_pursuit_radius,
                    closing_time_discipline: discipline.for_team(ai.team),
                },
            );

            (target == Some(DrivingTarget::Pickup(pickup.position)))
                .then_some((entity, ai.team, position))
        })
        .min_by(|(_, _, a_position), (_, _, b_position)| {
            a_position
                .distance_squared(pickup.position)
                .partial_cmp(&b_position.distance_squared(pickup.position))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| compare_positions(*a_position, *b_position))
        })
        .filter(|(_, team, position)| {
            !virtual_player_yields_player_pickup_claim(
                *team,
                player_position,
                price_pickup_for_team(pickup, integrity, *team, flag_stolen),
                *position,
            )
        })?;

    Some((entity, team))
}

/// Orders pickups by the most any team would pay, so a repair that a battered
/// team must have still earns early claim dibs even though the other team rates
/// it worthless. Only the claim order is shared; each claimant still pursues the
/// pickup at its own team's price.
fn compare_pickup_claim_priority(
    a: ArenaPickup,
    b: ArenaPickup,
    integrity: Option<&VehicleIntegrity>,
    flag_stolen: FlagStolen,
) -> std::cmp::Ordering {
    claim_priority(b, integrity, flag_stolen)
        .cmp(&claim_priority(a, integrity, flag_stolen))
        .then_with(|| compare_positions(a.position, b.position))
}

fn claim_priority(
    pickup: ArenaPickup,
    integrity: Option<&VehicleIntegrity>,
    flag_stolen: FlagStolen,
) -> u32 {
    price_pickup_for_team(pickup, integrity, AiTeam::Blue, flag_stolen)
        .priority
        .max(price_pickup_for_team(pickup, integrity, AiTeam::Red, flag_stolen).priority)
}

fn player_has_better_pickup_claim(
    player_position: Option<Vec2>,
    pickup: PickupTarget,
    virtual_player_position: Vec2,
) -> bool {
    let Some(player_position) = player_position else {
        return false;
    };

    let player_distance_sq = player_position.distance_squared(pickup.position);
    if player_distance_sq > PLAYER_PICKUP_PURSUIT_RADIUS * PLAYER_PICKUP_PURSUIT_RADIUS {
        return false;
    }

    player_distance_sq <= virtual_player_position.distance_squared(pickup.position)
}

fn virtual_player_yields_player_pickup_claim(
    team: AiTeam,
    player_position: Option<Vec2>,
    pickup: PickupTarget,
    virtual_player_position: Vec2,
) -> bool {
    team == AiTeam::Blue
        && player_has_better_pickup_claim(player_position, pickup, virtual_player_position)
}

fn flag_targets(
    flag_query: &Query<(&Transform, &CtfFlag), Without<VirtualPlayer>>,
    holder_positions: &[(Entity, Vec2)],
) -> Vec<FlagTarget> {
    flag_query
        .iter()
        .map(|(transform, flag)| FlagTarget {
            team: match flag.team {
                FlagTeam::Blue => AiTeam::Blue,
                FlagTeam::Red => AiTeam::Red,
            },
            home: flag.home,
            position: flag
                .holder
                .and_then(|holder| {
                    holder_positions
                        .iter()
                        .find(|(entity, _)| *entity == holder)
                        .map(|(_, position)| *position)
                })
                .unwrap_or_else(|| transform.translation.xy()),
            holder: flag.holder,
        })
        .collect()
}

fn holder_positions(
    query: &Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
    player: Option<(Entity, Vec2)>,
) -> Vec<(Entity, Vec2)> {
    let mut positions: Vec<(Entity, Vec2)> = query
        .iter()
        .map(|(entity, _, transform)| (entity, transform.translation.xy()))
        .collect();

    if let Some(player) = player {
        positions.push(player);
    }

    positions
}

fn assigned_ctf_targets(
    query: &Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
    flags: &[FlagTarget],
    threats: &[ThreatTarget],
) -> Vec<(Entity, Option<DrivingTarget>)> {
    let candidates: Vec<CtfTargetCandidate> = query
        .iter()
        .filter_map(|(entity, virtual_player, transform)| {
            let home_base = flags
                .iter()
                .find(|flag| flag.team == virtual_player.team)?
                .home;
            choose_capture_the_flag_target(
                entity,
                virtual_player.team,
                flags,
                threats,
                virtual_player.corner_throttle,
            )
            .map(|target| CtfTargetCandidate {
                entity,
                team: virtual_player.team,
                position: transform.translation.xy(),
                target,
                home_base,
                carries_enemy_flag: carries_enemy_flag(entity, virtual_player.team, flags),
            })
        })
        .collect();

    assign_ctf_targets(&candidates, flags)
}

/// Resolves which battered teams send a car home to pit-recover this frame.
///
/// Each team is priced against its own vehicle integrity: a team at or below
/// [`crate::gameplay::virtual_player::ai::PIT_RETREAT_INTEGRITY_FRACTION`] breaks
/// off its home-most non-carrier (see [`pit_retreat_car`]), which then limps home
/// and parks in the base zone to recover. The retreating car weaves around any
/// enemy planted on its run home (see [`pit_retreat_home_run_aim`]) rather than
/// ramming into the very foe that battered it, the same way a flag carrier jukes a
/// roadblock on its scoring run. A team trailing on captures in the closing
/// stretch is the exception: with the clock running out it keeps every car on the
/// equalising push rather than pulling one home to heal (see [`pit_retreat_car`]),
/// the defensive mirror of the kill-press clutch window. Without an integrity
/// resource (no combat loaded) no team ever retreats.
fn pit_retreat_targets(
    query: &Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
    flags: &[FlagTarget],
    threats: &[ThreatTarget],
    integrity: Option<&VehicleIntegrity>,
    captures: CaptureScore,
    clock: Option<&MatchClock>,
) -> Vec<(Entity, DrivingTarget)> {
    let Some(integrity) = integrity else {
        return Vec::new();
    };
    // A team trailing on captures in the closing stretch races the equaliser with
    // every car (see [`pit_retreat_car`]); absent a clock the window stays shut,
    // mirroring the kill-press clutch window in [`finish_off_targets`].
    let closing_time = clock.is_some_and(|clock| clock.is_closing_time());

    let mut targets = Vec::new();
    for team in [AiTeam::Blue, AiTeam::Red] {
        let Some(home) = flags
            .iter()
            .find(|flag| flag.team == team)
            .map(|flag| flag.home)
        else {
            continue;
        };
        let candidates: Vec<PitRetreatCandidate> = query
            .iter()
            .filter(|(_, virtual_player, _)| virtual_player.team == team)
            .map(|(entity, _, transform)| PitRetreatCandidate {
                entity,
                position: transform.translation.xy(),
                home,
                carries_enemy_flag: carries_enemy_flag(entity, team, flags),
            })
            .collect();
        let behind_on_captures =
            captures_for_team(captures, team.enemy()) > captures_for_team(captures, team);
        if let Some(entity) = pit_retreat_car(
            integrity.fraction_for_team(team),
            behind_on_captures,
            closing_time,
            &candidates,
        ) {
            let position = candidates
                .iter()
                .find(|candidate| candidate.entity == entity)
                .map_or(home, |candidate| candidate.position);
            // The retreating car weaves home on its own commitment-flexed line: a
            // reckless driver squeezes a tighter, faster berth past a blocker, a
            // disciplined one swings wider, just as a flag carrier does.
            let corner_throttle = query
                .get(entity)
                .map_or(MIN_THROTTLE, |(_, virtual_player, _)| {
                    virtual_player.corner_throttle
                });
            let aim = pit_retreat_home_run_aim(position, home, team, threats, corner_throttle);
            targets.push((entity, DrivingTarget::HomeBase(aim)));
        }
    }
    targets
}

/// Resolves which healthier teams break a car off to finish a reeling enemy.
///
/// The offensive counterpart to [`pit_retreat_targets`]: each team is priced
/// against both integrity pools, and a team that is the healthier of the two
/// while its enemy is reeling sends its keenest non-carrier (see
/// [`finish_off_car`]) to hunt the nearest enemy car and grind out the wreck.
/// Enemy positions come from the same threat list the defensive roles use, so a
/// hunter will press an enemy virtual player or the human flag-runner alike.
/// When the enemy is reeling *and* hauling this team's flag away, the hunt
/// redirects to that thief (the most valuable wreck on the board), read from the
/// team's own stolen flag, which sits at its carrier.
/// A team trailing on `captures` presses at even health (the comeback gamble),
/// mirroring the [`crate::gameplay::combat::most_wanted_wreck_bonus`] economy.
/// A team with cars to spare also piles a second hunter onto the same victim (see
/// [`pincer_partner`]), springing the combat pincer so the kill lands faster.
/// Without an integrity resource (no combat loaded) no team ever presses.
fn finish_off_targets(
    query: &Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
    flags: &[FlagTarget],
    threats: &[ThreatTarget],
    integrity: Option<&VehicleIntegrity>,
    captures: CaptureScore,
    clock: Option<&MatchClock>,
) -> Vec<(Entity, DrivingTarget)> {
    let Some(integrity) = integrity else {
        return Vec::new();
    };
    // In the match's closing stretch a trailing team widens its kill press to
    // chase a clutch wreck (see [`finish_off_car`]); absent a clock the window
    // stays shut, mirroring every other optional CTF resource.
    let closing_time = clock.is_some_and(|clock| clock.is_closing_time());

    let mut targets = Vec::new();
    for team in [AiTeam::Blue, AiTeam::Red] {
        let candidates: Vec<FinishOffCandidate> = query
            .iter()
            .filter(|(_, virtual_player, _)| virtual_player.team == team)
            .map(|(entity, _, transform)| FinishOffCandidate {
                entity,
                position: transform.translation.xy(),
                carries_enemy_flag: carries_enemy_flag(entity, team, flags),
            })
            .collect();
        let enemy_positions: Vec<Vec2> = threats
            .iter()
            .filter(|threat| threat.team == team.enemy())
            .map(|threat| threat.position)
            .collect();
        // Each enemy's instantaneous velocity, paired with its position, so the kill
        // press can lead a fleeing prey rather than tail-chase the spot it has left.
        // Read from the shared threat list (not the raw virtual-player query) so the
        // human flag-runner, which the query never sees, is led too: the threat list
        // now carries the human's tracked velocity (see [`PlayerVelocity`]).
        let enemy_velocities: Vec<(Vec2, Vec2)> = threats
            .iter()
            .filter(|threat| threat.team == team.enemy())
            .map(|threat| (threat.position, threat.velocity))
            .collect();
        // An enemy hauling this team's flag away is the single most valuable kill
        // on the board: a held flag sits at its carrier, so the team's own stolen
        // flag marks where the thief is. Hand it to the hunter so the kill press
        // chases the carrier rather than the merely-nearest foe.
        let enemy_flag_carrier = flags
            .iter()
            .find(|flag| flag.team == team && flag.holder.is_some())
            .map(|flag| flag.position);
        let behind_on_captures =
            captures_for_team(captures, team.enemy()) > captures_for_team(captures, team);
        if let Some((entity, prey)) = finish_off_car(
            integrity.fraction_for_team(team),
            integrity.fraction_for_team(team.enemy()),
            behind_on_captures,
            &candidates,
            &enemy_positions,
            enemy_flag_carrier,
            closing_time,
        ) {
            // Lead a prey loose in the open to where it is heading so the hunter
            // cuts it off, but shove a wall-pinned prey straight into the boundary to
            // spring the combat wall (or corner) crush instead (see [`finish_off_aim`]).
            let (hunter_position, hunter_speed) = query
                .iter()
                .find(|(candidate, _, _)| *candidate == entity)
                .map_or((prey, 0.0), |(_, virtual_player, transform)| {
                    (transform.translation.xy(), virtual_player.movement_speed)
                });
            let prey_velocity = enemy_velocities
                .iter()
                .find(|(position, _)| position.distance_squared(prey) <= f32::EPSILON)
                .map_or(Vec2::ZERO, |(_, velocity)| *velocity);
            let aim = finish_off_aim(
                hunter_position,
                prey,
                prey_velocity,
                hunter_speed,
                BOUNDS / 2.0,
            );
            targets.push((entity, DrivingTarget::FinishWreck(aim)));
            // Pile a second spare car onto the same victim when the team can field
            // one without abandoning the objective, springing the combat pincer so
            // the kill lands faster (see [`pincer_partner`]). The partner is picked
            // by its proximity to the real prey, then aimed at the same cut-off (or
            // wall-crush) point so both hunters converge on the victim together.
            if let Some(partner) = pincer_partner(entity, prey, &candidates) {
                targets.push((partner, DrivingTarget::FinishWreck(aim)));
            }
        }
    }
    targets
}

/// Resolves which leading teams recall a car to guard a closing-time lead.
///
/// The defensive counterpart to [`pit_retreat_targets`] and the mirror of the
/// trailing team's objective commitment: a team strictly ahead on captures in
/// the closing stretch (see [`lead_protection`]) recalls its home-most
/// non-carrier (see [`lead_defence_car`]) to its own home defensive lane, the
/// same well-worn [`DrivingTarget::DefendHomeBase`] role a threatened team already
/// takes, only stationed pre-emptively to protect the lead. Without a match clock
/// (an unstarted match) no team ever protects a lead.
fn lead_defence_targets(
    query: &Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
    flags: &[FlagTarget],
    clock: Option<&MatchClock>,
    captures: CaptureScore,
) -> Vec<(Entity, DrivingTarget)> {
    let protection = lead_protection(clock, captures);

    let mut targets = Vec::new();
    for team in [AiTeam::Blue, AiTeam::Red] {
        let Some(own_flag) = flags.iter().find(|flag| flag.team == team) else {
            continue;
        };
        let enemy_home = flags
            .iter()
            .find(|flag| flag.team == team.enemy())
            .map_or(own_flag.home, |enemy_flag| enemy_flag.home);
        let candidates: Vec<LeadDefenceCandidate> = query
            .iter()
            .filter(|(_, virtual_player, _)| virtual_player.team == team)
            .map(|(entity, _, transform)| LeadDefenceCandidate {
                entity,
                position: transform.translation.xy(),
                home: own_flag.home,
                carries_enemy_flag: carries_enemy_flag(entity, team, flags),
            })
            .collect();
        if let Some(entity) = lead_defence_car(protection.for_team(team), &candidates) {
            let guard = home_lane_guard_point(own_flag.home, enemy_home);
            targets.push((entity, DrivingTarget::DefendHomeBase(guard)));
        }
    }
    targets
}

/// Captures banked by `team`, reading the player tally for blue and the opponent
/// tally for red so the trailing team can be told apart from the leader.
const fn captures_for_team(captures: CaptureScore, team: AiTeam) -> u32 {
    match team {
        AiTeam::Blue => captures.player,
        AiTeam::Red => captures.opponents,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CtfTargetCandidate {
    entity: Entity,
    team: AiTeam,
    position: Vec2,
    target: DrivingTarget,
    home_base: Vec2,
    carries_enemy_flag: bool,
}

fn assign_ctf_targets(
    candidates: &[CtfTargetCandidate],
    flags: &[FlagTarget],
) -> Vec<(Entity, Option<DrivingTarget>)> {
    candidates
        .iter()
        .map(|candidate| {
            let target = if is_best_candidate_for_target(*candidate, candidates) {
                Some(candidate.target)
            } else {
                fallback_ctf_target(*candidate, candidates, flags)
            };
            (candidate.entity, target)
        })
        .collect()
}

fn fallback_ctf_target(
    candidate: CtfTargetCandidate,
    candidates: &[CtfTargetCandidate],
    flags: &[FlagTarget],
) -> Option<DrivingTarget> {
    if candidate.carries_enemy_flag {
        return Some(DrivingTarget::HomeBase(candidate.home_base));
    }

    if is_best_fallback_stolen_flag_route_guard(candidate, candidates, flags) {
        return stolen_flag_route_guard_target(candidate.team, flags);
    }

    if is_best_fallback_home_defender(candidate, candidates) {
        return defend_home_target(candidate.team, flags);
    }

    if is_best_fallback_midfield_interceptor(candidate, candidates, flags) {
        return midfield_interceptor_target(candidate.team, flags);
    }

    if is_best_fallback_enemy_flag_flanker(candidate, candidates, flags) {
        return enemy_flag_flank_target(candidate.team, flags);
    }

    None
}

fn is_best_fallback_stolen_flag_route_guard(
    candidate: CtfTargetCandidate,
    candidates: &[CtfTargetCandidate],
    flags: &[FlagTarget],
) -> bool {
    let Some(DrivingTarget::StolenHomeFlagRouteGuard(target)) =
        stolen_flag_route_guard_target(candidate.team, flags)
    else {
        return false;
    };

    candidates
        .iter()
        .copied()
        .filter(|other| {
            other.team == candidate.team
                && !other.carries_enemy_flag
                && !is_best_candidate_for_target(*other, candidates)
        })
        .min_by(|a, b| compare_candidates_by_distance_to(a, b, target))
        .is_some_and(|best| best.entity == candidate.entity)
}

/// Orders fallback candidates by proximity to a shared `target`, using the
/// car's `x` then `y` as a deterministic tie-breaker. Shared by every
/// single-target fallback role so they pick the same nearest car.
fn compare_candidates_by_distance_to(
    a: &CtfTargetCandidate,
    b: &CtfTargetCandidate,
    target: Vec2,
) -> std::cmp::Ordering {
    a.position
        .distance_squared(target)
        .partial_cmp(&b.position.distance_squared(target))
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| compare_positions(a.position, b.position))
}

fn is_best_candidate_for_target(
    candidate: CtfTargetCandidate,
    candidates: &[CtfTargetCandidate],
) -> bool {
    if !should_coordinate_ctf_target(candidate.target) {
        return true;
    }

    candidates
        .iter()
        .copied()
        .filter(|other| other.target == candidate.target)
        .min_by(compare_ctf_target_candidates)
        .is_some_and(|best| best.entity == candidate.entity)
}

fn is_best_fallback_home_defender(
    candidate: CtfTargetCandidate,
    candidates: &[CtfTargetCandidate],
) -> bool {
    candidates
        .iter()
        .copied()
        .filter(|other| {
            other.team == candidate.team
                && !other.carries_enemy_flag
                && !is_best_candidate_for_target(*other, candidates)
        })
        .min_by(compare_fallback_home_defenders)
        .is_some_and(|best| best.entity == candidate.entity)
}

fn compare_fallback_home_defenders(
    a: &CtfTargetCandidate,
    b: &CtfTargetCandidate,
) -> std::cmp::Ordering {
    a.position
        .distance_squared(a.home_base)
        .partial_cmp(&b.position.distance_squared(b.home_base))
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| compare_positions(a.position, b.position))
}

fn is_best_fallback_midfield_interceptor(
    candidate: CtfTargetCandidate,
    candidates: &[CtfTargetCandidate],
    flags: &[FlagTarget],
) -> bool {
    let Some(DrivingTarget::MidfieldInterceptor(target)) =
        midfield_interceptor_target(candidate.team, flags)
    else {
        return false;
    };

    candidates
        .iter()
        .copied()
        .filter(|other| {
            other.team == candidate.team
                && !other.carries_enemy_flag
                && !is_best_candidate_for_target(*other, candidates)
                && !is_best_fallback_home_defender(*other, candidates)
        })
        .min_by(|a, b| compare_candidates_by_distance_to(a, b, target))
        .is_some_and(|best| best.entity == candidate.entity)
}

fn is_best_fallback_enemy_flag_flanker(
    candidate: CtfTargetCandidate,
    candidates: &[CtfTargetCandidate],
    flags: &[FlagTarget],
) -> bool {
    let Some(DrivingTarget::EnemyFlagFlank(target)) =
        enemy_flag_flank_target(candidate.team, flags)
    else {
        return false;
    };

    candidates
        .iter()
        .copied()
        .filter(|other| {
            other.team == candidate.team
                && !other.carries_enemy_flag
                && !is_best_candidate_for_target(*other, candidates)
                && !is_best_fallback_home_defender(*other, candidates)
                && !is_best_fallback_midfield_interceptor(*other, candidates, flags)
        })
        .min_by(|a, b| compare_candidates_by_distance_to(a, b, target))
        .is_some_and(|best| best.entity == candidate.entity)
}

fn compare_ctf_target_candidates(
    a: &CtfTargetCandidate,
    b: &CtfTargetCandidate,
) -> std::cmp::Ordering {
    if matches!(a.target, DrivingTarget::StolenHomeFlag(_)) {
        return a
            .carries_enemy_flag
            .cmp(&b.carries_enemy_flag)
            .then_with(|| compare_ctf_target_distance(a, b));
    }

    compare_ctf_target_distance(a, b)
}

fn compare_ctf_target_distance(
    a: &CtfTargetCandidate,
    b: &CtfTargetCandidate,
) -> std::cmp::Ordering {
    a.position
        .distance_squared(a.target.position())
        .partial_cmp(&b.position.distance_squared(b.target.position()))
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| compare_positions(a.position, b.position))
}

fn carries_enemy_flag(entity: Entity, team: AiTeam, flags: &[FlagTarget]) -> bool {
    flags
        .iter()
        .any(|flag| flag.team == team.enemy() && flag.holder == Some(entity))
}

fn defend_home_target(team: AiTeam, flags: &[FlagTarget]) -> Option<DrivingTarget> {
    let own_flag = flags.iter().find(|flag| flag.team == team)?;
    let target = flags
        .iter()
        .find(|flag| flag.team == team.enemy())
        .map_or(own_flag.home, |enemy_flag| {
            home_lane_guard_point(own_flag.home, enemy_flag.home)
        });
    Some(DrivingTarget::DefendHomeBase(target))
}

fn midfield_interceptor_target(team: AiTeam, flags: &[FlagTarget]) -> Option<DrivingTarget> {
    let own_flag = flags.iter().find(|flag| flag.team == team)?;
    let enemy_flag = flags.iter().find(|flag| flag.team == team.enemy())?;
    let target = own_flag.home + (enemy_flag.home - own_flag.home) * MIDFIELD_LANE_GUARD_FACTOR;
    Some(DrivingTarget::MidfieldInterceptor(target))
}

fn enemy_flag_flank_target(team: AiTeam, flags: &[FlagTarget]) -> Option<DrivingTarget> {
    let own_flag = flags.iter().find(|flag| flag.team == team)?;
    let enemy_flag = flags.iter().find(|flag| flag.team == team.enemy())?;
    Some(DrivingTarget::EnemyFlagFlank(enemy_flag_flank_point(
        own_flag.home,
        enemy_flag.position,
    )))
}

fn stolen_flag_route_guard_target(team: AiTeam, flags: &[FlagTarget]) -> Option<DrivingTarget> {
    let own_flag = flags.iter().find(|flag| flag.team == team)?;
    if own_flag.holder.is_none()
        && own_flag.position.distance_squared(own_flag.home) <= f32::EPSILON
    {
        return None;
    }

    let target = own_flag.position + (own_flag.home - own_flag.position) * 0.5;
    Some(DrivingTarget::StolenHomeFlagRouteGuard(target))
}

fn enemy_flag_flank_point(home: Vec2, enemy_flag_position: Vec2) -> Vec2 {
    let to_enemy_flag = enemy_flag_position - home;
    let Some(direction) = to_enemy_flag.try_normalize() else {
        return enemy_flag_position;
    };
    let flank = Vec2::new(-direction.y, direction.x);
    enemy_flag_position + flank * HOME_LANE_GUARD_DISTANCE
}

fn home_lane_guard_point(home: Vec2, enemy_home: Vec2) -> Vec2 {
    let to_enemy_home = enemy_home - home;
    let distance = to_enemy_home.length();
    if distance <= HOME_LANE_GUARD_DISTANCE {
        return enemy_home;
    }

    let Some(direction) = to_enemy_home.try_normalize() else {
        return home;
    };
    home + direction * HOME_LANE_GUARD_DISTANCE
}

const fn nitro_multiplier_for_team(boosts: &NitroBoosts, team: AiTeam) -> f32 {
    match team {
        AiTeam::Blue => boosts.player_multiplier(),
        AiTeam::Red => boosts.opponent_multiplier(),
    }
}

const fn sabotage_multiplier_for_team(effects: &SabotageEffects, team: AiTeam) -> f32 {
    match team {
        AiTeam::Blue => effects.player_multiplier(),
        AiTeam::Red => effects.opponent_multiplier(),
    }
}

/// Combined speed multiplier a team's cars carry from the live combat resources:
/// the product of its nitro boost, integrity wear, wreck spin-out, wreck surge
/// and any enemy engine sabotage. Each absent resource (no combat loaded)
/// contributes a neutral `1.0`, matching the per-team multiplier the human player
/// movement applies. The flag-carry tax is applied separately, since it is
/// per-car rather than per-team.
fn team_movement_multiplier(
    team: AiTeam,
    nitro_boosts: Option<&NitroBoosts>,
    integrity: Option<&VehicleIntegrity>,
    wreck_stuns: Option<&WreckStuns>,
    wreck_surges: Option<&WreckSurges>,
    sabotage_effects: Option<&SabotageEffects>,
) -> f32 {
    let nitro = nitro_boosts.map_or(1.0, |boosts| nitro_multiplier_for_team(boosts, team));
    let integrity = integrity.map_or(1.0, |integrity| integrity.multiplier_for_team(team));
    let stun = wreck_stuns.map_or(1.0, |stuns| stuns.multiplier_for_team(team));
    let surge = wreck_surges.map_or(1.0, |surges| surges.multiplier_for_team(team));
    let sabotage =
        sabotage_effects.map_or(1.0, |effects| sabotage_multiplier_for_team(effects, team));
    nitro * integrity * stun * surge * sabotage
}

/// Arrive radius a car uses to reach `target`, flexed by its cornering
/// commitment.
///
/// Positional and patrol targets keep the wide [`WAYPOINT_ARRIVE_RADIUS`] so a
/// car settles on them cleanly (and a waypoint advances). Targets that *are* an
/// enemy car to ram, chasing the human player or finishing a reeling enemy off,
/// take the tight commitment-flexed [`pursuit_arrive_radius`] instead, so the car
/// drives through to hard contact rather than idling just outside ram range, and
/// a reckless driver presses that run-down deeper than a disciplined one.
fn arrive_radius_for_target(target: DrivingTarget, corner_throttle: f32) -> f32 {
    match target {
        DrivingTarget::Player(_) | DrivingTarget::FinishWreck(_) => {
            pursuit_arrive_radius(corner_throttle)
        }
        _ => WAYPOINT_ARRIVE_RADIUS,
    }
}

/// Arrive radius a driver commits to when running a foe down, flexed by its
/// cornering commitment around the [`PURSUIT_ARRIVE_RADIUS`] baseline.
///
/// A reckless driver commits a deeper run-down (a tighter radius, nosing further
/// through to contact), a disciplined one eases off a touch sooner, and the
/// neutral [`MIN_THROTTLE`] all-rounder keeps the exact baseline. Clamped to the
/// [`PURSUIT_ARRIVE_RADIUS_MIN`]..=[`PURSUIT_ARRIVE_RADIUS_MAX`] band so the depth
/// is always a positive distance well inside ram range, however extreme the
/// driver's commitment.
fn pursuit_arrive_radius(corner_throttle: f32) -> f32 {
    let depth = (corner_throttle - MIN_THROTTLE) * PURSUIT_COMMITMENT_DEPTH_GAIN;
    (PURSUIT_ARRIVE_RADIUS - depth).clamp(PURSUIT_ARRIVE_RADIUS_MIN, PURSUIT_ARRIVE_RADIUS_MAX)
}

const fn should_coordinate_ctf_target(target: DrivingTarget) -> bool {
    matches!(
        target,
        DrivingTarget::BlockFlagCarrierPursuer(_)
            | DrivingTarget::DefendHomeBase(_)
            | DrivingTarget::EnemyFlag(_)
            | DrivingTarget::EnemyFlagFlank(_)
            | DrivingTarget::EscortFlagCarrier(_)
            | DrivingTarget::StolenHomeFlagRouteGuard(_)
            | DrivingTarget::StolenHomeFlag(_)
            | DrivingTarget::UrgentDefendHomeBase(_)
    )
}

const fn player_position_for_team(team: AiTeam, player_position: Option<Vec2>) -> Option<Vec2> {
    match team {
        AiTeam::Blue => None,
        AiTeam::Red => player_position,
    }
}

#[cfg(test)]
mod tests;
