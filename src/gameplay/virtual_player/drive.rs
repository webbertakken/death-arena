use crate::gameplay::ctf::{CtfFlag, CtfMatchResult, FlagTeam};
use crate::gameplay::main::{BOUNDS, TIME_STEP};
use crate::gameplay::pickup::{NitroBoosts, Pickup};
use crate::gameplay::player::Player;
use crate::gameplay::virtual_player::ai::{
    choose_capture_the_flag_target, choose_driving_target, compute_steering, next_waypoint, AiTeam,
    DrivingChoices, DrivingTarget, FlagTarget, PickupTarget, ThreatTarget,
};
use crate::gameplay::virtual_player::VirtualPlayer;
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;

/// Distance (world units) at which a virtual player considers a waypoint
/// reached and advances to the next one.
const WAYPOINT_ARRIVE_RADIUS: f32 = 80.0;
const PICKUP_PURSUIT_RADIUS: f32 = 450.0;
const PLAYER_PURSUIT_RADIUS: f32 = 500.0;
const HOME_LANE_GUARD_DISTANCE: f32 = 220.0;
const MIDFIELD_LANE_GUARD_FACTOR: f32 = 0.5;
const TEAMMATE_SPACING_RADIUS: f32 = 90.0;

type HumanPlayerTransform = (With<Player>, Without<VirtualPlayer>);

/// Drives every [`VirtualPlayer`] towards its current patrol waypoint, applying
/// the same movement/rotation model the human player uses.
pub fn virtual_player_drive_system(
    mut query: Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
    player_query: Query<(Entity, &Transform), HumanPlayerTransform>,
    pickup_query: Query<(&Transform, &Pickup), Without<VirtualPlayer>>,
    flag_query: Query<(&Transform, &CtfFlag), Without<VirtualPlayer>>,
    nitro_boosts: Option<Res<NitroBoosts>>,
    match_result: Option<Res<CtfMatchResult>>,
) {
    if match_result
        .as_ref()
        .is_some_and(|result| result.winner.is_some())
    {
        return;
    }

    let player = player_query
        .get_single()
        .ok()
        .map(|(entity, transform)| (entity, transform.translation.xy()));
    let player_position = player.map(|(_, position)| position);
    let threats = threat_targets(&query, player_position);
    let available_pickups = pickup_targets(&pickup_query);
    let holder_positions = holder_positions(&query, player);
    let flags = flag_targets(&flag_query, &holder_positions);
    let assigned_ctf_targets = assigned_ctf_targets(&query, &flags, &threats);
    let teammate_positions = virtual_player_positions(&query);
    let claimed_pickups = claimed_pickups_for_virtual_players(
        &query,
        &assigned_ctf_targets,
        &available_pickups,
        player_position,
    );

    for (entity, mut ai, mut transform) in &mut query {
        let position = transform.translation.xy();
        let forward = (transform.rotation * Vec3::Y).xy();
        let ctf_target = assigned_ctf_targets
            .iter()
            .find(|(assigned_entity, _)| *assigned_entity == entity)
            .and_then(|(_, target)| *target);
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
                pickup_pursuit_radius: PICKUP_PURSUIT_RADIUS,
                player_position: player_position_for_team(ai.team, player_position),
                player_pursuit_radius: PLAYER_PURSUIT_RADIUS,
            },
        ) else {
            continue;
        };

        let spacing_target = matches!(target, DrivingTarget::PatrolWaypoint(_))
            .then(|| teammate_spacing_target(entity, ai.team, position, &teammate_positions))
            .flatten();
        let target_position = spacing_target.unwrap_or_else(|| target.position());
        let intent = compute_steering(position, forward, target_position, WAYPOINT_ARRIVE_RADIUS);

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
        let nitro_multiplier = nitro_boosts
            .as_ref()
            .map_or(1.0, |boosts| nitro_multiplier_for_team(boosts, ai.team));
        let movement_distance = intent.throttle * ai.movement_speed * nitro_multiplier * TIME_STEP;
        transform.translation += movement_direction * movement_distance;

        // Keep opponents inside the arena, just like the player.
        let extents = Vec3::new(BOUNDS.x / 2.0, BOUNDS.y / 2.0, 0.0);
        transform.translation.x = transform.translation.x.clamp(-extents.x, extents.x);
        transform.translation.y = transform.translation.y.clamp(-extents.y, extents.y);
        transform.translation.z = 4.0;
    }
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

fn pickup_targets(
    pickup_query: &Query<(&Transform, &Pickup), Without<VirtualPlayer>>,
) -> Vec<PickupTarget> {
    pickup_query
        .iter()
        .map(|(transform, pickup)| PickupTarget {
            position: transform.translation.xy(),
            priority: pickup.kind.virtual_player_priority(),
        })
        .collect()
}

