use bevy::prelude::*;

/// Minimum forward throttle so a virtual player keeps moving (and can therefore
/// keep turning) even when its target is to the side.
pub const MIN_THROTTLE: f32 = 0.3;

/// Heading dot-product below which a virtual player backs up instead of doing a
/// wide forward loop.
pub const REVERSE_DOT_THRESHOLD: f32 = -0.35;

/// Angular error (radians) at which the steering output saturates to full lock.
/// Within this range steering is proportional to the heading error.
pub const STEER_RANGE: f32 = std::f32::consts::FRAC_PI_4;

/// Distance ahead of a friendly flag carrier that an escort tries to occupy.
pub const ESCORT_LEAD_DISTANCE: f32 = 120.0;

/// Distance at which an enemy near a home flag becomes a defensive emergency.
pub const HOME_FLAG_THREAT_RADIUS: f32 = 500.0;

/// Distance from the home flag where defenders try to meet an incoming thief.
pub const HOME_FLAG_DEFENSE_DISTANCE: f32 = 140.0;

/// Maximum sideways distance from a CTF push where a pickup still counts as
/// being on the flag lane.
pub const CTF_PICKUP_LANE_WIDTH: f32 = 60.0;

/// Distance from home at which a flag carrier stops gambling on pickup detours
/// and commits to finishing the capture.
pub const FLAG_CARRIER_CAPTURE_COMMIT_DISTANCE: f32 = 180.0;

/// Minimum pickup priority that justifies interrupting a CTF objective.
pub const CTF_PICKUP_DETOUR_MIN_PRIORITY: u32 = 50;

/// Normalised driving intent produced by the virtual player brain.
///
/// Both fields are in the range `-1.0..=1.0` and are engine-agnostic: the
/// driving system multiplies them by the car's tuning constants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteeringIntent {
    /// Forward/backward intent. Positive drives forward.
    pub throttle: f32,
    /// Turning intent. Positive turns left (counter-clockwise), matching
    /// `Transform::rotate_z` with a positive angle.
    pub steer: f32,
}

impl SteeringIntent {
    pub const IDLE: Self = Self {
        throttle: 0.0,
        steer: 0.0,
    };
}

/// The kind of world target a virtual player is currently chasing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrivingTarget {
    DefendHomeBase(Vec2),
    HomeBase(Vec2),
    EnemyFlag(Vec2),
    EscortFlagCarrier(Vec2),
    MidfieldInterceptor(Vec2),
    PatrolWaypoint(Vec2),
    Pickup(Vec2),
    Player(Vec2),
    StolenHomeFlag(Vec2),
}

