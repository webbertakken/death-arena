use crate::gameplay::combat::{VehicleIntegrity, WreckStuns, WreckSurges, RAM_RADIUS};
use crate::gameplay::ctf::{
    flag_carrier_speed_multiplier, CaptureScore, CtfFlag, CtfMatchResult, FlagTeam, MatchClock,
};
use crate::gameplay::main::{BOUNDS, TIME_STEP};
use crate::gameplay::pickup::{NitroBoosts, Pickup, PickupKind, SabotageEffects};
use crate::gameplay::player::Player;
use crate::gameplay::virtual_player::ai::{
    choose_capture_the_flag_target, choose_driving_target, compare_positions, compute_steering,
    finish_off_car, finish_off_wall_crush_aim, lead_defence_car, next_waypoint, pincer_partner,
    pit_retreat_car, pit_retreat_home_run_aim, AiTeam, DrivingChoices, DrivingTarget,
    FinishOffCandidate, FlagTarget, LeadDefenceCandidate, PickupTarget, PitRetreatCandidate,
    ThreatTarget,
};
use crate::gameplay::virtual_player::VirtualPlayer;
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;

/// Distance (world units) at which a virtual player considers a waypoint
/// reached and advances to the next one.
const WAYPOINT_ARRIVE_RADIUS: f32 = 80.0;
/// Tighter arrive radius for a target a car means to *ram*: chasing the human
/// player or hunting a reeling enemy down.
///
/// The wide [`WAYPOINT_ARRIVE_RADIUS`] sits outside true ram range
/// ([`RAM_RADIUS`]), so a car that idles the instant it reaches it coasts to a
/// halt short of contact and the chase stutters. Kept well inside ram range
/// instead, so a hunter drives all the way through to a hard hit and shoves its
/// victim, the aggressive Death Rally run-down rather than a polite stop.
const PURSUIT_ARRIVE_RADIUS: f32 = 30.0;
/// A ram run-down must commit deeper than a waypoint stop, enforced at compile
/// time.
const _: () = assert!(PURSUIT_ARRIVE_RADIUS < WAYPOINT_ARRIVE_RADIUS);
/// A ram run-down must close well inside true ram range, enforced at compile
/// time, so the car is genuinely trading paint before it ever idles.
const _: () = assert!(PURSUIT_ARRIVE_RADIUS < RAM_RADIUS);
/// The human player's pickup-scavenging reach, used when deciding whether the
/// human has a better claim on a bag than a blue teammate. Each virtual player
/// scavenges at its own personality-driven [`VirtualPlayer::pickup_pursuit_radius`];
/// the human has no driving personality, so it mirrors the all-rounder baseline.
const PLAYER_PICKUP_PURSUIT_RADIUS: f32 = 450.0;
const HOME_LANE_GUARD_DISTANCE: f32 = 220.0;
const MIDFIELD_LANE_GUARD_FACTOR: f32 = 0.5;
const TEAMMATE_SPACING_RADIUS: f32 = 90.0;