fn claimed_pickups_for_virtual_players(
    query: &Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
    assigned_ctf_targets: &[(Entity, Option<DrivingTarget>)],
    pickups: &[PickupTarget],
    player_position: Option<Vec2>,
) -> Vec<(Entity, PickupTarget)> {
    let mut ordered_pickups = pickups.to_vec();
    ordered_pickups.sort_by(compare_pickup_claim_priority);

    let mut claimed_entities = Vec::new();
    ordered_pickups
        .iter()
        .copied()
        .filter_map(|pickup| {
            let entity = closest_eligible_pickup_claimant(
                query,
                assigned_ctf_targets,
                pickup,
                player_position,
                &claimed_entities,
            )?;
            claimed_entities.push(entity);
            Some((entity, pickup))
        })
        .collect()
}

fn closest_eligible_pickup_claimant(
    query: &Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
    assigned_ctf_targets: &[(Entity, Option<DrivingTarget>)],
    pickup: PickupTarget,
    player_position: Option<Vec2>,
    claimed_entities: &[Entity],
) -> Option<Entity> {
    let pickup_candidates = [pickup];
    let (entity, _, position) = query
        .iter()
        .filter(|(entity, _, _)| !claimed_entities.contains(entity))
        .filter_map(|(entity, ai, transform)| {
            let ctf_target = assigned_ctf_targets
                .iter()
                .find(|(assigned_entity, _)| *assigned_entity == entity)
                .and_then(|(_, target)| *target);
            let position = transform.translation.xy();
            let target = choose_driving_target(
                position,
                DrivingChoices {
                    waypoints: &ai.waypoints,
                    current_waypoint: ai.current_waypoint,
                    ctf_target,
                    pickups: &pickup_candidates,
                    pickup_pursuit_radius: PICKUP_PURSUIT_RADIUS,
                    player_position: player_position_for_team(ai.team, player_position),
                    player_pursuit_radius: PLAYER_PURSUIT_RADIUS,
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
                .then_with(|| {
                    a_position
                        .x
                        .partial_cmp(&b_position.x)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| {
                    a_position
                        .y
                        .partial_cmp(&b_position.y)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        })
        .filter(|(_, team, position)| {
            !virtual_player_yields_player_pickup_claim(*team, player_position, pickup, *position)
        })?;

    Some(entity)
}

fn compare_pickup_claim_priority(a: &PickupTarget, b: &PickupTarget) -> std::cmp::Ordering {
    b.priority
        .cmp(&a.priority)
        .then_with(|| {
            a.position
                .x
                .partial_cmp(&b.position.x)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| {
            a.position
                .y
                .partial_cmp(&b.position.y)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
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
    if player_distance_sq > PICKUP_PURSUIT_RADIUS * PICKUP_PURSUIT_RADIUS {
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

    if is_best_fallback_home_defender(candidate, candidates) {
        return defend_home_target(candidate.team, flags);
    }

    if is_best_fallback_midfield_interceptor(candidate, candidates, flags) {
        return midfield_interceptor_target(candidate.team, flags);
    }

    None
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
        .then_with(|| {
            a.position
                .x
                .partial_cmp(&b.position.x)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| {
            a.position
                .y
                .partial_cmp(&b.position.y)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
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
        .min_by(|a, b| compare_fallback_midfield_interceptors(a, b, target))
        .is_some_and(|best| best.entity == candidate.entity)
}

fn compare_fallback_midfield_interceptors(
    a: &CtfTargetCandidate,
    b: &CtfTargetCandidate,
    target: Vec2,
) -> std::cmp::Ordering {
    a.position
        .distance_squared(target)
        .partial_cmp(&b.position.distance_squared(target))
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| {
            a.position
                .x
                .partial_cmp(&b.position.x)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| {
            a.position
                .y
                .partial_cmp(&b.position.y)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
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
        .then_with(|| {
            a.position
                .x
                .partial_cmp(&b.position.x)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| {
            a.position
                .y
                .partial_cmp(&b.position.y)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
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

const fn should_coordinate_ctf_target(target: DrivingTarget) -> bool {
    matches!(
        target,
        DrivingTarget::DefendHomeBase(_)
            | DrivingTarget::EnemyFlag(_)
            | DrivingTarget::EscortFlagCarrier(_)
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

    fn spawn_ai_on_team(app: &mut App, team: AiTeam, waypoints: Vec<Vec2>) -> Entity {
        app.world
            .spawn((
                VirtualPlayer {
                    team,
                    movement_speed: 500.0,
                    rotation_speed: f32::to_radians(360.0),
                    waypoints,
                    current_waypoint: 0,
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
    fn flag_carrier_hunts_stolen_home_flag_before_red_base() {
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
            "expected flag carrier to defend stolen home flag, x={}",
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
            second_transform.translation.x > 0.0,
            "second opponent should defend the red base instead, x={}",
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
}