impl DrivingTarget {
    #[must_use]
    pub const fn position(self) -> Vec2 {
        match self {
            Self::DefendHomeBase(position)
            | Self::HomeBase(position)
            | Self::EnemyFlag(position)
            | Self::EscortFlagCarrier(position)
            | Self::MidfieldInterceptor(position)
            | Self::PatrolWaypoint(position)
            | Self::Pickup(position)
            | Self::Player(position)
            | Self::StolenHomeFlag(position) => position,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiTeam {
    Blue,
    Red,
}

impl AiTeam {
    pub const fn enemy(self) -> Self {
        match self {
            Self::Blue => Self::Red,
            Self::Red => Self::Blue,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlagTarget {
    pub team: AiTeam,
    pub home: Vec2,
    pub position: Vec2,
    pub holder: Option<Entity>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThreatTarget {
    pub team: AiTeam,
    pub position: Vec2,
}

/// A collectible target visible to virtual players.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickupTarget {
    pub position: Vec2,
    pub priority: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct DrivingChoices<'a> {
    pub waypoints: &'a [Vec2],
    pub current_waypoint: usize,
    pub ctf_target: Option<DrivingTarget>,
    pub pickups: &'a [PickupTarget],
    pub pickup_pursuit_radius: f32,
    pub player_position: Option<Vec2>,
    pub player_pursuit_radius: f32,
}

#[must_use]
pub fn choose_capture_the_flag_target(
    ai_entity: Entity,
    team: AiTeam,
    flags: &[FlagTarget],
    threats: &[ThreatTarget],
) -> Option<DrivingTarget> {
    let own_flag = flags.iter().find(|flag| flag.team == team)?;
    let enemy_flag = flags.iter().find(|flag| flag.team == team.enemy())?;

    if own_flag.holder.is_some() && own_flag.holder != Some(ai_entity) {
        return Some(DrivingTarget::StolenHomeFlag(stolen_flag_intercept_point(
            own_flag.position,
            enemy_flag.home,
        )));
    }

    if own_flag.holder.is_none() && own_flag.position.distance_squared(own_flag.home) > f32::EPSILON
    {
        return Some(DrivingTarget::StolenHomeFlag(own_flag.position));
    }

    if enemy_flag.holder == Some(ai_entity) {
        return Some(DrivingTarget::HomeBase(own_flag.home));
    }

    if enemy_flag.holder.is_some() {
        return Some(DrivingTarget::EscortFlagCarrier(escort_lead_point(
            enemy_flag.position,
            own_flag.home,
        )));
    }

    if let Some(threat) = closest_home_flag_threat(team, own_flag, threats) {
        return Some(DrivingTarget::DefendHomeBase(defensive_intercept_point(
            own_flag.position,
            threat.position,
        )));
    }

    enemy_flag
        .holder
        .is_none()
        .then_some(DrivingTarget::EnemyFlag(enemy_flag.position))
}

fn closest_home_flag_threat(
    team: AiTeam,
    own_flag: &FlagTarget,
    threats: &[ThreatTarget],
) -> Option<ThreatTarget> {
    if own_flag.holder.is_some() {
        return None;
    }

    let radius_sq = HOME_FLAG_THREAT_RADIUS * HOME_FLAG_THREAT_RADIUS;
    threats
        .iter()
        .copied()
        .filter_map(|threat| {
            let distance_sq = threat.position.distance_squared(own_flag.position);
            (threat.team == team.enemy() && distance_sq <= radius_sq)
                .then_some((threat, distance_sq))
        })
        .min_by(|(a_threat, a_dist), (b_threat, b_dist)| {
            a_dist
                .partial_cmp(b_dist)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    a_threat
                        .position
                        .x
                        .partial_cmp(&b_threat.position.x)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| {
                    a_threat
                        .position
                        .y
                        .partial_cmp(&b_threat.position.y)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        })
        .map(|(threat, _)| threat)
}

fn defensive_intercept_point(flag_position: Vec2, threat_position: Vec2) -> Vec2 {
    let to_threat = threat_position - flag_position;
    let distance = to_threat.length();
    if distance <= HOME_FLAG_DEFENSE_DISTANCE {
        return threat_position;
    }

    let Some(direction) = to_threat.try_normalize() else {
        return flag_position;
    };
    flag_position + direction * HOME_FLAG_DEFENSE_DISTANCE
}

fn stolen_flag_intercept_point(flag_position: Vec2, enemy_home: Vec2) -> Vec2 {
    let to_enemy_home = enemy_home - flag_position;
    let distance = to_enemy_home.length();
    if distance <= HOME_FLAG_DEFENSE_DISTANCE {
        return flag_position;
    }

    let Some(direction) = to_enemy_home.try_normalize() else {
        return flag_position;
    };
    flag_position + direction * HOME_FLAG_DEFENSE_DISTANCE
}

fn escort_lead_point(carrier_position: Vec2, home: Vec2) -> Vec2 {
    let to_home = home - carrier_position;
    let distance = to_home.length();
    if distance <= ESCORT_LEAD_DISTANCE {
        return home;
    }

    let Some(direction) = to_home.try_normalize() else {
        return carrier_position;
    };
    carrier_position + direction * ESCORT_LEAD_DISTANCE
}

/// Pick the next driving target for a virtual player.
///
/// Valuable nearby pickups take priority over the patrol route and CTF lane
/// pushes so opponents can steal trackside rewards without abandoning the play.
/// When multiple pickups are in range, virtual players chase the highest priority
/// first and use distance as the tie-breaker.
#[must_use]
pub fn choose_driving_target(position: Vec2, choices: DrivingChoices<'_>) -> Option<DrivingTarget> {
    let ctf_target = choices.ctf_target;
    if let Some(target) = ctf_target {
        let pickup = pickup_detour(position, target, &choices);
        if pickup.is_some() {
            return pickup.map(|pickup| DrivingTarget::Pickup(pickup.position));
        }
        return Some(target);
    }

    let pickup = best_pickup(
        position,
        choices.pickups,
        choices.pickup_pursuit_radius,
        |_| true,
    );
    pickup
        .map(|pickup| DrivingTarget::Pickup(pickup.position))
        .or_else(|| {
            choices
                .player_position
                .filter(|player| {
                    position.distance_squared(*player) <= choices.player_pursuit_radius.powi(2)
                })
                .map(DrivingTarget::Player)
        })
        .or_else(|| {
            choices
                .waypoints
                .get(choices.current_waypoint)
                .copied()
                .map(DrivingTarget::PatrolWaypoint)
        })
}

fn pickup_detour(
    position: Vec2,
    target: DrivingTarget,
    choices: &DrivingChoices<'_>,
) -> Option<PickupTarget> {
    if matches!(target, DrivingTarget::HomeBase(_))
        && position.distance(target.position()) <= FLAG_CARRIER_CAPTURE_COMMIT_DISTANCE
    {
        return None;
    }

    if !matches!(
        target,
        DrivingTarget::DefendHomeBase(_)
            | DrivingTarget::EnemyFlag(_)
            | DrivingTarget::HomeBase(_)
            | DrivingTarget::StolenHomeFlag(_)
    ) {
        return None;
    }

    let target_distance_sq = position.distance_squared(target.position());
    best_pickup(
        position,
        choices.pickups,
        choices.pickup_pursuit_radius,
        |pickup| {
            pickup.priority >= CTF_PICKUP_DETOUR_MIN_PRIORITY
                && position.distance_squared(pickup.position) < target_distance_sq
                && is_ahead_of_target_push(position, pickup.position, target.position())
                && is_on_target_lane(position, pickup.position, target.position())
        },
    )
}

fn is_ahead_of_target_push(position: Vec2, pickup: Vec2, target: Vec2) -> bool {
    let to_pickup = pickup - position;
    let to_target = target - position;
    to_pickup.dot(to_target) > 0.0
}

fn is_on_target_lane(position: Vec2, pickup: Vec2, target: Vec2) -> bool {
    let to_target = target - position;
    let Some(direction) = to_target.try_normalize() else {
        return false;
    };

    let lateral_distance = (pickup - position).perp_dot(direction).abs();
    lateral_distance <= CTF_PICKUP_LANE_WIDTH
}

fn best_pickup(
    position: Vec2,
    pickups: &[PickupTarget],
    pursuit_radius: f32,
    is_eligible: impl Fn(PickupTarget) -> bool,
) -> Option<PickupTarget> {
    let radius_sq = pursuit_radius * pursuit_radius;
    pickups
        .iter()
        .copied()
        .filter(|pickup| is_eligible(*pickup))
        .filter_map(|pickup| {
            let distance_sq = position.distance_squared(pickup.position);
            (distance_sq <= radius_sq).then_some((pickup, distance_sq))
        })
        .min_by(|(a_pickup, a_dist), (b_pickup, b_dist)| {
            b_pickup.priority.cmp(&a_pickup.priority).then_with(|| {
                a_dist
                    .partial_cmp(b_dist)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        })
        .map(|(pickup, _)| pickup)
}

/// Decide how a virtual player should drive to reach `target`.
///
/// `forward` is the car's current facing direction (need not be normalised).
/// When the car is within `arrive_radius` of the target it idles so the caller
/// can advance to the next waypoint.
pub fn compute_steering(
    position: Vec2,
    forward: Vec2,
    target: Vec2,
    arrive_radius: f32,
) -> SteeringIntent {
    let to_target = target - position;
    let distance = to_target.length();
    if distance <= arrive_radius {
        return SteeringIntent::IDLE;
    }

    let dir = to_target / distance;
    let Some(heading) = forward.try_normalize() else {
        // Degenerate facing: crawl forward so the next frame has a direction.
        return SteeringIntent {
            throttle: MIN_THROTTLE,
            steer: 0.0,
        };
    };

    // Signed angle from the car's heading to the target direction.
    // Positive => target is to the left (counter-clockwise).
    let angle = heading.perp_dot(dir).atan2(heading.dot(dir));
    let alignment = heading.dot(dir);

    if alignment < REVERSE_DOT_THRESHOLD {
        return reverse_steering_intent(angle, alignment);
    }

    // Drive hardest when aligned, but never stall: a car cannot strafe, so it
    // must keep rolling to rotate towards a target that is to the side.
    let steer = (angle / STEER_RANGE).clamp(-1.0, 1.0);
    let throttle = alignment.clamp(MIN_THROTTLE, 1.0);

    SteeringIntent { throttle, steer }
}

fn reverse_steering_intent(angle: f32, alignment: f32) -> SteeringIntent {
    let reverse_angle = if angle >= 0.0 {
        angle - std::f32::consts::PI
    } else {
        angle + std::f32::consts::PI
    };

    SteeringIntent {
        throttle: alignment.clamp(-1.0, -MIN_THROTTLE),
        steer: (reverse_angle / STEER_RANGE).clamp(-1.0, 1.0),
    }
}

/// Index of the next waypoint in a cyclic patrol route.
///
/// Returns `0` for an empty or single-point route so callers never index out of
/// bounds.
pub const fn next_waypoint(current: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let next = current + 1;
    if next >= len {
        0
    } else {
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARRIVE: f32 = 10.0;
    const EPSILON: f32 = 1e-3;

    fn choices<'a>(
        waypoints: &'a [Vec2],
        current_waypoint: usize,
        ctf_target: Option<DrivingTarget>,
        pickups: &'a [PickupTarget],
        player_position: Option<Vec2>,
        player_pursuit_radius: f32,
    ) -> DrivingChoices<'a> {
        DrivingChoices {
            waypoints,
            current_waypoint,
            ctf_target,
            pickups,
            pickup_pursuit_radius: 100.0,
            player_position,
            player_pursuit_radius,
        }
    }

    fn assert_vec2_near(actual: Vec2, expected: Vec2) {
        assert!(
            actual.distance(expected) <= EPSILON,
            "actual={actual}, expected={expected}"
        );
    }

    #[test]
    fn idles_when_within_arrive_radius() {
        let intent = compute_steering(Vec2::ZERO, Vec2::Y, Vec2::new(0.0, 5.0), ARRIVE);
        assert_eq!(intent, SteeringIntent::IDLE);
    }

    #[test]
    fn drives_straight_forward_when_target_is_dead_ahead() {
        // Facing +Y, target far along +Y.
        let intent = compute_steering(Vec2::ZERO, Vec2::Y, Vec2::new(0.0, 500.0), ARRIVE);
        assert!(intent.steer.abs() < 1e-4, "steer was {}", intent.steer);
        assert!(
            (intent.throttle - 1.0).abs() < 1e-4,
            "throttle {}",
            intent.throttle
        );
    }

    #[test]
    fn steers_left_when_target_is_to_the_left() {
        // Facing +Y; target to the left is -X.
        let intent = compute_steering(Vec2::ZERO, Vec2::Y, Vec2::new(-500.0, 0.0), ARRIVE);
        assert!(
            intent.steer > 0.0,
            "expected positive steer, got {}",
            intent.steer
        );
    }

    #[test]
    fn steers_right_when_target_is_to_the_right() {
        // Facing +Y; target to the right is +X.
        let intent = compute_steering(Vec2::ZERO, Vec2::Y, Vec2::new(500.0, 0.0), ARRIVE);
        assert!(
            intent.steer < 0.0,
            "expected negative steer, got {}",
            intent.steer
        );
    }

    #[test]
    fn reverses_left_when_target_is_in_left_rear_quarter() {
        let intent = compute_steering(Vec2::ZERO, Vec2::Y, Vec2::new(-500.0, -500.0), ARRIVE);

        assert!((intent.steer + 1.0).abs() < 1e-4, "steer {}", intent.steer);
        assert!(
            (intent.throttle + std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-4,
            "throttle {}",
            intent.throttle
        );
    }

    #[test]
    fn reverses_when_target_is_directly_behind() {
        let intent = compute_steering(Vec2::ZERO, Vec2::Y, Vec2::new(0.0, -500.0), ARRIVE);

        assert!(
            intent.throttle < 0.0,
            "expected reverse throttle, got {}",
            intent.throttle
        );
        assert!(intent.steer.abs() < 1e-4, "steer {}", intent.steer);
    }

    #[test]
    fn never_stalls_when_target_is_perpendicular() {
        let intent = compute_steering(Vec2::ZERO, Vec2::Y, Vec2::new(500.0, 0.0), ARRIVE);
        assert!(intent.throttle >= MIN_THROTTLE);
    }

    #[test]
    fn degenerate_forward_vector_crawls_forward() {
        let intent = compute_steering(Vec2::ZERO, Vec2::ZERO, Vec2::new(0.0, 500.0), ARRIVE);
        assert!(intent.steer.abs() < 1e-4);
        assert!((intent.throttle - MIN_THROTTLE).abs() < 1e-4);
    }

    #[test]
    fn waypoint_cycles_back_to_start() {
        assert_eq!(next_waypoint(0, 3), 1);
        assert_eq!(next_waypoint(1, 3), 2);
        assert_eq!(next_waypoint(2, 3), 0);
    }

    #[test]
    fn waypoint_is_safe_for_degenerate_routes() {
        assert_eq!(next_waypoint(0, 0), 0);
        assert_eq!(next_waypoint(5, 0), 0);
        assert_eq!(next_waypoint(0, 1), 0);
    }

    #[test]
    fn targets_nearby_pickup_before_patrol_waypoint() {
        let waypoints = [Vec2::new(500.0, 0.0)];
        let pickups = [PickupTarget {
            position: Vec2::new(25.0, 0.0),
            priority: 100,
        }];
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(&waypoints, 0, None, &pickups, None, 0.0),
        );

        assert_eq!(target, Some(DrivingTarget::Pickup(Vec2::new(25.0, 0.0))));
    }

    #[test]
    fn targets_highest_value_pickup_in_pursuit_radius() {
        let waypoints = [Vec2::new(500.0, 0.0)];
        let pickups = [
            PickupTarget {
                position: Vec2::new(25.0, 0.0),
                priority: 25,
            },
            PickupTarget {
                position: Vec2::new(75.0, 0.0),
                priority: 100,
            },
        ];
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(&waypoints, 0, None, &pickups, None, 0.0),
        );

        assert_eq!(target, Some(DrivingTarget::Pickup(Vec2::new(75.0, 0.0))));
    }

    #[test]
    fn flag_carrier_prioritises_home_base_over_off_lane_pickup_detours() {
        let pickups = [PickupTarget {
            position: Vec2::new(25.0, 80.0),
            priority: 100,
        }];
        let home = Vec2::new(500.0, 0.0);
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(
                &[],
                0,
                Some(DrivingTarget::HomeBase(home)),
                &pickups,
                None,
                0.0,
            ),
        );

        assert_eq!(target, Some(DrivingTarget::HomeBase(home)));
    }

    #[test]
    fn ignores_pickups_outside_pursuit_radius() {
        let waypoint = Vec2::new(500.0, 0.0);
        let waypoints = [waypoint];
        let pickups = [PickupTarget {
            position: Vec2::new(250.0, 0.0),
            priority: 100,
        }];
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(&waypoints, 0, None, &pickups, None, 0.0),
        );

        assert_eq!(target, Some(DrivingTarget::PatrolWaypoint(waypoint)));
    }

    #[test]
    fn returns_no_target_without_waypoints_or_pickups() {
        assert_eq!(
            choose_driving_target(Vec2::ZERO, choices(&[], 0, None, &[], None, 0.0)),
            None
        );
    }

    #[test]
    fn targets_player_inside_pursuit_radius_before_patrol_waypoint() {
        let waypoints = [Vec2::new(0.0, 500.0)];
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(&waypoints, 0, None, &[], Some(Vec2::new(200.0, 0.0)), 250.0),
        );

        assert_eq!(target, Some(DrivingTarget::Player(Vec2::new(200.0, 0.0))));
    }

    #[test]
    fn pickup_stays_higher_priority_than_player_chase() {
        let waypoints = [Vec2::new(0.0, 500.0)];
        let pickups = [PickupTarget {
            position: Vec2::new(-50.0, 0.0),
            priority: 25,
        }];
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(
                &waypoints,
                0,
                None,
                &pickups,
                Some(Vec2::new(50.0, 0.0)),
                250.0,
            ),
        );

        assert_eq!(target, Some(DrivingTarget::Pickup(Vec2::new(-50.0, 0.0))));
    }

    #[test]
    fn ignores_player_outside_pursuit_radius() {
        let waypoint = Vec2::new(0.0, 500.0);
        let waypoints = [waypoint];
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(&waypoints, 0, None, &[], Some(Vec2::new(300.0, 0.0)), 250.0),
        );

        assert_eq!(target, Some(DrivingTarget::PatrolWaypoint(waypoint)));
    }

    #[test]
    fn targets_closer_pickup_before_distant_enemy_flag() {
        let waypoints = [Vec2::new(0.0, 500.0)];
        let pickups = [PickupTarget {
            position: Vec2::new(-25.0, 0.0),
            priority: 100,
        }];
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(
                &waypoints,
                0,
                Some(DrivingTarget::EnemyFlag(Vec2::new(-300.0, 0.0))),
                &pickups,
                None,
                0.0,
            ),
        );

        assert_eq!(target, Some(DrivingTarget::Pickup(Vec2::new(-25.0, 0.0))));
    }

    #[test]
    fn attacker_chooses_affordable_detour_when_richest_pickup_is_past_flag() {
        let waypoints = [Vec2::new(0.0, 500.0)];
        let pickups = [
            PickupTarget {
                position: Vec2::new(80.0, 0.0),
                priority: 50,
            },
            PickupTarget {
                position: Vec2::new(420.0, 0.0),
                priority: 100,
            },
        ];
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(
                &waypoints,
                0,
                Some(DrivingTarget::EnemyFlag(Vec2::new(300.0, 0.0))),
                &pickups,
                None,
                0.0,
            ),
        );

        assert_eq!(target, Some(DrivingTarget::Pickup(Vec2::new(80.0, 0.0))));
    }