type HumanPlayerTransform = (With<Player>, Without<VirtualPlayer>);

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
/// `sabotage_effects` rides as its own parameter rather than in the bundled
/// [`VirtualPlayerDriveContext`]: it is the eighth optional resource and folding
/// it into the tuple expands the destructure enough to trip clippy's line gate,
/// so it stays separate (still well inside the argument limit).
pub fn virtual_player_drive_system(
    mut query: Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
    player_query: Query<(Entity, &Transform), HumanPlayerTransform>,
    pickup_query: Query<(&Transform, &Pickup), Without<VirtualPlayer>>,
    flag_query: Query<(&Transform, &CtfFlag), Without<VirtualPlayer>>,
    sabotage_effects: Option<Res<SabotageEffects>>,
    context: VirtualPlayerDriveContext,
) {
    let (nitro_boosts, integrity, wreck_stuns, wreck_surges, match_result, captures, match_clock) =
        context;
    if match_result
        .as_ref()
        .is_some_and(|result| result.winner.is_some())
    {
        return;
    }
    let captures = captures.as_deref().copied().unwrap_or_default();
    // Closing-time discipline: in the final stretch of a round every team leaves
    // cash bags on the track, whether it is committing to attack (not ahead) or
    // protecting a lead (ahead). Only a real edge is worth breaking off for.
    let discipline = closing_time_pickup_discipline(match_clock.as_deref(), captures);

    let player = player_query
        .get_single()
        .ok()
        .map(|(entity, transform)| (entity, transform.translation.xy()));
    let player_position = player.map(|(_, position)| position);
    let threats = threat_targets(&query, player_position);
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
        let entity_pickups: Vec<PickupTarget> = claimed_pickups
            .iter()
            .filter_map(|(assigned_entity, pickup)| (*assigned_entity == entity).then_some(*pickup))
            .collect();
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
        let intent = compute_steering(
            position,
            forward,
            target_position,
            arrive_radius_for_target(target),
            ai.corner_throttle,
        );

        if intent == crate::gameplay::virtual_player::ai::SteeringIntent::IDLE {
            if matches!(target, DrivingTarget::PatrolWaypoint(_)) {
                ai.current_waypoint = next_waypoint(ai.current_waypoint, ai.waypoints.len());
            }
            continue;
        }

        // Rotation: positive steer turns left (counter-clockwise).
        transform.rotate_z(intent.steer * ai.rotation_speed * TIME_STEP);

        // Translation along the (rotated) forward vector.
        let movement_direction = transform.rotation * Vec3::Y;
        let carry_multiplier =
            flag_carrier_speed_multiplier(carries_enemy_flag(entity, ai.team, &flags));
        let movement_distance = intent.throttle
            * ai.movement_speed
            * team_movement_multiplier(
                ai.team,
                nitro_boosts.as_deref(),
                integrity.as_deref(),
                wreck_stuns.as_deref(),
                wreck_surges.as_deref(),
                sabotage_effects.as_deref(),
            )
            * carry_multiplier
            * TIME_STEP;
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
    pit_retreat_targets(query, flags, threats, integrity)
        .into_iter()
        .chain(finish_off_targets(
            query, flags, threats, integrity, captures,
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
) -> Vec<ThreatTarget> {
    let mut threats: Vec<ThreatTarget> = query
        .iter()
        .map(|(_, virtual_player, transform)| ThreatTarget {
            team: virtual_player.team,
            position: transform.translation.xy(),
        })
        .collect();

    if let Some(position) = player_position {
        threats.push(ThreatTarget {
            team: AiTeam::Blue,
            position,
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

/// Per-team flag for closing-time clutch play: should this team's cars commit to
/// the CTF objective and stop chasing opportunistic pickup detours?
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ObjectiveCommitment {
    blue: bool,
    red: bool,
}

impl ObjectiveCommitment {
    const fn for_team(self, team: AiTeam) -> bool {
        match team {
            AiTeam::Blue => self.blue,
            AiTeam::Red => self.red,
        }
    }
}

/// Decides which teams commit to the objective given the round clock and the
/// capture scoreline.
///
/// Outside the closing stretch no team commits. Within it every team that is not
/// ahead on captures does: a closing-time leader keeps playing the field, while a
/// trailing side, and both sides of a level sudden death, drop cash detours to
/// race the flag. A missing clock (an unstarted match) never forces commitment.
fn objective_commitment(clock: Option<&MatchClock>, captures: CaptureScore) -> ObjectiveCommitment {
    if !clock.is_some_and(|clock| clock.is_closing_time()) {
        return ObjectiveCommitment::default();
    }
    ObjectiveCommitment {
        blue: captures.player <= captures.opponents,
        red: captures.opponents <= captures.player,
    }
}

/// Per-team flag for closing-time lead protection: should this team, ahead on
/// captures with the clock running down, recall a car to guard its lead?
///
/// The exact complement of [`ObjectiveCommitment`]: in the closing stretch every
/// team is either committing to attack (not ahead) or protecting a lead (ahead),
/// so the trailing/level side races the flag while the leading side digs in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct LeadProtection {
    blue: bool,
    red: bool,
}

impl LeadProtection {
    const fn for_team(self, team: AiTeam) -> bool {
        match team {
            AiTeam::Blue => self.blue,
            AiTeam::Red => self.red,
        }
    }
}

/// Decides which teams protect a lead given the round clock and the scoreline.
///
/// Outside the closing stretch no team protects. Within it a team strictly ahead
/// on captures recalls a car to guard (see
/// [`crate::gameplay::virtual_player::ai::lead_defence_car`]); a trailing or
/// level side does not, since it is busy committing to the objective. A missing
/// clock (an unstarted match) never forces protection. Mirrors and complements
/// [`objective_commitment`]: a team protects exactly when it is *not* committing.
fn lead_protection(clock: Option<&MatchClock>, captures: CaptureScore) -> LeadProtection {
    if !clock.is_some_and(|clock| clock.is_closing_time()) {
        return LeadProtection::default();
    }
    LeadProtection {
        blue: captures.player > captures.opponents,
        red: captures.opponents > captures.player,
    }
}

/// Per-team flag for closing-time pickup discipline: should this team's cars
/// leave cash bags on the track and only break off an objective for a real edge?
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ClosingTimePickupDiscipline {
    blue: bool,
    red: bool,
}

impl ClosingTimePickupDiscipline {
    const fn for_team(self, team: AiTeam) -> bool {
        match team {
            AiTeam::Blue => self.blue,
            AiTeam::Red => self.red,
        }
    }
}

/// Decides which teams discipline their pickup detours given the round clock and
/// scoreline.
///
/// In the closing stretch every team is either committing to attack (not ahead,
/// see [`objective_commitment`]) or protecting a lead (ahead, see
/// [`lead_protection`]); with the clock running out a cash bag is a distraction
/// either way, so the discipline is the union of the two complementary
/// predicates and a closing-time leader stops farming cash just as a trailing
/// team does. Outside the closing stretch neither holds, so no team disciplines
/// and cash bags are fair game again.
fn closing_time_pickup_discipline(
    clock: Option<&MatchClock>,
    captures: CaptureScore,
) -> ClosingTimePickupDiscipline {
    let commit = objective_commitment(clock, captures);
    let protect = lead_protection(clock, captures);
    ClosingTimePickupDiscipline {
        blue: commit.for_team(AiTeam::Blue) || protect.for_team(AiTeam::Blue),
        red: commit.for_team(AiTeam::Red) || protect.for_team(AiTeam::Red),
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
            choose_capture_the_flag_target(entity, virtual_player.team, flags, threats).map(
                |target| CtfTargetCandidate {
                    entity,
                    team: virtual_player.team,
                    position: transform.translation.xy(),
                    target,
                    home_base,
                    carries_enemy_flag: carries_enemy_flag(entity, virtual_player.team, flags),
                },
            )
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
/// roadblock on its scoring run. Without an integrity resource (no combat loaded)
/// no team ever retreats.
fn pit_retreat_targets(
    query: &Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
    flags: &[FlagTarget],
    threats: &[ThreatTarget],
    integrity: Option<&VehicleIntegrity>,
) -> Vec<(Entity, DrivingTarget)> {
    let Some(integrity) = integrity else {
        return Vec::new();
    };

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
        if let Some(entity) = pit_retreat_car(integrity.fraction_for_team(team), &candidates) {
            let position = candidates
                .iter()
                .find(|candidate| candidate.entity == entity)
                .map_or(home, |candidate| candidate.position);
            let aim = pit_retreat_home_run_aim(position, home, team, threats);
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
) -> Vec<(Entity, DrivingTarget)> {
    let Some(integrity) = integrity else {
        return Vec::new();
    };

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
        ) {
            // When the prey is pinned near a wall, aim past it into the boundary so
            // the charge shoves it in and springs the combat wall (or corner) crush
            // rather than scraping it in the open (see [`finish_off_wall_crush_aim`]).
            let aim = finish_off_wall_crush_aim(prey, BOUNDS / 2.0);
            targets.push((entity, DrivingTarget::FinishWreck(aim)));
            // Pile a second spare car onto the same victim when the team can field
            // one without abandoning the objective, springing the combat pincer so
            // the kill lands faster (see [`pincer_partner`]). The partner is picked
            // by its proximity to the real prey, then aimed at the same wall-crush
            // point so both hunters shove the victim into the boundary together.
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

/// Arrive radius a car uses to reach `target`.
///
/// Positional and patrol targets keep the wide [`WAYPOINT_ARRIVE_RADIUS`] so a
/// car settles on them cleanly (and a waypoint advances). Targets that *are* an
/// enemy car to ram, chasing the human player or finishing a reeling enemy off,
/// take the tight [`PURSUIT_ARRIVE_RADIUS`] instead, so the car drives through
/// to hard contact rather than idling just outside ram range.
const fn arrive_radius_for_target(target: DrivingTarget) -> f32 {
    match target {
        DrivingTarget::Player(_) | DrivingTarget::FinishWreck(_) => PURSUIT_ARRIVE_RADIUS,
        _ => WAYPOINT_ARRIVE_RADIUS,
    }
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
mod tests {
    use super::*;
    use crate::gameplay::combat::MAX_INTEGRITY;
    use crate::gameplay::ctf::{CtfFlag, CtfMatchWinner, FlagTeam};
    use crate::gameplay::virtual_player::VirtualPlayer;

    fn app_with_system() -> App {
        let mut app = App::new();
        app.add_system(virtual_player_drive_system);
        app
    }

    fn spawn_player(app: &mut App, position: Vec3) -> Entity {
        app.world
            .spawn((
                Player {
                    movement_speed: 0.0,
                    rotation_speed: 0.0,
                    engine_max_speed_multiplier: 0.0,
                    forward_max_speed_base: 0.0,
                    backward_max_speed_base: 0.0,
                    wheels_turning_multiplier: 0.0,
                },
                Transform::from_translation(position),
            ))
            .id()
    }

    fn spawn_ai(app: &mut App, waypoints: Vec<Vec2>) -> Entity {
        spawn_ai_on_team(app, AiTeam::Red, waypoints)
    }

    /// Baseline pursuit radius for test fixtures: matches the all-rounder driving
    /// personality, the neutral feel every behavioural assertion is measured
    /// against.
    const TEST_PURSUIT_RADIUS: f32 = 500.0;

    /// Baseline pickup-scavenging radius for test fixtures: matches the all-rounder
    /// driving personality and the former uniform global, the neutral greed every
    /// pickup-behaviour assertion is measured against.
    const TEST_PICKUP_PURSUIT_RADIUS: f32 = 450.0;

    fn spawn_ai_on_team(app: &mut App, team: AiTeam, waypoints: Vec<Vec2>) -> Entity {
        spawn_ai_with_pursuit(app, team, waypoints, TEST_PURSUIT_RADIUS)
    }

    fn spawn_ai_with_pursuit(
        app: &mut App,
        team: AiTeam,
        waypoints: Vec<Vec2>,
        player_pursuit_radius: f32,
    ) -> Entity {
        app.world
            .spawn((
                VirtualPlayer {
                    team,
                    movement_speed: 500.0,
                    rotation_speed: f32::to_radians(360.0),
                    waypoints,
                    current_waypoint: 0,
                    player_pursuit_radius,
                    pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                    corner_throttle: 0.3,
                },
                Transform::from_translation(Vec3::new(0.0, 0.0, 4.0)),
            ))
            .id()
    }

    /// Spawns a Red driver with a bespoke pickup-scavenging radius (and the
    /// baseline player-pursuit reach), so a test can pit a greedy personality
    /// against a disciplined one on the same pickup.
    fn spawn_ai_with_pickup_pursuit(
        app: &mut App,
        waypoints: Vec<Vec2>,
        pickup_pursuit_radius: f32,
    ) -> Entity {
        app.world
            .spawn((
                VirtualPlayer {
                    team: AiTeam::Red,
                    movement_speed: 500.0,
                    rotation_speed: f32::to_radians(360.0),
                    waypoints,
                    current_waypoint: 0,
                    player_pursuit_radius: TEST_PURSUIT_RADIUS,
                    pickup_pursuit_radius,
                    corner_throttle: 0.3,
                },
                Transform::from_translation(Vec3::new(0.0, 0.0, 4.0)),
            ))
            .id()
    }

    fn spawn_ai_at(app: &mut App, waypoints: Vec<Vec2>, translation: Vec3) -> Entity {
        app.world
            .spawn((
                VirtualPlayer {
                    team: AiTeam::Red,
                    movement_speed: 500.0,
                    rotation_speed: f32::to_radians(360.0),
                    waypoints,
                    current_waypoint: 0,
                    player_pursuit_radius: TEST_PURSUIT_RADIUS,
                    pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                    corner_throttle: 0.3,
                },
                Transform::from_translation(translation),
            ))
            .id()
    }

    fn spawn_flag(
        app: &mut App,
        team: FlagTeam,
        home: Vec2,
        position: Vec3,
        holder: Option<Entity>,
    ) {
        app.world.spawn((
            CtfFlag { team, home, holder },
            Transform::from_translation(position),
        ));
    }

    fn one_frame_ai_y(team: AiTeam, nitro: Option<fn(&mut NitroBoosts)>) -> f32 {
        let mut app = app_with_system();
        if let Some(trigger) = nitro {
            app.init_resource::<NitroBoosts>();
            trigger(&mut app.world.resource_mut::<NitroBoosts>());
        }
        let ai = spawn_ai_on_team(&mut app, team, vec![Vec2::new(0.0, 1000.0)]);

        app.update();

        app.world.get::<Transform>(ai).unwrap().translation.y
    }

    #[test]
    fn no_team_commits_to_the_objective_outside_closing_time() {
        use crate::gameplay::ctf::{MatchPhase, CLOSING_TIME_FRAMES};

        let clock = MatchClock {
            frames_remaining: CLOSING_TIME_FRAMES + 1,
            phase: MatchPhase::Regulation,
        };
        assert_eq!(
            objective_commitment(
                Some(&clock),
                CaptureScore {
                    player: 0,
                    opponents: 2,
                },
            ),
            ObjectiveCommitment::default(),
            "a round with time to spare must not force any team to commit"
        );
    }

    #[test]
    fn closing_time_commits_every_team_that_is_not_ahead() {
        use crate::gameplay::ctf::{MatchPhase, CLOSING_TIME_FRAMES};

        let clock = MatchClock {
            frames_remaining: CLOSING_TIME_FRAMES,
            phase: MatchPhase::Regulation,
        };
        // Blue (player) trails Red (opponents) on captures.
        let commitment = objective_commitment(
            Some(&clock),
            CaptureScore {
                player: 1,
                opponents: 2,
            },
        );
        assert!(
            commitment.for_team(AiTeam::Blue),
            "the trailing team drops its detours and races the flag"
        );
        assert!(
            !commitment.for_team(AiTeam::Red),
            "the closing-time leader keeps playing the field"
        );
    }

    #[test]
    fn a_level_sudden_death_commits_both_teams() {
        use crate::gameplay::ctf::MatchPhase;

        let clock = MatchClock {
            frames_remaining: 1,
            phase: MatchPhase::SuddenDeath,
        };
        let commitment = objective_commitment(
            Some(&clock),
            CaptureScore {
                player: 2,
                opponents: 2,
            },
        );
        assert!(commitment.for_team(AiTeam::Blue));
        assert!(
            commitment.for_team(AiTeam::Red),
            "golden goal: both level sides race for the decider"
        );
    }

    #[test]
    fn a_missing_clock_never_forces_commitment() {
        assert_eq!(
            objective_commitment(
                None,
                CaptureScore {
                    player: 0,
                    opponents: 3,
                },
            ),
            ObjectiveCommitment::default(),
            "an unstarted match (no clock) plays the field as normal"
        );
    }

    #[test]
    fn a_committing_team_skips_a_cash_detour_to_race_the_flag() {
        use crate::gameplay::ctf::{MatchPhase, MATCH_TIME_LIMIT_FRAMES};

        // A red attacker at the origin facing +Y, the blue (enemy) flag dead
        // ahead and sitting at home so the car is assigned to go steal it. A cash
        // bag sits just ahead and to the right, inside the flag lane: a free grab
        // in normal play that a clock-racing team should leave on the track.
        fn attacker_x_after_one_frame(closing: bool) -> f32 {
            let mut app = app_with_system();
            // Red (opponents) trails, so in closing time it commits to the push.
            app.insert_resource(CaptureScore {
                player: 1,
                opponents: 0,
            });
            app.insert_resource(MatchClock {
                frames_remaining: if closing { 10 } else { MATCH_TIME_LIMIT_FRAMES },
                phase: MatchPhase::Regulation,
            });

            let attacker = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 2000.0)]);
            spawn_flag(
                &mut app,
                FlagTeam::Blue,
                Vec2::new(0.0, 800.0),
                Vec3::new(0.0, 800.0, 0.0),
                None,
            );
            spawn_flag(
                &mut app,
                FlagTeam::Red,
                Vec2::new(0.0, -1000.0),
                Vec3::new(0.0, -1000.0, 0.0),
                None,
            );
            app.world.spawn((
                Pickup {
                    kind: PickupKind::Cash,
                },
                Transform::from_translation(Vec3::new(40.0, 80.0, 2.0)),
            ));

            app.update();

            app.world.get::<Transform>(attacker).unwrap().translation.x
        }

        let detoured = attacker_x_after_one_frame(false);
        let committed = attacker_x_after_one_frame(true);

        assert!(
            detoured > 0.1,
            "normal play veers right toward the cash bag: {detoured}"
        );
        assert!(
            committed.abs() < 1e-3,
            "closing-time commitment drives straight at the flag, ignoring the cash: {committed}"
        );
    }

    #[test]
    fn a_leading_team_also_skips_a_cash_detour_in_closing_time() {
        use crate::gameplay::ctf::{MatchPhase, MATCH_TIME_LIMIT_FRAMES};

        // The mirror of the committing-team detour test for the side that is
        // ahead. A lone red attacker, *leading* on captures, is assigned to steal
        // the blue flag dead ahead, with a cash bag just inside the flag lane. A
        // trailing team already leaves that bag (it commits); a leader running
        // down the clock should too, rather than greedily farming cash on a lead
        // it is about to win on. The lone car is never recalled to defend (the
        // lead-defence guard never pulls a team's last car), so the only thing
        // that can hold it off the bag is the broadened closing-time discipline.
        fn attacker_x_after_one_frame(closing: bool) -> f32 {
            let mut app = app_with_system();
            // Red (opponents) leads, so it never "commits"; only the discipline
            // that now also covers a protecting leader can leave the cash bag.
            app.insert_resource(CaptureScore {
                player: 0,
                opponents: 1,
            });
            app.insert_resource(MatchClock {
                frames_remaining: if closing { 10 } else { MATCH_TIME_LIMIT_FRAMES },
                phase: MatchPhase::Regulation,
            });

            let attacker = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 2000.0)]);
            spawn_flag(
                &mut app,
                FlagTeam::Blue,
                Vec2::new(0.0, 800.0),
                Vec3::new(0.0, 800.0, 0.0),
                None,
            );
            spawn_flag(
                &mut app,
                FlagTeam::Red,
                Vec2::new(0.0, -1000.0),
                Vec3::new(0.0, -1000.0, 0.0),
                None,
            );
            app.world.spawn((
                Pickup {
                    kind: PickupKind::Cash,
                },
                Transform::from_translation(Vec3::new(40.0, 80.0, 2.0)),
            ));

            app.update();

            app.world.get::<Transform>(attacker).unwrap().translation.x
        }

        let detoured = attacker_x_after_one_frame(false);
        let disciplined = attacker_x_after_one_frame(true);

        assert!(
            detoured > 0.1,
            "outside closing time even a leader veers right for the free cash bag: {detoured}"
        );
        assert!(
            disciplined.abs() < 1e-3,
            "in closing time a leader leaves the cash and races the flag too: {disciplined}"
        );
    }

    #[test]
    fn no_team_protects_a_lead_outside_closing_time() {
        use crate::gameplay::ctf::{MatchPhase, CLOSING_TIME_FRAMES};

        let clock = MatchClock {
            frames_remaining: CLOSING_TIME_FRAMES + 1,
            phase: MatchPhase::Regulation,
        };
        assert_eq!(
            lead_protection(
                Some(&clock),
                CaptureScore {
                    player: 2,
                    opponents: 0,
                },
            ),
            LeadProtection::default(),
            "a round with time to spare must not pull the leader back to defend"
        );
    }

    #[test]
    fn closing_time_protects_only_the_team_that_is_ahead() {
        use crate::gameplay::ctf::{MatchPhase, CLOSING_TIME_FRAMES};

        let clock = MatchClock {
            frames_remaining: CLOSING_TIME_FRAMES,
            phase: MatchPhase::Regulation,
        };
        // Blue (player) leads Red (opponents) on captures.
        let protection = lead_protection(
            Some(&clock),
            CaptureScore {
                player: 2,
                opponents: 1,
            },
        );
        assert!(
            protection.for_team(AiTeam::Blue),
            "the leader digs in to guard its lead"
        );
        assert!(
            !protection.for_team(AiTeam::Red),
            "the trailing team commits to attack, it does not protect"
        );
    }

    #[test]
    fn a_level_sudden_death_protects_no_team() {
        use crate::gameplay::ctf::MatchPhase;

        let clock = MatchClock {
            frames_remaining: 1,
            phase: MatchPhase::SuddenDeath,
        };
        assert_eq!(
            lead_protection(
                Some(&clock),
                CaptureScore {
                    player: 2,
                    opponents: 2,
                },
            ),
            LeadProtection::default(),
            "golden goal: no one is ahead, so both sides race the decider"
        );
    }

    #[test]
    fn protection_is_the_exact_complement_of_commitment_in_closing_time() {
        use crate::gameplay::ctf::MatchPhase;

        let clock = MatchClock {
            frames_remaining: 1,
            phase: MatchPhase::SuddenDeath,
        };
        for (player, opponents) in [(0, 0), (2, 1), (1, 2)] {
            let captures = CaptureScore { player, opponents };
            let commit = objective_commitment(Some(&clock), captures);
            let protect = lead_protection(Some(&clock), captures);
            for team in [AiTeam::Blue, AiTeam::Red] {
                assert_ne!(
                    commit.for_team(team),
                    protect.for_team(team),
                    "in closing time a team either commits or protects, never both nor neither"
                );
            }
        }
    }

    #[test]
    fn closing_time_disciplines_both_the_leader_and_the_trailer() {
        use crate::gameplay::ctf::{MatchPhase, CLOSING_TIME_FRAMES};

        let clock = MatchClock {
            frames_remaining: CLOSING_TIME_FRAMES,
            phase: MatchPhase::Regulation,
        };
        // Blue (player) leads Red (opponents): the leader protects, the trailer
        // commits, yet in the closing stretch both leave cash bags on the track.
        let discipline = closing_time_pickup_discipline(
            Some(&clock),
            CaptureScore {
                player: 2,
                opponents: 1,
            },
        );
        assert!(
            discipline.for_team(AiTeam::Blue),
            "the leader stops farming cash while it protects its lead"
        );
        assert!(
            discipline.for_team(AiTeam::Red),
            "the trailing team stops farming cash while it commits to attack"
        );
    }

    #[test]
    fn no_team_disciplines_its_detours_outside_closing_time() {
        use crate::gameplay::ctf::{MatchPhase, CLOSING_TIME_FRAMES};

        let clock = MatchClock {
            frames_remaining: CLOSING_TIME_FRAMES + 1,
            phase: MatchPhase::Regulation,
        };
        assert_eq!(
            closing_time_pickup_discipline(
                Some(&clock),
                CaptureScore {
                    player: 2,
                    opponents: 1,
                },
            ),
            ClosingTimePickupDiscipline::default(),
            "with time to spare a cash bag is fair game for either side"
        );
        assert_eq!(
            closing_time_pickup_discipline(
                None,
                CaptureScore {
                    player: 2,
                    opponents: 1,
                },
            ),
            ClosingTimePickupDiscipline::default(),
            "an unstarted match (no clock) plays the field as normal"
        );
    }

    #[test]
    fn closing_time_discipline_is_the_union_of_commitment_and_protection() {
        use crate::gameplay::ctf::MatchPhase;

        let clock = MatchClock {
            frames_remaining: 1,
            phase: MatchPhase::SuddenDeath,
        };
        for (player, opponents) in [(0, 0), (2, 1), (1, 2)] {
            let captures = CaptureScore { player, opponents };
            let commit = objective_commitment(Some(&clock), captures);
            let protect = lead_protection(Some(&clock), captures);
            let discipline = closing_time_pickup_discipline(Some(&clock), captures);
            for team in [AiTeam::Blue, AiTeam::Red] {
                assert_eq!(
                    discipline.for_team(team),
                    commit.for_team(team) || protect.for_team(team),
                    "discipline must be the per-team union of commitment and protection"
                );
            }
        }
    }

    #[test]
    fn a_leading_team_recalls_its_home_most_car_to_defend_in_closing_time() {
        use crate::gameplay::ctf::{MatchPhase, MATCH_TIME_LIMIT_FRAMES};

        // Red home sits at the origin and the blue (enemy) flag and base straight
        // ahead at +Y. A free red car drives forward to attack the enemy flag; a
        // car recalled to guard the lead instead heads back down its home lane
        // (toward the guard point at +Y 220), so a car sitting forward of that
        // point reverses. The home-most red car starts at (0, 500), forward of the
        // guard point but short of the flag, so the two intents pull opposite ways.
        fn home_most_dy(protecting: bool) -> f32 {
            let mut app = app_with_system();
            // Red (opponents) leads, so in closing time it protects that lead.
            app.insert_resource(CaptureScore {
                player: 0,
                opponents: 1,
            });
            app.insert_resource(MatchClock {
                frames_remaining: if protecting {
                    10
                } else {
                    MATCH_TIME_LIMIT_FRAMES
                },
                phase: MatchPhase::Regulation,
            });

            let home_most = spawn_ai_at(
                &mut app,
                vec![Vec2::new(0.0, 2000.0)],
                Vec3::new(0.0, 500.0, 4.0),
            );
            // A second, more-forward red car keeps the team above the lone-car
            // guard and is never the home-most pick.
            spawn_ai_at(
                &mut app,
                vec![Vec2::new(0.0, 2000.0)],
                Vec3::new(0.0, 1500.0, 4.0),
            );
            spawn_flag(
                &mut app,
                FlagTeam::Red,
                Vec2::ZERO,
                Vec3::new(0.0, 0.0, 4.0),
                None,
            );
            spawn_flag(
                &mut app,
                FlagTeam::Blue,
                Vec2::new(0.0, 1000.0),
                Vec3::new(0.0, 1000.0, 4.0),
                None,
            );

            app.update();

            app.world.get::<Transform>(home_most).unwrap().translation.y - 500.0
        }

        let attacking = home_most_dy(false);
        let defending = home_most_dy(true);

        assert!(
            attacking > 0.1,
            "outside closing time the leader's car pushes forward to attack: {attacking}"
        );
        assert!(
            defending < -0.1,
            "in closing time the leader recalls its home-most car to guard the lead: {defending}"
        );
    }

    #[test]
    fn a_trailing_team_is_not_recalled_to_defend_in_closing_time() {
        use crate::gameplay::ctf::MatchPhase;

        // Red trails on captures, so in closing time it commits to attack rather
        // than protecting a lead it does not hold: its home-most car pushes on.
        let mut app = app_with_system();
        app.insert_resource(CaptureScore {
            player: 1,
            opponents: 0,
        });
        app.insert_resource(MatchClock {
            frames_remaining: 10,
            phase: MatchPhase::Regulation,
        });
        let home_most = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 2000.0)],
            Vec3::new(0.0, 500.0, 4.0),
        );
        spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 2000.0)],
            Vec3::new(0.0, 1500.0, 4.0),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::ZERO,
            Vec3::new(0.0, 0.0, 4.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(0.0, 1000.0),
            Vec3::new(0.0, 1000.0, 4.0),
            None,
        );

        app.update();

        let dy = app.world.get::<Transform>(home_most).unwrap().translation.y - 500.0;
        assert!(
            dy > 0.1,
            "a trailing team commits forward, it is never recalled to camp: {dy}"
        );
    }

    #[test]
    fn moves_towards_a_distant_waypoint() {
        let mut app = app_with_system();
        // Facing +Y by default, waypoint straight ahead.
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.y > 0.0,
            "expected forward movement, y={}",
            transform.translation.y
        );
    }

    #[test]
    fn carrying_the_enemy_flag_slows_a_virtual_player() {
        use crate::gameplay::ctf::FLAG_CARRIER_SPEED_MULTIPLIER;

        // Control: an empty-handed red patroller driving straight at a waypoint.
        let mut free_app = app_with_system();
        let free_ai = spawn_ai_on_team(&mut free_app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
        free_app.update();
        let free_y = free_app
            .world
            .get::<Transform>(free_ai)
            .unwrap()
            .translation
            .y;

        // Carrier: a red car hauling the blue flag runs home to its red base,
        // which sits straight ahead so the heading (and throttle) match the
        // control exactly. Only the flag-carry tax differs.
        let mut carrier_app = app_with_system();
        let carrier = spawn_ai_on_team(&mut carrier_app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut carrier_app,
            FlagTeam::Red,
            Vec2::new(0.0, 1000.0),
            Vec3::new(0.0, 1000.0, 4.0),
            None,
        );
        spawn_flag(
            &mut carrier_app,
            FlagTeam::Blue,
            Vec2::new(0.0, -1000.0),
            Vec3::new(0.0, -1000.0, 4.0),
            Some(carrier),
        );
        carrier_app.update();
        let carrier_y = carrier_app
            .world
            .get::<Transform>(carrier)
            .unwrap()
            .translation
            .y;

        assert!(
            carrier_y > 0.0 && carrier_y < free_y,
            "free={free_y}, carrier={carrier_y}"
        );
        assert!(
            (carrier_y - free_y * FLAG_CARRIER_SPEED_MULTIPLIER).abs() <= 1e-3,
            "carrier should drive at the flag-carrier multiplier: free={free_y}, carrier={carrier_y}"
        );
    }

    #[test]
    fn a_battered_team_parks_its_home_most_car_in_the_pit() {
        // Red home sits at the origin and the blue (enemy) flag straight ahead
        // at +Y, so a healthy red car drives forward to attack it. One frame is
        // run twice: once healthy, once battered.
        fn run(opponent_integrity: f32) -> (f32, f32) {
            let mut app = app_with_system();
            app.insert_resource(VehicleIntegrity {
                player: MAX_INTEGRITY,
                opponent: opponent_integrity,
            });
            // The home-most car spawns exactly on its red base.
            let near = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
            // A distant second red car keeps the team above the lone-car guard.
            let far = spawn_ai_at(
                &mut app,
                vec![Vec2::new(0.0, 1000.0)],
                Vec3::new(0.0, 1500.0, 4.0),
            );
            spawn_flag(
                &mut app,
                FlagTeam::Red,
                Vec2::ZERO,
                Vec3::new(0.0, 0.0, 4.0),
                None,
            );
            spawn_flag(
                &mut app,
                FlagTeam::Blue,
                Vec2::new(0.0, 1000.0),
                Vec3::new(0.0, 1000.0, 4.0),
                None,
            );

            app.update();

            let near_y = app.world.get::<Transform>(near).unwrap().translation.y;
            let far_y = app.world.get::<Transform>(far).unwrap().translation.y;
            (near_y, far_y)
        }

        let (healthy_near, _) = run(MAX_INTEGRITY);
        let (battered_near, battered_far) = run(20.0);

        assert!(
            healthy_near > 1.0,
            "a healthy home car should attack, not idle: {healthy_near}"
        );
        assert!(
            battered_near.abs() < 0.001,
            "a battered home-most car should park in its pit: {battered_near}"
        );
        assert!(
            (battered_far - 1500.0).abs() > 0.001,
            "the distant car keeps playing rather than retreating: {battered_far}"
        );
    }

    #[test]
    fn a_battered_retreating_car_weaves_around_a_blocker_on_its_run_home() {
        // Red home sits at the origin. A battered red team sends its home-most car
        // back to pit-recover from straight above its base. With a stationary enemy
        // planted on the run home it weaves off the line to dodge a ram it can least
        // afford; with the lane clear it limps straight back. One frame loop is run
        // twice to compare the retreating car's sideways drift.
        fn run(with_blocker: bool) -> f32 {
            let mut app = app_with_system();
            app.insert_resource(VehicleIntegrity {
                player: MAX_INTEGRITY,
                opponent: 20.0,
            });
            let near = spawn_ai_at(
                &mut app,
                vec![Vec2::new(0.0, 0.0)],
                Vec3::new(0.0, 600.0, 4.0),
            );
            // A distant second red car keeps the team above the lone-car guard.
            spawn_ai_at(
                &mut app,
                vec![Vec2::new(0.0, 0.0)],
                Vec3::new(0.0, 1400.0, 4.0),
            );
            spawn_flag(
                &mut app,
                FlagTeam::Red,
                Vec2::ZERO,
                Vec3::new(0.0, 0.0, 4.0),
                None,
            );
            spawn_flag(
                &mut app,
                FlagTeam::Blue,
                Vec2::new(0.0, 2000.0),
                Vec3::new(0.0, 2000.0, 4.0),
                None,
            );
            if with_blocker {
                // A stationary enemy dead on the run home, between the retreating
                // car and its base, so it stays a fixed roadblock every frame.
                app.world.spawn((
                    VirtualPlayer {
                        team: AiTeam::Blue,
                        movement_speed: 0.0,
                        rotation_speed: 0.0,
                        waypoints: vec![Vec2::new(0.0, 300.0)],
                        current_waypoint: 0,
                        player_pursuit_radius: TEST_PURSUIT_RADIUS,
                        pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                        corner_throttle: 0.3,
                    },
                    Transform::from_translation(Vec3::new(0.0, 300.0, 4.0)),
                ));
            }

            for _ in 0..20 {
                app.update();
            }

            app.world.get::<Transform>(near).unwrap().translation.x
        }

        let weaved = run(true);
        let straight = run(false);

        assert!(
            straight.abs() < 0.001,
            "with the lane clear the limping car should track straight home: {straight}"
        );
        assert!(
            weaved.abs() > 1.0,
            "with an enemy on the line the limping car should weave off it: {weaved}"
        );
    }

    #[test]
    fn a_lone_battered_car_keeps_playing_instead_of_retreating() {
        let mut app = app_with_system();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            opponent: 10.0,
        });
        // A single red car on its own base, with the enemy flag straight ahead.
        let lone = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::ZERO,
            Vec3::new(0.0, 0.0, 4.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(0.0, 1000.0),
            Vec3::new(0.0, 1000.0, 4.0),
            None,
        );

        app.update();

        let y = app.world.get::<Transform>(lone).unwrap().translation.y;
        assert!(
            y > 1.0,
            "a lone battered car must keep attacking rather than abandon the field: {y}"
        );
    }

    #[test]
    fn a_healthier_team_hunts_a_reeling_enemy() {
        // A red hunter sits at the origin facing +Y with its patrol waypoint
        // straight ahead, while a lone blue car sits straight behind it. The
        // blue team's wear is the variable: healthy blue and the red car drives
        // forward to its waypoint; reeling blue and the red car breaks off,
        // reversing to hunt the battered enemy down.
        fn run(player_integrity: f32) -> f32 {
            let mut app = app_with_system();
            app.insert_resource(VehicleIntegrity {
                player: player_integrity,
                opponent: MAX_INTEGRITY,
            });
            let hunter = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
            // A second red car keeps the team above the lone-car guard and sits
            // far from the prey so the origin car is the one chosen to hunt.
            spawn_ai_at(
                &mut app,
                vec![Vec2::new(0.0, 1000.0)],
                Vec3::new(0.0, 1500.0, 4.0),
            );
            // The blue prey, straight behind the red hunter.
            app.world.spawn((
                VirtualPlayer {
                    team: AiTeam::Blue,
                    movement_speed: 500.0,
                    rotation_speed: f32::to_radians(360.0),
                    waypoints: vec![Vec2::new(0.0, -2000.0)],
                    current_waypoint: 0,
                    player_pursuit_radius: TEST_PURSUIT_RADIUS,
                    pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                    corner_throttle: 0.3,
                },
                Transform::from_translation(Vec3::new(0.0, -1000.0, 4.0)),
            ));

            app.update();

            app.world.get::<Transform>(hunter).unwrap().translation.y
        }

        let healthy = run(MAX_INTEGRITY);
        let reeling = run(20.0);

        assert!(
            healthy > 1.0,
            "against a healthy enemy the red car attacks forward: {healthy}"
        );
        assert!(
            reeling < -0.001,
            "against a reeling enemy the red car breaks off to hunt it down behind: {reeling}"
        );
    }

    #[test]
    fn a_team_with_cars_to_spare_pincers_a_reeling_enemy() {
        // Three healthy red cars against a lone reeling blue prey straight behind
        // them. The two nearest red cars (B nearest, then A at the origin) both
        // break off to gang up on the kill, springing the combat pincer, while the
        // third (C, farthest) stays on the objective. A lone kill press would send
        // only B and leave A driving forward to its waypoint; A reversing to hunt
        // is the tell that the second hunter joined.
        let mut app = app_with_system();
        app.insert_resource(VehicleIntegrity {
            player: 20.0,
            opponent: MAX_INTEGRITY,
        });
        // A: at the origin facing +Y, waypoint ahead. The pincer partner.
        let car_a = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 600.0)]);
        // B: nearest the prey, the primary hunter.
        let car_b = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 600.0)],
            Vec3::new(0.0, -200.0, 4.0),
        );
        // C: farthest from the prey, stays on the objective driving to its waypoint.
        let car_c = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 600.0)],
            Vec3::new(0.0, 400.0, 4.0),
        );
        // The reeling blue prey, straight behind the red cars.
        app.world.spawn((
            VirtualPlayer {
                team: AiTeam::Blue,
                movement_speed: 500.0,
                rotation_speed: f32::to_radians(360.0),
                waypoints: vec![Vec2::new(0.0, -2000.0)],
                current_waypoint: 0,
                player_pursuit_radius: TEST_PURSUIT_RADIUS,
                pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                corner_throttle: 0.3,
            },
            Transform::from_translation(Vec3::new(0.0, -500.0, 4.0)),
        ));

        app.update();

        let a_y = app.world.get::<Transform>(car_a).unwrap().translation.y;
        let b_y = app.world.get::<Transform>(car_b).unwrap().translation.y;
        let c_y = app.world.get::<Transform>(car_c).unwrap().translation.y;

        assert!(
            b_y < -200.0,
            "the primary hunter breaks off to chase the prey behind it: {b_y}"
        );
        assert!(
            a_y < 0.0,
            "the spare car joins the pincer, reversing to gang up rather than driving its route: {a_y}"
        );
        assert!(
            c_y > 400.0,
            "the farthest car stays on the objective, never abandoning the field: {c_y}"
        );
    }

    #[test]
    fn a_reeling_team_does_not_over_commit_to_a_kill() {
        // Both teams are battered but level. Neither is the healthier side, so
        // the red car keeps attacking its waypoint rather than trading itself
        // into a mutual wreck chasing the blue car behind it.
        let mut app = app_with_system();
        app.insert_resource(VehicleIntegrity {
            player: 20.0,
            opponent: 20.0,
        });
        let red = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
        spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(0.0, 1500.0, 4.0),
        );
        app.world.spawn((
            VirtualPlayer {
                team: AiTeam::Blue,
                movement_speed: 500.0,
                rotation_speed: f32::to_radians(360.0),
                waypoints: vec![Vec2::new(0.0, -2000.0)],
                current_waypoint: 0,
                player_pursuit_radius: TEST_PURSUIT_RADIUS,
                pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                corner_throttle: 0.3,
            },
            Transform::from_translation(Vec3::new(0.0, -1000.0, 4.0)),
        ));

        app.update();

        let y = app.world.get::<Transform>(red).unwrap().translation.y;
        assert!(
            y > 1.0,
            "a team that is no healthier than its enemy keeps playing the objective: {y}"
        );
    }

    #[test]
    fn a_team_trailing_on_captures_hunts_the_reeling_leader_at_even_health() {
        // Both teams are equally battered, so durability alone keeps the red car
        // on its objective (see `a_reeling_team_does_not_over_commit_to_a_kill`).
        // The capture scoreline is the variable: once red trails blue it takes
        // the even-health gamble and breaks off to hunt the blue car behind it,
        // the AI mirror of the most-wanted comeback bounty.
        fn run(blue_captures: u32, red_captures: u32) -> f32 {
            let mut app = app_with_system();
            app.insert_resource(VehicleIntegrity {
                player: 20.0,
                opponent: 20.0,
            });
            app.insert_resource(CaptureScore {
                player: blue_captures,
                opponents: red_captures,
            });
            let red = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
            // A second red car keeps the team above the lone-car guard and sits
            // far from the prey so the origin car is the one chosen to hunt.
            spawn_ai_at(
                &mut app,
                vec![Vec2::new(0.0, 1000.0)],
                Vec3::new(0.0, 1500.0, 4.0),
            );
            // The reeling blue prey, straight behind the red hunter.
            app.world.spawn((
                VirtualPlayer {
                    team: AiTeam::Blue,
                    movement_speed: 500.0,
                    rotation_speed: f32::to_radians(360.0),
                    waypoints: vec![Vec2::new(0.0, -2000.0)],
                    current_waypoint: 0,
                    player_pursuit_radius: TEST_PURSUIT_RADIUS,
                    pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                    corner_throttle: 0.3,
                },
                Transform::from_translation(Vec3::new(0.0, -1000.0, 4.0)),
            ));

            app.update();

            app.world.get::<Transform>(red).unwrap().translation.y
        }

        let level = run(0, 0);
        let trailing = run(2, 0);

        assert!(
            level > 1.0,
            "level on captures the even-health red car keeps to its objective: {level}"
        );
        assert!(
            trailing < -0.001,
            "trailing on captures the red car breaks off to hunt the leader down: {trailing}"
        );
    }

    fn assert_arrive_radius_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= f32::EPSILON,
            "actual={actual}, expected={expected}"
        );
    }

    #[test]
    fn ram_targets_arrive_tighter_than_positional_ones() {
        // Ramming an enemy car means driving through it, so chase targets close
        // far tighter than the waypoint/positional boundary. The tighter < wider
        // invariant itself is enforced at compile time on PURSUIT_ARRIVE_RADIUS.
        assert_arrive_radius_eq(
            arrive_radius_for_target(DrivingTarget::FinishWreck(Vec2::ZERO)),
            PURSUIT_ARRIVE_RADIUS,
        );
        assert_arrive_radius_eq(
            arrive_radius_for_target(DrivingTarget::Player(Vec2::ZERO)),
            PURSUIT_ARRIVE_RADIUS,
        );
        assert_arrive_radius_eq(
            arrive_radius_for_target(DrivingTarget::PatrolWaypoint(Vec2::ZERO)),
            WAYPOINT_ARRIVE_RADIUS,
        );
        assert_arrive_radius_eq(
            arrive_radius_for_target(DrivingTarget::EnemyFlag(Vec2::ZERO)),
            WAYPOINT_ARRIVE_RADIUS,
        );
    }

    #[test]
    fn a_hunter_drives_through_to_ram_a_reeling_enemy_at_close_range() {
        // A red hunter sits at the origin facing +Y with a patrol waypoint
        // straight ahead; a reeling blue car sits just 60 units behind it, well
        // inside the wide waypoint arrive radius yet outside true ram range. The
        // hunter must keep driving back into the prey to land the wreck rather
        // than coasting to an idle at the arrive boundary, short of contact.
        let mut app = app_with_system();
        app.insert_resource(VehicleIntegrity {
            player: 20.0,
            opponent: MAX_INTEGRITY,
        });
        let hunter = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
        // A distant second red car keeps the team above the lone-car guard and
        // sits far from the prey so the origin car is the chosen hunter.
        spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(0.0, 1500.0, 4.0),
        );
        // The reeling blue prey, a close 60 units behind the red hunter: nearer
        // than WAYPOINT_ARRIVE_RADIUS but further than PURSUIT_ARRIVE_RADIUS.
        app.world.spawn((
            VirtualPlayer {
                team: AiTeam::Blue,
                movement_speed: 500.0,
                rotation_speed: f32::to_radians(360.0),
                waypoints: vec![Vec2::new(0.0, -2000.0)],
                current_waypoint: 0,
                player_pursuit_radius: TEST_PURSUIT_RADIUS,
                pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                corner_throttle: 0.3,
            },
            Transform::from_translation(Vec3::new(0.0, -60.0, 4.0)),
        ));

        app.update();

        let y = app.world.get::<Transform>(hunter).unwrap().translation.y;
        assert!(
            y < -0.001,
            "a hunter must drive through to ram a close reeling enemy, not idle short of it: {y}"
        );
    }

    #[test]
    fn a_hunter_shoves_a_wall_pinned_prey_into_the_boundary() {
        // A reeling blue prey hugs the +x wall (x = 920, inside the crush band);
        // the red hunter sits directly below it on the open side, facing +Y. Aiming
        // straight at the prey would carry the hunter due north (no sideways drift);
        // aiming past it into the wall instead bends the charge toward +x, so a
        // rightward nudge is the tell that the kill press is setting up a wall crush.
        let mut app = app_with_system();
        app.insert_resource(VehicleIntegrity {
            player: 20.0,
            opponent: MAX_INTEGRITY,
        });
        let hunter_start_x = 920.0;
        let hunter = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(hunter_start_x, -800.0, 4.0),
        );
        // A distant second red car keeps the team above the lone-car guard and sits
        // far from the prey so the lower car is the chosen hunter.
        spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(0.0, 3000.0, 4.0),
        );
        // The reeling blue prey, pinned against the +x wall straight above the hunter.
        app.world.spawn((
            VirtualPlayer {
                team: AiTeam::Blue,
                movement_speed: 500.0,
                rotation_speed: f32::to_radians(360.0),
                waypoints: vec![Vec2::new(hunter_start_x, -2000.0)],
                current_waypoint: 0,
                player_pursuit_radius: TEST_PURSUIT_RADIUS,
                pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                corner_throttle: 0.3,
            },
            Transform::from_translation(Vec3::new(hunter_start_x, -300.0, 4.0)),
        ));

        app.update();

        let x = app.world.get::<Transform>(hunter).unwrap().translation.x;
        assert!(
            x > hunter_start_x + 1e-3,
            "a hunter must bend its charge toward the wall to crush a pinned prey, \
             not drive straight at it: {x}"
        );
    }

    #[test]
    fn reverses_towards_a_waypoint_behind_the_car() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, -1000.0)]);

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.y < 0.0,
            "expected reverse movement, y={}",
            transform.translation.y
        );
    }

    #[test]
    fn finished_match_stops_virtual_players() {
        let mut app = app_with_system();
        app.insert_resource(CtfMatchResult {
            winner: Some(CtfMatchWinner::Player),
        });
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert_eq!(transform.translation, Vec3::new(0.0, 0.0, 4.0));
    }

    #[test]
    fn advances_waypoint_once_arrived() {
        let mut app = app_with_system();
        // Start already on top of the first waypoint so it should advance.
        let ai = spawn_ai(&mut app, vec![Vec2::ZERO, Vec2::new(500.0, 0.0)]);

        app.update();

        let vp = app.world.get::<VirtualPlayer>(ai).unwrap();
        assert_eq!(vp.current_waypoint, 1);
    }

    #[test]
    fn stays_within_arena_bounds() {
        let mut app = app_with_system();
        let edge = Vec3::new(BOUNDS.x / 2.0, BOUNDS.y / 2.0, 4.0);
        let ai = app
            .world
            .spawn((
                VirtualPlayer {
                    team: AiTeam::Red,
                    movement_speed: 5000.0,
                    rotation_speed: f32::to_radians(360.0),
                    waypoints: vec![Vec2::new(BOUNDS.x, BOUNDS.y)],
                    current_waypoint: 0,
                    player_pursuit_radius: TEST_PURSUIT_RADIUS,
                    pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                    corner_throttle: 0.3,
                },
                Transform::from_translation(edge),
            ))
            .id();

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(transform.translation.x <= BOUNDS.x / 2.0 + 1e-3);
        assert!(transform.translation.y <= BOUNDS.y / 2.0 + 1e-3);
    }

    #[test]
    fn idle_ai_without_waypoints_does_not_panic() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![]);

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert_eq!(transform.translation, Vec3::new(0.0, 0.0, 4.0));
    }

    #[test]
    fn pursues_nearby_pickup_before_patrol_waypoint() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
        ));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected opponent to turn towards pickup, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn pursues_nearby_player_before_patrol_waypoint() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_player(&mut app, Vec3::new(200.0, 0.0, 5.0));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected opponent to turn towards player, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn an_eager_personality_hunts_a_player_a_cautious_one_leaves_alone() {
        // Same player, same patrol route, two different driving personalities. The
        // human sits 300 units to the right; the patrol waypoint is far up the
        // y-axis the car already faces. The drive system must honour each car's own
        // pursuit radius, not a shared global, so eagerness is a genuine
        // personality trait rather than uniform across the roster.
        let player = Vec3::new(300.0, 0.0, 5.0);

        // Cautious technician-style car: 200-unit reach falls short of the player,
        // so it stays disciplined and keeps lapping its patrol route.
        let mut cautious_app = app_with_system();
        let cautious = spawn_ai_with_pursuit(
            &mut cautious_app,
            AiTeam::Red,
            vec![Vec2::new(0.0, 1000.0)],
            200.0,
        );
        spawn_player(&mut cautious_app, player);
        cautious_app.update();
        let cautious_transform = cautious_app.world.get::<Transform>(cautious).unwrap();
        assert!(
            cautious_transform.translation.x.abs() < 1e-4,
            "a cautious driver leaves a player beyond its reach alone, x={}",
            cautious_transform.translation.x
        );
        assert!(
            cautious_transform.translation.y > 0.0,
            "a cautious driver keeps lapping its patrol route, y={}",
            cautious_transform.translation.y
        );

        // Eager sprinter-style car: 400-unit reach covers the same player, so it
        // breaks off the route to run the player down.
        let mut eager_app = app_with_system();
        let eager = spawn_ai_with_pursuit(
            &mut eager_app,
            AiTeam::Red,
            vec![Vec2::new(0.0, 1000.0)],
            400.0,
        );
        spawn_player(&mut eager_app, player);
        eager_app.update();
        let eager_transform = eager_app.world.get::<Transform>(eager).unwrap();
        assert!(
            eager_transform.translation.x > 0.0,
            "an eager driver runs down a player within its reach, x={}",
            eager_transform.translation.x
        );
    }

    #[test]
    fn a_greedy_personality_scavenges_a_pickup_a_disciplined_one_ignores() {
        // Same pickup, same patrol route, two different driving personalities. A
        // cash bag sits 480 units to the right, just outside the former uniform
        // 450-unit reach; the patrol waypoint is far up the y-axis the car already
        // faces. The drive system must honour each car's own pickup-scavenging
        // radius, not a shared global, so greed is a genuine personality trait
        // rather than uniform across the roster.
        fn spawn_cash(app: &mut App, position: Vec3) {
            app.world.spawn((
                Pickup {
                    kind: PickupKind::Cash,
                },
                Transform::from_translation(position),
            ));
        }
        let pickup = Vec3::new(480.0, 0.0, 2.0);

        // Disciplined technician-style car: 380-unit greed falls short of the bag,
        // so it stays on its line and keeps lapping its patrol route.
        let mut disciplined_app = app_with_system();
        let disciplined =
            spawn_ai_with_pickup_pursuit(&mut disciplined_app, vec![Vec2::new(0.0, 1000.0)], 380.0);
        spawn_cash(&mut disciplined_app, pickup);
        disciplined_app.update();
        let disciplined_transform = disciplined_app.world.get::<Transform>(disciplined).unwrap();
        assert!(
            disciplined_transform.translation.x.abs() < 1e-4,
            "a disciplined driver leaves a pickup beyond its reach alone, x={}",
            disciplined_transform.translation.x
        );
        assert!(
            disciplined_transform.translation.y > 0.0,
            "a disciplined driver keeps lapping its patrol route, y={}",
            disciplined_transform.translation.y
        );

        // Greedy sprinter-style car: 520-unit greed covers the same bag, so it
        // breaks off the route to scavenge it.
        let mut greedy_app = app_with_system();
        let greedy =
            spawn_ai_with_pickup_pursuit(&mut greedy_app, vec![Vec2::new(0.0, 1000.0)], 520.0);
        spawn_cash(&mut greedy_app, pickup);
        greedy_app.update();
        let greedy_transform = greedy_app.world.get::<Transform>(greedy).unwrap();
        assert!(
            greedy_transform.translation.x > 0.0,
            "a greedy driver breaks off to scavenge a pickup within its reach, x={}",
            greedy_transform.translation.x
        );
    }

    #[test]
    fn blue_virtual_player_does_not_chase_human_teammate() {
        let mut app = app_with_system();
        let ai = spawn_ai_on_team(&mut app, AiTeam::Blue, vec![Vec2::new(0.0, 1000.0)]);
        spawn_player(&mut app, Vec3::new(200.0, 0.0, 5.0));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x.abs() < 1e-4,
            "expected blue teammate to stay on patrol, x={}",
            transform.translation.x
        );
        assert!(
            transform.translation.y > 0.0,
            "expected blue teammate to keep moving, y={}",
            transform.translation.y
        );
    }

    #[test]
    fn blue_virtual_player_leaves_player_claimed_pickup_alone() {
        let mut app = app_with_system();
        let ai = spawn_ai_on_team(&mut app, AiTeam::Blue, vec![Vec2::new(0.0, 1000.0)]);
        spawn_player(&mut app, Vec3::new(180.0, 0.0, 5.0));
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
        ));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x.abs() < 1e-4,
            "expected teammate to leave player-claimed pickup alone, x={}",
            transform.translation.x
        );
        assert!(
            transform.translation.y > 0.0,
            "expected teammate to keep patrolling, y={}",
            transform.translation.y
        );
    }

    #[test]
    fn blue_virtual_player_does_not_claim_pickup_when_player_is_closer() {
        let pickup = PickupTarget {
            position: Vec2::new(200.0, 0.0),
            priority: 100,
        };

        let yields = virtual_player_yields_player_pickup_claim(
            AiTeam::Blue,
            Some(Vec2::new(180.0, 0.0)),
            pickup,
            Vec2::ZERO,
        );

        assert!(yields);
    }

    #[test]
    fn red_virtual_player_claims_pickup_even_when_player_is_closer() {
        let pickup = PickupTarget {
            position: Vec2::new(200.0, 0.0),
            priority: 100,
        };

        let yields = virtual_player_yields_player_pickup_claim(
            AiTeam::Red,
            Some(Vec2::new(180.0, 0.0)),
            pickup,
            Vec2::ZERO,
        );

        assert!(!yields);
    }

    #[test]
    fn red_virtual_player_contests_player_claimed_pickup() {
        let mut app = app_with_system();
        let ai = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
        spawn_player(&mut app, Vec3::new(180.0, 0.0, 5.0));
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
        ));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected opponent to contest player-claimed pickup, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn pickup_stays_higher_priority_than_player_chase() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_player(&mut app, Vec3::new(200.0, 0.0, 5.0));
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Repair,
            },
            Transform::from_translation(Vec3::new(-200.0, 0.0, 2.0)),
        ));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x < 0.0,
            "expected opponent to prioritise pickup, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn pursues_richer_pickup_before_closer_pickup() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Repair,
            },
            Transform::from_translation(Vec3::new(-25.0, 0.0, 2.0)),
        ));
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(150.0, 0.0, 2.0)),
        ));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected opponent to turn towards richer pickup, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn pursues_nitro_before_cash_for_race_pressure() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(-25.0, 0.0, 2.0)),
        ));
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Nitro,
            },
            Transform::from_translation(Vec3::new(150.0, 0.0, 2.0)),
        ));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected opponent to prioritise nitro pressure, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn only_one_virtual_player_pursues_a_shared_pickup() {
        let mut app = app_with_system();
        let first_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let second_ai = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(0.0, 100.0, 4.0),
        );
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
        ));

        app.update();

        let first_transform = app.world.get::<Transform>(first_ai).unwrap();
        let second_transform = app.world.get::<Transform>(second_ai).unwrap();

        assert!(
            first_transform.translation.x > 0.0,
            "expected first opponent to claim pickup, x={}",
            first_transform.translation.x
        );
        assert!(
            second_transform.translation.x.abs() < 1e-4,
            "expected second opponent to keep patrol line, x={}",
            second_transform.translation.x
        );
        assert!(
            second_transform.translation.y > 100.0,
            "expected second opponent to keep moving, y={}",
            second_transform.translation.y
        );
    }

    #[test]
    fn nearby_teammates_spread_out_before_patrolling() {
        let mut app = app_with_system();
        let left_ai = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(0.0, 0.0, 4.0),
        );
        let right_ai = spawn_ai_at(
            &mut app,
            vec![Vec2::new(40.0, 1000.0)],
            Vec3::new(40.0, 0.0, 4.0),
        );

        app.update();

        let left_transform = app.world.get::<Transform>(left_ai).unwrap();
        let right_transform = app.world.get::<Transform>(right_ai).unwrap();

        assert!(
            left_transform.translation.x < 0.0,
            "expected left teammate to steer away, x={}",
            left_transform.translation.x
        );
        assert!(
            right_transform.translation.x > 40.0,
            "expected right teammate to steer away, x={}",
            right_transform.translation.x
        );
    }

    #[test]
    fn closest_virtual_player_claims_shared_pickup_even_if_spawned_later() {
        let mut app = app_with_system();
        let far_ai = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(0.0, 0.0, 4.0),
        );
        let close_ai = spawn_ai_at(
            &mut app,
            vec![Vec2::new(120.0, 1000.0)],
            Vec3::new(120.0, 0.0, 4.0),
        );
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(220.0, 0.0, 2.0)),
        ));

        app.update();

        let far_transform = app.world.get::<Transform>(far_ai).unwrap();
        let close_transform = app.world.get::<Transform>(close_ai).unwrap();

        assert!(
            far_transform.translation.x.abs() < 1e-4,
            "expected farther opponent to keep patrol line, x={}",
            far_transform.translation.x
        );
        assert!(
            far_transform.translation.y > 0.0,
            "expected farther opponent to keep moving, y={}",
            far_transform.translation.y
        );
        assert!(
            close_transform.translation.x > 120.0,
            "expected closer opponent to claim pickup, x={}",
            close_transform.translation.x
        );
    }

    #[test]
    fn second_virtual_player_claims_next_pickup_when_closest_ai_is_busy() {
        let mut app = app_with_system();
        let close_ai = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(0.0, 0.0, 4.0),
        );
        let far_ai = spawn_ai_at(
            &mut app,
            vec![Vec2::new(300.0, 1000.0)],
            Vec3::new(300.0, 0.0, 4.0),
        );
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Nitro,
            },
            Transform::from_translation(Vec3::new(-150.0, 0.0, 2.0)),
        ));
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(80.0, 0.0, 2.0)),
        ));

        app.update();

        let close_transform = app.world.get::<Transform>(close_ai).unwrap();
        let far_transform = app.world.get::<Transform>(far_ai).unwrap();

        assert!(
            close_transform.translation.x < 0.0,
            "expected closest opponent to take high-value nitro, x={}",
            close_transform.translation.x
        );
        assert!(
            far_transform.translation.x < 300.0,
            "expected second opponent to claim remaining cash, x={}",
            far_transform.translation.x
        );
    }

    #[test]
    fn only_one_virtual_player_intercepts_home_flag_threat() {
        let first = CtfTargetCandidate {
            entity: Entity::from_raw(1),
            team: AiTeam::Red,
            position: Vec2::new(350.0, 0.0),
            target: DrivingTarget::DefendHomeBase(Vec2::new(360.0, 0.0)),
            home_base: Vec2::new(500.0, 0.0),
            carries_enemy_flag: false,
        };
        let second = CtfTargetCandidate {
            entity: Entity::from_raw(2),
            team: AiTeam::Red,
            position: Vec2::new(0.0, 0.0),
            target: DrivingTarget::DefendHomeBase(Vec2::new(360.0, 0.0)),
            home_base: Vec2::new(500.0, 0.0),
            carries_enemy_flag: false,
        };
        let assignments = assign_ctf_targets(
            &[first, second],
            &[FlagTarget {
                team: AiTeam::Red,
                home: Vec2::new(500.0, 0.0),
                position: Vec2::new(500.0, 0.0),
                holder: None,
            }],
        );

        assert_eq!(
            assignments,
            vec![
                (
                    Entity::from_raw(1),
                    Some(DrivingTarget::DefendHomeBase(Vec2::new(360.0, 0.0)))
                ),
                (
                    Entity::from_raw(2),
                    Some(DrivingTarget::DefendHomeBase(Vec2::new(500.0, 0.0)))
                ),
            ]
        );
    }

    #[test]
    fn spare_defender_guards_home_flag_lane() {
        let attacker = CtfTargetCandidate {
            entity: Entity::from_raw(1),
            team: AiTeam::Red,
            position: Vec2::new(-300.0, 0.0),
            target: DrivingTarget::EnemyFlag(Vec2::new(-500.0, 0.0)),
            home_base: Vec2::new(500.0, 0.0),
            carries_enemy_flag: false,
        };
        let spare = CtfTargetCandidate {
            entity: Entity::from_raw(2),
            team: AiTeam::Red,
            position: Vec2::new(450.0, 0.0),
            target: DrivingTarget::EnemyFlag(Vec2::new(-500.0, 0.0)),
            home_base: Vec2::new(500.0, 0.0),
            carries_enemy_flag: false,
        };
        let assignments = assign_ctf_targets(
            &[attacker, spare],
            &[
                FlagTarget {
                    team: AiTeam::Red,
                    home: Vec2::new(500.0, 0.0),
                    position: Vec2::new(500.0, 0.0),
                    holder: None,
                },
                FlagTarget {
                    team: AiTeam::Blue,
                    home: Vec2::new(-500.0, 0.0),
                    position: Vec2::new(-500.0, 0.0),
                    holder: None,
                },
            ],
        );

        assert_eq!(
            assignments,
            vec![
                (
                    Entity::from_raw(1),
                    Some(DrivingTarget::EnemyFlag(Vec2::new(-500.0, 0.0)))
                ),
                (
                    Entity::from_raw(2),
                    Some(DrivingTarget::DefendHomeBase(Vec2::new(280.0, 0.0)))
                ),
            ]
        );
    }

    #[test]
    fn equal_distance_ctf_role_assignment_uses_position_tiebreakers() {
        let left = CtfTargetCandidate {
            entity: Entity::from_raw(1),
            team: AiTeam::Red,
            position: Vec2::new(-50.0, 0.0),
            target: DrivingTarget::EnemyFlag(Vec2::ZERO),
            home_base: Vec2::new(500.0, 0.0),
            carries_enemy_flag: false,
        };
        let right = CtfTargetCandidate {
            entity: Entity::from_raw(2),
            team: AiTeam::Red,
            position: Vec2::new(50.0, 0.0),
            target: DrivingTarget::EnemyFlag(Vec2::ZERO),
            home_base: Vec2::new(500.0, 0.0),
            carries_enemy_flag: false,
        };
        let flags = [FlagTarget {
            team: AiTeam::Red,
            home: Vec2::new(500.0, 0.0),
            position: Vec2::new(500.0, 0.0),
            holder: None,
        }];

        let assignments = assign_ctf_targets(&[right, left], &flags);

        assert_eq!(
            assignments,
            vec![
                (
                    Entity::from_raw(2),
                    Some(DrivingTarget::DefendHomeBase(Vec2::new(500.0, 0.0)))
                ),
                (
                    Entity::from_raw(1),
                    Some(DrivingTarget::EnemyFlag(Vec2::ZERO))
                ),
            ]
        );
    }

    #[test]
    fn pursues_blue_flag_before_pickup_or_patrol_waypoint() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-200.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
        ));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x < 0.0,
            "expected opponent to turn towards blue flag, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn flag_carrier_ignores_pickup_behind_route_home() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
            Some(ai),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(-200.0, 0.0, 2.0)),
        ));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected flag carrier to turn towards home base, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn defends_red_flag_when_player_is_about_to_steal_it() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_player(&mut app, Vec3::new(250.0, 0.0, 5.0));
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-400.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected opponent to defend the threatened red flag, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn blue_virtual_player_pursues_red_flag() {
        let mut app = app_with_system();
        let ai = spawn_ai_on_team(&mut app, AiTeam::Blue, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-500.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(200.0, 0.0, 2.0),
            None,
        );

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected blue opponent to turn towards red flag, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn attacker_detours_for_closer_pickup_along_blue_flag_push() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-400.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(-100.0, 0.0, 2.0)),
        ));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x < 0.0,
            "expected attacker to stay on the flag-side pickup lane, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn flag_carrier_returns_to_red_base() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
            Some(ai),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected flag carrier to turn towards red base, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn flag_carrier_stages_outside_contested_red_base() {
        let mut app = app_with_system();
        let ai = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(500.0, 0.0, 4.0),
        );
        spawn_player(&mut app, Vec3::new(500.0, 120.0, 5.0));
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
            Some(ai),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.y < 0.0,
            "expected flag carrier to stage away from red-base contest, y={}",
            transform.translation.y
        );
    }

    #[test]
    fn teammate_clears_contested_red_base_for_flag_carrier() {
        let mut app = app_with_system();
        let carrier = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(300.0, 0.0, 4.0),
        );
        let defender = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(560.0, 0.0, 4.0),
        );
        spawn_player(&mut app, Vec3::new(430.0, 0.0, 5.0));
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(300.0, 0.0, 2.0),
            Some(carrier),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );

        app.update();

        let carrier_transform = app.world.get::<Transform>(carrier).unwrap();
        let defender_transform = app.world.get::<Transform>(defender).unwrap();
        assert!(
            carrier_transform.translation.x > 300.0,
            "expected carrier to keep pushing home, x={}",
            carrier_transform.translation.x
        );
        assert!(
            defender_transform.translation.x < 560.0,
            "expected teammate to clear base contester, x={}",
            defender_transform.translation.x
        );
    }

    #[test]
    fn flag_carrier_intercepts_stolen_home_flag_before_scoring() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let player = spawn_player(&mut app, Vec3::new(-800.0, 0.0, 5.0));
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
            Some(ai),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(-800.0, 0.0, 2.0),
            Some(player),
        );

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x < 0.0,
            "expected flag carrier to intercept stolen home flag, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn teammate_defends_stolen_home_flag_before_flag_carrier() {
        let mut app = app_with_system();
        let carrier = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let defender = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(0.0, 50.0, 4.0),
        );
        let player = spawn_player(&mut app, Vec3::new(-800.0, 0.0, 5.0));
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
            Some(carrier),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(-800.0, 0.0, 2.0),
            Some(player),
        );

        app.update();

        let carrier_transform = app.world.get::<Transform>(carrier).unwrap();
        let defender_transform = app.world.get::<Transform>(defender).unwrap();
        assert!(
            carrier_transform.translation.x > 0.0,
            "flag carrier should wait on the scoring route, x={}",
            carrier_transform.translation.x
        );
        assert!(
            defender_transform.translation.x < 0.0,
            "free teammate should chase the stolen home flag, x={}",
            defender_transform.translation.x
        );
    }

    #[test]
    fn teammate_escorts_flag_carrier_before_pickup_or_patrol_waypoint() {
        let mut app = app_with_system();
        let carrier = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let escort = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-200.0, 0.0, 2.0),
            Some(carrier),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
        ));

        app.update();

        let transform = app.world.get::<Transform>(escort).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected escort to lead the carrier towards home, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn teammate_blocks_nearby_flag_carrier_pursuer() {
        let mut app = app_with_system();
        let carrier = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(-120.0, 0.0, 4.0),
        );
        let blocker = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_player(&mut app, Vec3::new(-240.0, 0.0, 5.0));
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-120.0, 0.0, 2.0),
            Some(carrier),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );

        app.update();

        let blocker_transform = app.world.get::<Transform>(blocker).unwrap();
        assert!(
            blocker_transform.translation.x < 0.0,
            "expected teammate to block the pursuer, x={}",
            blocker_transform.translation.x
        );
    }

    #[test]
    fn only_one_teammate_escorts_flag_carrier() {
        let mut app = app_with_system();
        let carrier = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let first_escort = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let second_escort = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-200.0, 0.0, 2.0),
            Some(carrier),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );

        app.update();

        let first_transform = app.world.get::<Transform>(first_escort).unwrap();
        let second_transform = app.world.get::<Transform>(second_escort).unwrap();

        assert!(
            first_transform.translation.x > 0.0,
            "expected first teammate to lead the carrier home, x={}",
            first_transform.translation.x
        );
        assert!(
            second_transform.translation.x > 0.0,
            "expected spare teammate to defend the red base, x={}",
            second_transform.translation.x
        );
    }

    #[test]
    fn defender_intercepts_stolen_red_flag_before_enemy_flag() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let player = spawn_player(&mut app, Vec3::new(300.0, 0.0, 5.0));
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-200.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(200.0, 0.0, 2.0),
            Some(player),
        );

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected defender to cut off the stolen red flag, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn spare_defender_screens_stolen_home_flag_route() {
        let flag_hunter = CtfTargetCandidate {
            entity: Entity::from_raw(1),
            team: AiTeam::Red,
            position: Vec2::new(-700.0, 0.0),
            target: DrivingTarget::StolenHomeFlag(Vec2::new(-660.0, 0.0)),
            home_base: Vec2::new(500.0, 0.0),
            carries_enemy_flag: false,
        };
        let route_screen = CtfTargetCandidate {
            entity: Entity::from_raw(2),
            team: AiTeam::Red,
            position: Vec2::new(-100.0, 0.0),
            target: DrivingTarget::StolenHomeFlag(Vec2::new(-660.0, 0.0)),
            home_base: Vec2::new(500.0, 0.0),
            carries_enemy_flag: false,
        };
        let home_guard = CtfTargetCandidate {
            entity: Entity::from_raw(3),
            team: AiTeam::Red,
            position: Vec2::new(450.0, 0.0),
            target: DrivingTarget::StolenHomeFlag(Vec2::new(-660.0, 0.0)),
            home_base: Vec2::new(500.0, 0.0),
            carries_enemy_flag: false,
        };
        let assignments = assign_ctf_targets(
            &[flag_hunter, route_screen, home_guard],
            &[
                FlagTarget {
                    team: AiTeam::Red,
                    home: Vec2::new(500.0, 0.0),
                    position: Vec2::new(-800.0, 0.0),
                    holder: Some(Entity::from_raw(42)),
                },
                FlagTarget {
                    team: AiTeam::Blue,
                    home: Vec2::new(-500.0, 0.0),
                    position: Vec2::new(-500.0, 0.0),
                    holder: None,
                },
            ],
        );

        assert_eq!(
            assignments,
            vec![
                (
                    Entity::from_raw(1),
                    Some(DrivingTarget::StolenHomeFlag(Vec2::new(-660.0, 0.0)))
                ),
                (
                    Entity::from_raw(2),
                    Some(DrivingTarget::StolenHomeFlagRouteGuard(Vec2::new(
                        -150.0, 0.0
                    )))
                ),
                (
                    Entity::from_raw(3),
                    Some(DrivingTarget::DefendHomeBase(Vec2::new(280.0, 0.0)))
                ),
            ]
        );
    }

    #[test]
    fn defender_intercepts_current_carrier_for_held_home_flag() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let player = spawn_player(&mut app, Vec3::new(250.0, 0.0, 5.0));
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-200.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(-250.0, 0.0, 2.0),
            Some(player),
        );

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected defender to cut off the current carrier, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn only_one_virtual_player_intercepts_stolen_red_flag() {
        let mut app = app_with_system();
        let first_ai = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(0.0, -50.0, 4.0),
        );
        let second_ai = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(0.0, 50.0, 4.0),
        );
        let player = spawn_player(&mut app, Vec3::new(-800.0, 0.0, 5.0));
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-500.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(-800.0, 0.0, 2.0),
            Some(player),
        );

        app.update();

        let first_transform = app.world.get::<Transform>(first_ai).unwrap();
        let second_transform = app.world.get::<Transform>(second_ai).unwrap();
        assert!(
            first_transform.translation.x < 0.0,
            "first opponent should hunt the flag carrier, x={}",
            first_transform.translation.x
        );
        assert!(
            second_transform.translation.x < 0.0,
            "second opponent should screen the stolen-flag route, x={}",
            second_transform.translation.x
        );
    }

    #[test]
    fn only_one_virtual_player_pursues_a_shared_enemy_flag() {
        let mut app = app_with_system();
        let first_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let second_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-200.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
        ));

        app.update();

        let first_transform = app.world.get::<Transform>(first_ai).unwrap();
        let second_transform = app.world.get::<Transform>(second_ai).unwrap();

        assert!(
            first_transform.translation.x < 0.0,
            "expected first opponent to claim the blue flag, x={}",
            first_transform.translation.x
        );
        assert!(
            second_transform.translation.x > 0.0,
            "expected second opponent to race for another objective, x={}",
            second_transform.translation.x
        );
    }

    #[test]
    fn spare_attacker_defends_home_base_when_enemy_flag_is_claimed() {
        let mut app = app_with_system();
        let first_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let second_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-200.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );

        app.update();

        let first_transform = app.world.get::<Transform>(first_ai).unwrap();
        let second_transform = app.world.get::<Transform>(second_ai).unwrap();

        assert!(
            first_transform.translation.x < 0.0,
            "expected first opponent to claim the blue flag, x={}",
            first_transform.translation.x
        );
        assert!(
            second_transform.translation.x > 0.0,
            "expected spare opponent to defend the red base, x={}",
            second_transform.translation.x
        );
    }

    #[test]
    fn extra_spare_virtual_player_blocks_midfield_lane() {
        let attacker = CtfTargetCandidate {
            entity: Entity::from_raw(1),
            team: AiTeam::Red,
            position: Vec2::new(-300.0, 0.0),
            target: DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)),
            home_base: Vec2::new(500.0, 0.0),
            carries_enemy_flag: false,
        };
        let close_spare = CtfTargetCandidate {
            entity: Entity::from_raw(2),
            team: AiTeam::Red,
            position: Vec2::new(450.0, 0.0),
            target: DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)),
            home_base: Vec2::new(500.0, 0.0),
            carries_enemy_flag: false,
        };
        let far_spare = CtfTargetCandidate {
            entity: Entity::from_raw(3),
            team: AiTeam::Red,
            position: Vec2::new(0.0, 0.0),
            target: DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)),
            home_base: Vec2::new(500.0, 0.0),
            carries_enemy_flag: false,
        };
        let assignments = assign_ctf_targets(
            &[attacker, close_spare, far_spare],
            &[
                FlagTarget {
                    team: AiTeam::Red,
                    home: Vec2::new(500.0, 0.0),
                    position: Vec2::new(500.0, 0.0),
                    holder: None,
                },
                FlagTarget {
                    team: AiTeam::Blue,
                    home: Vec2::new(-500.0, 0.0),
                    position: Vec2::new(-400.0, 0.0),
                    holder: None,
                },
            ],
        );

        assert_eq!(
            assignments,
            vec![
                (
                    Entity::from_raw(1),
                    Some(DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)))
                ),
                (
                    Entity::from_raw(2),
                    Some(DrivingTarget::DefendHomeBase(Vec2::new(280.0, 0.0)))
                ),
                (
                    Entity::from_raw(3),
                    Some(DrivingTarget::MidfieldInterceptor(Vec2::ZERO))
                ),
            ]
        );
    }

    #[test]
    fn fourth_spare_virtual_player_flanks_enemy_flag() {
        let attacker = CtfTargetCandidate {
            entity: Entity::from_raw(1),
            team: AiTeam::Red,
            position: Vec2::new(-360.0, 0.0),
            target: DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)),
            home_base: Vec2::new(500.0, 0.0),
            carries_enemy_flag: false,
        };
        let close_home_guard = CtfTargetCandidate {
            entity: Entity::from_raw(2),
            team: AiTeam::Red,
            position: Vec2::new(450.0, 0.0),
            target: DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)),
            home_base: Vec2::new(500.0, 0.0),
            carries_enemy_flag: false,
        };
        let midfield_guard = CtfTargetCandidate {
            entity: Entity::from_raw(3),
            team: AiTeam::Red,
            position: Vec2::ZERO,
            target: DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)),
            home_base: Vec2::new(500.0, 0.0),
            carries_enemy_flag: false,
        };
        let flanker = CtfTargetCandidate {
            entity: Entity::from_raw(4),
            team: AiTeam::Red,
            position: Vec2::new(-400.0, -160.0),
            target: DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)),
            home_base: Vec2::new(500.0, 0.0),
            carries_enemy_flag: false,
        };
        let assignments = assign_ctf_targets(
            &[attacker, close_home_guard, midfield_guard, flanker],
            &[
                FlagTarget {
                    team: AiTeam::Red,
                    home: Vec2::new(500.0, 0.0),
                    position: Vec2::new(500.0, 0.0),
                    holder: None,
                },
                FlagTarget {
                    team: AiTeam::Blue,
                    home: Vec2::new(-500.0, 0.0),
                    position: Vec2::new(-400.0, 0.0),
                    holder: None,
                },
            ],
        );

        assert_eq!(
            assignments,
            vec![
                (
                    Entity::from_raw(1),
                    Some(DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)))
                ),
                (
                    Entity::from_raw(2),
                    Some(DrivingTarget::DefendHomeBase(Vec2::new(280.0, 0.0)))
                ),
                (
                    Entity::from_raw(3),
                    Some(DrivingTarget::MidfieldInterceptor(Vec2::ZERO))
                ),
                (
                    Entity::from_raw(4),
                    Some(DrivingTarget::EnemyFlagFlank(Vec2::new(-400.0, -220.0)))
                ),
            ]
        );
    }

    #[test]
    fn closest_virtual_player_claims_shared_enemy_flag() {
        let mut app = app_with_system();
        let far_ai = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(0.0, 0.0, 4.0),
        );
        let close_ai = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(-300.0, 0.0, 4.0),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-400.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );

        app.update();

        let far_transform = app.world.get::<Transform>(far_ai).unwrap();
        let close_transform = app.world.get::<Transform>(close_ai).unwrap();

        assert!(
            far_transform.translation.x > 0.0,
            "far opponent should defend the red base, x={}",
            far_transform.translation.x
        );
        assert!(
            close_transform.translation.x < -300.0,
            "closest opponent should claim the blue flag, x={}",
            close_transform.translation.x
        );
    }

    #[test]
    fn pickup_detour_still_reserves_enemy_flag_attack_role() {
        let mut app = app_with_system();
        let first_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let second_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-400.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(-100.0, 0.0, 2.0)),
        ));

        app.update();

        let first_transform = app.world.get::<Transform>(first_ai).unwrap();
        let second_transform = app.world.get::<Transform>(second_ai).unwrap();

        assert!(
            first_transform.translation.x < 0.0,
            "expected attacker to detour towards pickup on the flag lane, x={}",
            first_transform.translation.x
        );
        assert!(
            second_transform.translation.x > 0.0,
            "expected spare opponent to defend once attack lane is reserved, x={}",
            second_transform.translation.x
        );
    }

    #[test]
    fn spare_defender_detours_for_pickup_on_home_lane() {
        let mut app = app_with_system();
        let first_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let second_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-400.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
        ));

        app.update();

        let first_transform = app.world.get::<Transform>(first_ai).unwrap();
        let second_transform = app.world.get::<Transform>(second_ai).unwrap();

        assert!(
            first_transform.translation.x < 0.0,
            "expected attacker to keep the blue flag role, x={}",
            first_transform.translation.x
        );
        assert!(
            second_transform.translation.x > 0.0,
            "expected spare defender to detour through the home-lane pickup, x={}",
            second_transform.translation.x
        );
    }

    #[test]
    fn pricing_lifts_a_sabotage_for_the_team_whose_flag_is_stolen() {
        use crate::gameplay::virtual_player::ai::CTF_WIDE_DETOUR_MIN_PRIORITY;

        // Red's flag is being hauled off by an enemy. A flag is only ever held by
        // an enemy, so the same event makes Blue the carrier running it home: the
        // robbed team (Red) prices the sabotage to chase the thief, the carrier
        // team (Blue) to cover its own getaway, and the chase outranks the getaway.
        let flags = [
            FlagTarget {
                team: AiTeam::Red,
                home: Vec2::new(500.0, 0.0),
                position: Vec2::new(0.0, 0.0),
                holder: Some(Entity::from_raw(7)),
            },
            FlagTarget {
                team: AiTeam::Blue,
                home: Vec2::new(-500.0, 0.0),
                position: Vec2::new(-500.0, 0.0),
                holder: None,
            },
        ];
        let stolen = flag_stolen_state(&flags);
        assert!(stolen.for_team(AiTeam::Red), "an enemy holds Red's flag");
        assert!(
            !stolen.for_team(AiTeam::Blue),
            "Blue's own flag is safe at home"
        );

        let sabotage = ArenaPickup {
            position: Vec2::new(50.0, 0.0),
            kind: PickupKind::Sabotage,
        };
        let robbed = price_pickup_for_team(sabotage, None, AiTeam::Red, stolen).priority;
        let carrier = price_pickup_for_team(sabotage, None, AiTeam::Blue, stolen).priority;

        assert!(
            robbed >= CTF_WIDE_DETOUR_MIN_PRIORITY,
            "the robbed team must value the sabotage enough to chase the thief: {robbed}"
        );
        assert!(
            carrier >= CTF_WIDE_DETOUR_MIN_PRIORITY,
            "the carrier team must value the sabotage enough to cover its getaway: {carrier}"
        );
        assert!(
            robbed > carrier,
            "chasing the thief must still outrank covering our own run: robbed={robbed}, carrier={carrier}"
        );
    }

    #[test]
    fn pricing_lifts_a_sabotage_for_the_team_carrying_the_enemy_flag() {
        use crate::gameplay::pickup::collect::SABOTAGE_FLAG_GETAWAY_VIRTUAL_PLAYER_PRIORITY;
        use crate::gameplay::virtual_player::ai::CTF_WIDE_DETOUR_MIN_PRIORITY;

        // Red hauls Blue's flag home; Red's own flag sits safe at base. So Red is
        // the carrier-team that values the sabotage as getaway cover, while Blue is
        // the robbed team that values it to chase the thief.
        let flags = [
            FlagTarget {
                team: AiTeam::Blue,
                home: Vec2::new(-500.0, 0.0),
                position: Vec2::new(0.0, 0.0),
                holder: Some(Entity::from_raw(7)),
            },
            FlagTarget {
                team: AiTeam::Red,
                home: Vec2::new(500.0, 0.0),
                position: Vec2::new(500.0, 0.0),
                holder: None,
            },
        ];
        let stolen = flag_stolen_state(&flags);
        assert!(stolen.for_team(AiTeam::Blue), "an enemy holds Blue's flag");
        assert!(!stolen.for_team(AiTeam::Red), "Red's flag is safe at home");

        let sabotage = ArenaPickup {
            position: Vec2::new(50.0, 0.0),
            kind: PickupKind::Sabotage,
        };
        let carrier_team = price_pickup_for_team(sabotage, None, AiTeam::Red, stolen).priority;
        let robbed_team = price_pickup_for_team(sabotage, None, AiTeam::Blue, stolen).priority;

        assert_eq!(
            carrier_team, SABOTAGE_FLAG_GETAWAY_VIRTUAL_PLAYER_PRIORITY,
            "the team running the enemy flag home prices the sabotage as getaway cover: {carrier_team}"
        );
        assert!(
            carrier_team >= CTF_WIDE_DETOUR_MIN_PRIORITY,
            "getaway cover must justify pulling an escort off a committed run: {carrier_team}"
        );
        assert!(
            carrier_team > PickupKind::Sabotage.virtual_player_priority(),
            "covering our own carrier must beat the flat sabotage value: {carrier_team}"
        );
        assert!(
            robbed_team > carrier_team,
            "defending the robbed team's own steal still outranks getaway cover: \
             robbed={robbed_team}, carrier={carrier_team}"
        );
    }

    /// Drives one frame with the lone Red defender facing straight down its
    /// home-defence route (`-x`) while Red's flag is being carried off and the
    /// round is in closing-time discipline, then reports the defender's `y`
    /// drift. A pickup of `kind` sits off the route at `(50, 80)`: only a pickup
    /// worth the wide closing-time detour pulls the defender off the line, so a
    /// positive `y` means it broke off for the pickup.
    fn disciplined_defender_detour_y(kind: PickupKind) -> f32 {
        use crate::gameplay::ctf::{MatchPhase, CLOSING_TIME_FRAMES};
        use std::f32::consts::FRAC_PI_2;

        let mut app = app_with_system();
        app.insert_resource(MatchClock {
            frames_remaining: CLOSING_TIME_FRAMES,
            phase: MatchPhase::Regulation,
        });

        let defender = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(200.0, 0.0, 4.0),
        );
        // Face the defender west, straight along its intercept route, so any +y
        // motion is a genuine detour and not steering slack off the spawn facing.
        app.world.get_mut::<Transform>(defender).unwrap().rotation =
            Quat::from_rotation_z(FRAC_PI_2);

        let carrier = spawn_player(&mut app, Vec3::new(0.0, 0.0, 5.0));
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-500.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
            Some(carrier),
        );
        app.world.spawn((
            Pickup { kind },
            Transform::from_translation(Vec3::new(50.0, 80.0, 2.0)),
        ));

        app.update();

        app.world.get::<Transform>(defender).unwrap().translation.y
    }

    #[test]
    fn stolen_flag_pulls_a_disciplined_defender_onto_a_sabotage() {
        let sabotage_y = disciplined_defender_detour_y(PickupKind::Sabotage);
        let cash_y = disciplined_defender_detour_y(PickupKind::Cash);

        assert!(
            sabotage_y > 0.0,
            "a defender must break off onto the sabotage to slow the thief carrying its flag, y={sabotage_y}"
        );
        assert!(
            cash_y.abs() < 1e-3,
            "a disciplined defender must leave a cash bag and hold its intercept route, y={cash_y}"
        );
    }

    /// Drives one frame with a Red escort facing east along its escort route while
    /// a Red carrier hauls the Blue flag home (Red's own flag safe) in closing-time
    /// discipline, then reports the escort's `y` drift. A pickup of `kind` sits off
    /// the route at `(300, 80)`: only a pickup worth the wide closing-time detour
    /// pulls the escort off the line, so a positive `y` means it broke off for it.
    /// A flat-priced sabotage stays on its narrow lane and is dropped in closing
    /// time, so only the getaway-priced sabotage (our carrier is running) detours.
    fn disciplined_escort_detour_y(kind: PickupKind) -> f32 {
        disciplined_escort_detour_y_with_integrity(kind, None)
    }

    fn disciplined_escort_detour_y_with_integrity(
        kind: PickupKind,
        integrity: Option<VehicleIntegrity>,
    ) -> f32 {
        use crate::gameplay::ctf::{MatchPhase, CLOSING_TIME_FRAMES};
        use std::f32::consts::FRAC_PI_2;

        let mut app = app_with_system();
        app.insert_resource(MatchClock {
            frames_remaining: CLOSING_TIME_FRAMES,
            phase: MatchPhase::Regulation,
        });
        if let Some(integrity) = integrity {
            app.insert_resource(integrity);
        }

        let escort = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(200.0, 0.0, 4.0),
        );
        // Face the escort east, straight along its escort route toward home, so any
        // +y motion is a genuine detour and not steering slack off the spawn facing.
        app.world.get_mut::<Transform>(escort).unwrap().rotation =
            Quat::from_rotation_z(-FRAC_PI_2);

        let carrier = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(400.0, 0.0, 4.0),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(400.0, 0.0, 2.0),
            Some(carrier),
        );
        app.world.spawn((
            Pickup { kind },
            Transform::from_translation(Vec3::new(300.0, 80.0, 2.0)),
        ));

        app.update();

        app.world.get::<Transform>(escort).unwrap().translation.y
    }

    #[test]
    fn carried_flag_pulls_a_disciplined_escort_onto_a_sabotage() {
        let sabotage_y = disciplined_escort_detour_y(PickupKind::Sabotage);
        let cash_y = disciplined_escort_detour_y(PickupKind::Cash);

        assert!(
            sabotage_y > 0.0,
            "an escort must break off onto the sabotage to cover its carrier's run home, y={sabotage_y}"
        );
        assert!(
            cash_y.abs() < 1e-3,
            "a disciplined escort must leave a cash bag and hold its escort route, y={cash_y}"
        );
    }

    #[test]
    fn carried_flag_pulls_a_disciplined_escort_onto_a_shield() {
        // The defensive mirror of the getaway sabotage: while a teammate runs the
        // enemy flag home (fragile, double ram bleed) a healthy escort-team would
        // normally leave a flat-priced shield, but the getaway lift makes it break
        // off to armour the run even under closing-time discipline.
        let shield_y = disciplined_escort_detour_y(PickupKind::Shield);
        let cash_y = disciplined_escort_detour_y(PickupKind::Cash);

        assert!(
            shield_y > 0.0,
            "an escort must break off onto the shield to armour its carrier's run home, y={shield_y}"
        );
        assert!(
            cash_y.abs() < 1e-3,
            "a disciplined escort must leave a cash bag and hold its escort route, y={cash_y}"
        );
    }

    #[test]
    fn carried_flag_pulls_a_worn_disciplined_escort_onto_a_repair() {
        // The third leg of the getaway tripod: while a teammate runs the enemy flag
        // home, a worn escort-team tops up the integrity buffer the gauntlet will
        // burn. Unlike the shield/sabotage getaway lifts, a repair heals nothing on
        // a full team, so the team is held to half durability (0.5, above the
        // pit-retreat band) where a bare repair is worth only 110, below the
        // closing-time wide-detour bar. The getaway top-up lifts it over that bar
        // while a cash bag stays left.
        let worn = || {
            Some(VehicleIntegrity {
                player: 100.0,
                opponent: 50.0,
            })
        };
        let repair_y = disciplined_escort_detour_y_with_integrity(PickupKind::Repair, worn());
        let cash_y = disciplined_escort_detour_y_with_integrity(PickupKind::Cash, worn());

        assert!(
            repair_y > 0.0,
            "a worn escort must break off onto the repair to top up its carrier's run home, y={repair_y}"
        );
        assert!(
            cash_y.abs() < 1e-3,
            "a disciplined escort must leave a cash bag and hold its escort route, y={cash_y}"
        );
    }

    #[test]
    fn nitro_boost_increases_virtual_player_distance() {
        let normal_y = one_frame_ai_y(AiTeam::Red, None);
        let boosted_y = one_frame_ai_y(AiTeam::Red, Some(NitroBoosts::trigger_opponent));

        assert!(
            boosted_y > normal_y,
            "normal={normal_y}, boosted={boosted_y}"
        );
    }

    #[test]
    fn player_team_nitro_boosts_blue_virtual_players() {
        let normal_y = one_frame_ai_y(AiTeam::Blue, None);
        let boosted_y = one_frame_ai_y(AiTeam::Blue, Some(NitroBoosts::trigger_player));

        assert!(
            boosted_y > normal_y,
            "normal={normal_y}, boosted={boosted_y}"
        );
    }

    #[test]
    fn opponent_nitro_does_not_boost_blue_virtual_players() {
        let normal_y = one_frame_ai_y(AiTeam::Blue, None);
        let opponent_boosted_y = one_frame_ai_y(AiTeam::Blue, Some(NitroBoosts::trigger_opponent));

        assert!(
            (opponent_boosted_y - normal_y).abs() < 1e-4,
            "normal={normal_y}, opponent_boosted={opponent_boosted_y}"
        );
    }

    fn one_frame_ai_y_with_integrity(team: AiTeam, integrity: VehicleIntegrity) -> f32 {
        let mut app = app_with_system();
        app.insert_resource(integrity);
        let ai = spawn_ai_on_team(&mut app, team, vec![Vec2::new(0.0, 1000.0)]);

        app.update();

        app.world.get::<Transform>(ai).unwrap().translation.y
    }

    #[test]
    fn battered_integrity_reduces_opponent_distance() {
        let healthy_y = one_frame_ai_y(AiTeam::Red, None);
        let wrecked_y = one_frame_ai_y_with_integrity(
            AiTeam::Red,
            VehicleIntegrity {
                player: 100.0,
                opponent: 0.0,
            },
        );

        assert!(
            wrecked_y > 0.0 && wrecked_y < healthy_y,
            "healthy={healthy_y}, wrecked={wrecked_y}"
        );
    }

    fn one_frame_ai_y_with_stun(team: AiTeam, stuns: WreckStuns) -> f32 {
        let mut app = app_with_system();
        app.insert_resource(stuns);
        let ai = spawn_ai_on_team(&mut app, team, vec![Vec2::new(0.0, 1000.0)]);

        app.update();

        app.world.get::<Transform>(ai).unwrap().translation.y
    }

    #[test]
    fn a_wreck_spin_out_reduces_opponent_distance() {
        let healthy_y = one_frame_ai_y(AiTeam::Red, None);
        let mut stuns = WreckStuns::default();
        stuns.trigger_opponent();
        let stunned_y = one_frame_ai_y_with_stun(AiTeam::Red, stuns);

        assert!(
            stunned_y > 0.0 && stunned_y < healthy_y,
            "a spun-out opponent should crawl forward: healthy={healthy_y}, stunned={stunned_y}"
        );
    }

    #[test]
    fn an_opponent_spin_out_does_not_slow_blue_virtual_players() {
        let healthy_y = one_frame_ai_y(AiTeam::Blue, None);
        let mut stuns = WreckStuns::default();
        stuns.trigger_opponent();
        let blue_y = one_frame_ai_y_with_stun(AiTeam::Blue, stuns);

        assert!(
            (blue_y - healthy_y).abs() < 1e-4,
            "the opponents' spin-out must not slow blue cars: healthy={healthy_y}, blue={blue_y}"
        );
    }

    fn one_frame_ai_y_with_surge(team: AiTeam, surges: WreckSurges) -> f32 {
        let mut app = app_with_system();
        app.insert_resource(surges);
        let ai = spawn_ai_on_team(&mut app, team, vec![Vec2::new(0.0, 1000.0)]);

        app.update();

        app.world.get::<Transform>(ai).unwrap().translation.y
    }

    #[test]
    fn a_fresh_kill_surge_increases_opponent_distance() {
        let healthy_y = one_frame_ai_y(AiTeam::Red, None);
        let mut surges = WreckSurges::default();
        surges.trigger_opponent();
        let surging_y = one_frame_ai_y_with_surge(AiTeam::Red, surges);

        assert!(
            surging_y > healthy_y,
            "a fresh-kill surge should speed an opponent up: healthy={healthy_y}, surging={surging_y}"
        );
    }

    #[test]
    fn a_player_team_surge_does_not_speed_red_virtual_players() {
        let healthy_y = one_frame_ai_y(AiTeam::Red, None);
        let mut surges = WreckSurges::default();
        surges.trigger_player();
        let red_y = one_frame_ai_y_with_surge(AiTeam::Red, surges);

        assert!(
            (red_y - healthy_y).abs() < 1e-4,
            "the player team's surge must not speed red cars: healthy={healthy_y}, red={red_y}"
        );
    }

    #[test]
    fn opponent_wear_does_not_slow_blue_virtual_players() {
        let healthy_y = one_frame_ai_y(AiTeam::Blue, None);
        let opponent_wrecked_y = one_frame_ai_y_with_integrity(
            AiTeam::Blue,
            VehicleIntegrity {
                player: 100.0,
                opponent: 0.0,
            },
        );

        assert!(
            (opponent_wrecked_y - healthy_y).abs() < 1e-4,
            "healthy={healthy_y}, opponent_wrecked={opponent_wrecked_y}"
        );
    }

    fn one_frame_ai_y_with_sabotage(team: AiTeam, effects: SabotageEffects) -> f32 {
        let mut app = app_with_system();
        app.insert_resource(effects);
        let ai = spawn_ai_on_team(&mut app, team, vec![Vec2::new(0.0, 1000.0)]);

        app.update();

        app.world.get::<Transform>(ai).unwrap().translation.y
    }

    #[test]
    fn sabotaging_the_opponent_reduces_its_distance() {
        let healthy_y = one_frame_ai_y(AiTeam::Red, None);
        let mut effects = SabotageEffects::default();
        effects.sabotage_opponent();
        let sabotaged_y = one_frame_ai_y_with_sabotage(AiTeam::Red, effects);

        assert!(
            sabotaged_y > 0.0 && sabotaged_y < healthy_y,
            "a sabotaged opponent should crawl forward: healthy={healthy_y}, sabotaged={sabotaged_y}"
        );
    }

    #[test]
    fn sabotaging_the_opponent_does_not_slow_blue_virtual_players() {
        let healthy_y = one_frame_ai_y(AiTeam::Blue, None);
        let mut effects = SabotageEffects::default();
        effects.sabotage_opponent();
        let blue_y = one_frame_ai_y_with_sabotage(AiTeam::Blue, effects);

        assert!(
            (blue_y - healthy_y).abs() < 1e-4,
            "sabotaging red must not slow blue cars: healthy={healthy_y}, blue={blue_y}"
        );
    }

    fn attacker_x_after_frames(integrity: Option<VehicleIntegrity>, frames: u32) -> f32 {
        let mut app = app_with_system();
        if let Some(integrity) = integrity {
            app.insert_resource(integrity);
        }
        // Red attacker facing +Y, enemy (blue) flag up the lane, own flag behind.
        let ai = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(0.0, -600.0),
            Vec3::new(0.0, -600.0, 1.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(0.0, 600.0),
            Vec3::new(0.0, 600.0, 1.0),
            None,
        );
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Repair,
            },
            Transform::from_translation(Vec3::new(80.0, 150.0, 2.0)),
        ));

        for _ in 0..frames {
            app.update();
        }

        app.world.get::<Transform>(ai).unwrap().translation.x
    }

    #[test]
    fn battered_attacker_peels_off_for_a_repair_on_the_flag_lane() {
        let healthy_x = attacker_x_after_frames(None, 15);
        let wrecked_x = attacker_x_after_frames(
            Some(VehicleIntegrity {
                player: 100.0,
                opponent: 0.0,
            }),
            15,
        );

        assert!(
            healthy_x.abs() < 1.0,
            "a pristine attacker should hold the flag lane, x={healthy_x}"
        );
        assert!(
            wrecked_x > 5.0,
            "a wrecked attacker should peel off toward the repair, healthy={healthy_x}, wrecked={wrecked_x}"
        );
    }

    /// Mirror of [`attacker_x_after_frames`] for a Blue (player-team) attacker, so
    /// repair pursuit can be checked against the attacker's *own* team wear.
    fn blue_attacker_x_after_frames(integrity: Option<VehicleIntegrity>, frames: u32) -> f32 {
        let mut app = app_with_system();
        if let Some(integrity) = integrity {
            app.insert_resource(integrity);
        }
        // Blue attacker facing +Y, enemy (red) flag up the lane, own flag behind.
        let ai = spawn_ai_on_team(&mut app, AiTeam::Blue, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(0.0, -600.0),
            Vec3::new(0.0, -600.0, 1.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(0.0, 600.0),
            Vec3::new(0.0, 600.0, 1.0),
            None,
        );
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Repair,
            },
            Transform::from_translation(Vec3::new(80.0, 150.0, 2.0)),
        ));

        for _ in 0..frames {
            app.update();
        }

        app.world.get::<Transform>(ai).unwrap().translation.x
    }

    #[test]
    fn healthy_attacker_holds_lane_when_only_the_enemy_is_wrecked() {
        // Blue is pristine, Red is wrecked. A repair is worthless to a full team
        // (durability is capped), so a healthy attacker must keep pushing the flag
        // rather than detour for a patch-up it cannot use: repairs are valued by
        // your OWN wear, never the enemy's.
        let healthy_x = blue_attacker_x_after_frames(
            Some(VehicleIntegrity {
                player: 100.0,
                opponent: 0.0,
            }),
            15,
        );

        assert!(
            healthy_x.abs() < 1.0,
            "a pristine attacker should hold the flag lane even when the enemy is wrecked, x={healthy_x}"
        );
    }

    #[test]
    fn battered_blue_attacker_still_peels_off_for_a_repair() {
        // The mirror case: when the Blue attacker's own team is wrecked it must
        // chase the repair, proving per-team pricing scales repair pursuit for
        // either side.
        let wrecked_x = blue_attacker_x_after_frames(
            Some(VehicleIntegrity {
                player: 0.0,
                opponent: 100.0,
            }),
            15,
        );

        assert!(
            wrecked_x > 5.0,
            "a wrecked blue attacker should peel off toward the repair, x={wrecked_x}"
        );
    }
}