    #[test]
    fn attacker_ignores_pickup_far_off_the_flag_lane() {
        let waypoints = [Vec2::new(0.0, 500.0)];
        let pickups = [
            PickupTarget {
                position: Vec2::new(80.0, 0.0),
                priority: 50,
            },
            PickupTarget {
                position: Vec2::new(60.0, 70.0),
                priority: 100,
            },
        ];
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(
                &waypoints,
                0,
                Some(DrivingTarget::EnemyFlag(Vec2::new(300.0, 0.0))),
                &pickups,
                None,
                0.0,
            ),
        );

        assert_eq!(target, Some(DrivingTarget::Pickup(Vec2::new(80.0, 0.0))));
    }

    #[test]
    fn attacker_ignores_low_value_pickup_on_flag_lane() {
        let waypoints = [Vec2::new(0.0, 500.0)];
        let pickups = [PickupTarget {
            position: Vec2::new(80.0, 0.0),
            priority: 25,
        }];
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(
                &waypoints,
                0,
                Some(DrivingTarget::EnemyFlag(Vec2::new(300.0, 0.0))),
                &pickups,
                None,
                0.0,
            ),
        );

        assert_eq!(
            target,
            Some(DrivingTarget::EnemyFlag(Vec2::new(300.0, 0.0)))
        );
    }

    #[test]
    fn attacker_ignores_pickup_behind_enemy_flag_push() {
        let waypoints = [Vec2::new(0.0, 500.0)];
        let pickups = [PickupTarget {
            position: Vec2::new(100.0, 0.0),
            priority: 100,
        }];
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(
                &waypoints,
                0,
                Some(DrivingTarget::EnemyFlag(Vec2::new(-300.0, 0.0))),
                &pickups,
                None,
                0.0,
            ),
        );

        assert_eq!(
            target,
            Some(DrivingTarget::EnemyFlag(Vec2::new(-300.0, 0.0)))
        );
    }

    #[test]
    fn flag_carrier_ignores_pickup_behind_route_home() {
        let waypoints = [Vec2::new(0.0, 500.0)];
        let pickups = [PickupTarget {
            position: Vec2::new(25.0, 0.0),
            priority: 100,
        }];
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(
                &waypoints,
                0,
                Some(DrivingTarget::HomeBase(Vec2::new(-300.0, 0.0))),
                &pickups,
                None,
                0.0,
            ),
        );

        assert_eq!(
            target,
            Some(DrivingTarget::HomeBase(Vec2::new(-300.0, 0.0)))
        );
    }

    #[test]
    fn flag_carrier_detours_for_pickup_on_route_home() {
        let waypoints = [Vec2::new(0.0, 500.0)];
        let pickups = [PickupTarget {
            position: Vec2::new(-80.0, 0.0),
            priority: 100,
        }];
        let home = Vec2::new(-300.0, 0.0);
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(
                &waypoints,
                0,
                Some(DrivingTarget::HomeBase(home)),
                &pickups,
                None,
                0.0,
            ),
        );

        assert_eq!(target, Some(DrivingTarget::Pickup(Vec2::new(-80.0, 0.0))));
    }

    #[test]
    fn flag_carrier_commits_to_capture_near_home_base() {
        let waypoints = [Vec2::new(0.0, 500.0)];
        let pickups = [PickupTarget {
            position: Vec2::new(-40.0, 0.0),
            priority: 100,
        }];
        let home = Vec2::new(-120.0, 0.0);
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(
                &waypoints,
                0,
                Some(DrivingTarget::HomeBase(home)),
                &pickups,
                None,
                0.0,
            ),
        );

        assert_eq!(target, Some(DrivingTarget::HomeBase(home)));
    }

    #[test]
    fn defender_ignores_pickup_detours() {
        let waypoints = [Vec2::new(0.0, 500.0)];
        let pickups = [PickupTarget {
            position: Vec2::new(25.0, 80.0),
            priority: 100,
        }];
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(
                &waypoints,
                0,
                Some(DrivingTarget::StolenHomeFlag(Vec2::new(-300.0, 0.0))),
                &pickups,
                None,
                0.0,
            ),
        );

        assert_eq!(
            target,
            Some(DrivingTarget::StolenHomeFlag(Vec2::new(-300.0, 0.0)))
        );
    }

    #[test]
    fn defender_detours_for_pickup_on_stolen_flag_intercept_lane() {
        let waypoints = [Vec2::new(0.0, 500.0)];
        let pickups = [PickupTarget {
            position: Vec2::new(-80.0, 0.0),
            priority: 150,
        }];
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(
                &waypoints,
                0,
                Some(DrivingTarget::StolenHomeFlag(Vec2::new(-300.0, 0.0))),
                &pickups,
                None,
                0.0,
            ),
        );

        assert_eq!(target, Some(DrivingTarget::Pickup(Vec2::new(-80.0, 0.0))));
    }

    #[test]
    fn home_defender_detours_for_pickup_on_defensive_lane() {
        let waypoints = [Vec2::new(0.0, 500.0)];
        let pickups = [PickupTarget {
            position: Vec2::new(80.0, 0.0),
            priority: 100,
        }];
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(
                &waypoints,
                0,
                Some(DrivingTarget::DefendHomeBase(Vec2::new(300.0, 0.0))),
                &pickups,
                None,
                0.0,
            ),
        );

        assert_eq!(target, Some(DrivingTarget::Pickup(Vec2::new(80.0, 0.0))));
    }

    #[test]
    fn home_defender_ignores_pickup_far_off_defensive_lane() {
        let waypoints = [Vec2::new(0.0, 500.0)];
        let pickups = [PickupTarget {
            position: Vec2::new(80.0, 70.0),
            priority: 100,
        }];
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(
                &waypoints,
                0,
                Some(DrivingTarget::DefendHomeBase(Vec2::new(300.0, 0.0))),
                &pickups,
                None,
                0.0,
            ),
        );

        assert_eq!(
            target,
            Some(DrivingTarget::DefendHomeBase(Vec2::new(300.0, 0.0)))
        );
    }

    #[test]
    fn carrier_returns_enemy_flag_to_home_base() {
        let ai = Entity::from_raw(7);
        let target = choose_capture_the_flag_target(
            ai,
            AiTeam::Red,
            &[
                FlagTarget {
                    team: AiTeam::Blue,
                    home: Vec2::new(-500.0, 0.0),
                    position: Vec2::new(100.0, 0.0),
                    holder: Some(ai),
                },
                FlagTarget {
                    team: AiTeam::Red,
                    home: Vec2::new(500.0, 0.0),
                    position: Vec2::new(500.0, 0.0),
                    holder: None,
                },
            ],
            &[],
        );

        assert_eq!(target, Some(DrivingTarget::HomeBase(Vec2::new(500.0, 0.0))));
    }

    #[test]
    fn carrier_hunts_stolen_home_flag_before_returning_to_base() {
        let ai = Entity::from_raw(7);
        let thief = Entity::from_raw(1);
        let target = choose_capture_the_flag_target(
            ai,
            AiTeam::Red,
            &[
                FlagTarget {
                    team: AiTeam::Blue,
                    home: Vec2::new(-500.0, 0.0),
                    position: Vec2::new(100.0, 0.0),
                    holder: Some(ai),
                },
                FlagTarget {
                    team: AiTeam::Red,
                    home: Vec2::new(500.0, 0.0),
                    position: Vec2::new(-200.0, 0.0),
                    holder: Some(thief),
                },
            ],
            &[],
        );

        assert_eq!(
            target,
            Some(DrivingTarget::StolenHomeFlag(Vec2::new(-340.0, 0.0)))
        );
    }

    #[test]
    fn free_opponent_targets_unheld_enemy_flag() {
        let target = choose_capture_the_flag_target(
            Entity::from_raw(7),
            AiTeam::Red,
            &[
                FlagTarget {
                    team: AiTeam::Blue,
                    home: Vec2::new(-500.0, 0.0),
                    position: Vec2::new(-450.0, 20.0),
                    holder: None,
                },
                FlagTarget {
                    team: AiTeam::Red,
                    home: Vec2::new(500.0, 0.0),
                    position: Vec2::new(500.0, 0.0),
                    holder: None,
                },
            ],
            &[],
        );

        assert_eq!(
            target,
            Some(DrivingTarget::EnemyFlag(Vec2::new(-450.0, 20.0)))
        );
    }

    #[test]
    fn defender_targets_stolen_own_flag_before_enemy_flag() {
        let target = choose_capture_the_flag_target(
            Entity::from_raw(7),
            AiTeam::Red,
            &[
                FlagTarget {
                    team: AiTeam::Blue,
                    home: Vec2::new(-500.0, 0.0),
                    position: Vec2::new(-450.0, 20.0),
                    holder: None,
                },
                FlagTarget {
                    team: AiTeam::Red,
                    home: Vec2::new(500.0, 0.0),
                    position: Vec2::new(100.0, 0.0),
                    holder: Some(Entity::from_raw(1)),
                },
            ],
            &[],
        );

        assert_eq!(
            target,
            Some(DrivingTarget::StolenHomeFlag(Vec2::new(-40.0, 0.0)))
        );
    }

    #[test]
    fn defender_intercepts_stolen_home_flag_towards_enemy_base() {
        let target = choose_capture_the_flag_target(
            Entity::from_raw(7),
            AiTeam::Red,
            &[
                FlagTarget {
                    team: AiTeam::Blue,
                    home: Vec2::new(-500.0, 0.0),
                    position: Vec2::new(-500.0, 0.0),
                    holder: None,
                },
                FlagTarget {
                    team: AiTeam::Red,
                    home: Vec2::new(500.0, 0.0),
                    position: Vec2::new(100.0, 0.0),
                    holder: Some(Entity::from_raw(1)),
                },
            ],
            &[],
        );

        assert_eq!(
            target,
            Some(DrivingTarget::StolenHomeFlag(Vec2::new(-40.0, 0.0)))
        );
    }

    #[test]
    fn defender_recovers_dropped_own_flag_before_enemy_flag() {
        let target = choose_capture_the_flag_target(
            Entity::from_raw(7),
            AiTeam::Red,
            &[
                FlagTarget {
                    team: AiTeam::Blue,
                    home: Vec2::new(-500.0, 0.0),
                    position: Vec2::new(-450.0, 20.0),
                    holder: None,
                },
                FlagTarget {
                    team: AiTeam::Red,
                    home: Vec2::new(500.0, 0.0),
                    position: Vec2::new(100.0, 0.0),
                    holder: None,
                },
            ],
            &[],
        );

        assert_eq!(
            target,
            Some(DrivingTarget::StolenHomeFlag(Vec2::new(100.0, 0.0)))
        );
    }

    #[test]
    fn escorts_teammate_carrying_enemy_flag() {
        let target = choose_capture_the_flag_target(
            Entity::from_raw(7),
            AiTeam::Red,
            &[
                FlagTarget {
                    team: AiTeam::Blue,
                    home: Vec2::new(-500.0, 0.0),
                    position: Vec2::new(-450.0, 20.0),
                    holder: Some(Entity::from_raw(8)),
                },
                FlagTarget {
                    team: AiTeam::Red,
                    home: Vec2::new(500.0, 0.0),
                    position: Vec2::new(500.0, 0.0),
                    holder: None,
                },
            ],
            &[],
        );

        let Some(DrivingTarget::EscortFlagCarrier(position)) = target else {
            panic!("expected escort target, got {target:?}");
        };
        assert_vec2_near(position, Vec2::new(-330.02658, 17.474_243));
    }

    #[test]
    fn defender_protects_home_flag_before_it_is_stolen() {
        let target = choose_capture_the_flag_target(
            Entity::from_raw(7),
            AiTeam::Red,
            &[
                FlagTarget {
                    team: AiTeam::Blue,
                    home: Vec2::new(-500.0, 0.0),
                    position: Vec2::new(-450.0, 20.0),
                    holder: None,
                },
                FlagTarget {
                    team: AiTeam::Red,
                    home: Vec2::new(500.0, 0.0),
                    position: Vec2::new(500.0, 0.0),
                    holder: None,
                },
            ],
            &[ThreatTarget {
                team: AiTeam::Blue,
                position: Vec2::new(300.0, 0.0),
            }],
        );

        assert_eq!(
            target,
            Some(DrivingTarget::DefendHomeBase(Vec2::new(360.0, 0.0)))
        );
    }

    #[test]
    fn defender_intercepts_closest_home_flag_threat() {
        let target = choose_capture_the_flag_target(
            Entity::from_raw(7),
            AiTeam::Red,
            &[
                FlagTarget {
                    team: AiTeam::Blue,
                    home: Vec2::new(-500.0, 0.0),
                    position: Vec2::new(-450.0, 20.0),
                    holder: None,
                },
                FlagTarget {
                    team: AiTeam::Red,
                    home: Vec2::new(500.0, 0.0),
                    position: Vec2::new(500.0, 0.0),
                    holder: None,
                },
            ],
            &[
                ThreatTarget {
                    team: AiTeam::Blue,
                    position: Vec2::new(260.0, 0.0),
                },
                ThreatTarget {
                    team: AiTeam::Blue,
                    position: Vec2::new(500.0, 90.0),
                },
            ],
        );

        assert_eq!(
            target,
            Some(DrivingTarget::DefendHomeBase(Vec2::new(500.0, 90.0)))
        );
    }

    #[test]
    fn defender_ignores_distant_home_flag_threats() {
        let target = choose_capture_the_flag_target(
            Entity::from_raw(7),
            AiTeam::Red,
            &[
                FlagTarget {
                    team: AiTeam::Blue,
                    home: Vec2::new(-500.0, 0.0),
                    position: Vec2::new(-450.0, 20.0),
                    holder: None,
                },
                FlagTarget {
                    team: AiTeam::Red,
                    home: Vec2::new(500.0, 0.0),
                    position: Vec2::new(500.0, 0.0),
                    holder: None,
                },
            ],
            &[ThreatTarget {
                team: AiTeam::Blue,
                position: Vec2::new(-100.0, 0.0),
            }],
        );

        assert_eq!(
            target,
            Some(DrivingTarget::EnemyFlag(Vec2::new(-450.0, 20.0)))
        );
    }
}
