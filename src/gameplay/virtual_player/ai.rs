use bevy::prelude::*;

/// Minimum forward throttle so a virtual player keeps moving (and can therefore
/// keep turning) even when its target is to the side or behind it.
pub const MIN_THROTTLE: f32 = 0.3;

/// Angular error (radians) at which the steering output saturates to full lock.
/// Within this range steering is proportional to the heading error.
pub const STEER_RANGE: f32 = std::f32::consts::FRAC_PI_4;

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
    HomeBase(Vec2),
    EnemyFlag(Vec2),
    PatrolWaypoint(Vec2),
    Pickup(Vec2),
    Player(Vec2),
    StolenHomeFlag(Vec2),
}

impl DrivingTarget {
    #[must_use]
    pub const fn position(self) -> Vec2 {
        match self {
            Self::HomeBase(position)
            | Self::EnemyFlag(position)
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

/// A collectible target visible to virtual players.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickupTarget {
    pub position: Vec2,
    pub bounty: u32,
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
) -> Option<DrivingTarget> {
    let own_flag = flags.iter().find(|flag| flag.team == team)?;
    let enemy_flag = flags.iter().find(|flag| flag.team == team.enemy())?;

    if enemy_flag.holder == Some(ai_entity) {
        return Some(DrivingTarget::HomeBase(own_flag.home));
    }

    if own_flag.holder.is_some() && own_flag.holder != Some(ai_entity) {
        return Some(DrivingTarget::StolenHomeFlag(own_flag.position));
    }

    enemy_flag
        .holder
        .is_none()
        .then_some(DrivingTarget::EnemyFlag(enemy_flag.position))
}

/// Pick the next driving target for a virtual player.
///
/// Valuable nearby pickups take priority over the patrol route so opponents can
/// steal trackside rewards instead of blindly lapping past them. When multiple
/// pickups are in range, virtual players chase the richest bounty first and use
/// distance as the tie-breaker.
#[must_use]
pub fn choose_driving_target(position: Vec2, choices: DrivingChoices<'_>) -> Option<DrivingTarget> {
    let radius_sq = choices.pickup_pursuit_radius * choices.pickup_pursuit_radius;
    let pickup = choices
        .pickups
        .iter()
        .copied()
        .filter_map(|pickup| {
            let distance_sq = position.distance_squared(pickup.position);
            (distance_sq <= radius_sq).then_some((pickup, distance_sq))
        })
        .min_by(|(a_pickup, a_dist), (b_pickup, b_dist)| {
            b_pickup.bounty.cmp(&a_pickup.bounty).then_with(|| {
                a_dist
                    .partial_cmp(b_dist)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        })
        .map(|(pickup, _)| pickup);

    let ctf_target = choices.ctf_target;
    if let Some(target) = ctf_target {
        if target_allows_pickup_detour(position, target, pickup) {
            return pickup.map(|pickup| DrivingTarget::Pickup(pickup.position));
        }
        return Some(target);
    }

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

fn target_allows_pickup_detour(
    position: Vec2,
    target: DrivingTarget,
    pickup: Option<PickupTarget>,
) -> bool {
    matches!(target, DrivingTarget::EnemyFlag(_))
        && pickup.is_some_and(|pickup| {
            position.distance_squared(pickup.position)
                < position.distance_squared(target.position())
        })
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
    let steer = (angle / STEER_RANGE).clamp(-1.0, 1.0);

    // Drive hardest when aligned, but never stall: a car cannot strafe, so it
    // must keep rolling to rotate towards a target that is to the side/behind.
    let throttle = heading.dot(dir).clamp(MIN_THROTTLE, 1.0);

    SteeringIntent { throttle, steer }
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
    fn saturates_steering_when_target_is_behind() {
        // Facing +Y; target directly behind (-Y) but slightly left so the sign
        // is well defined.
        let intent = compute_steering(Vec2::ZERO, Vec2::Y, Vec2::new(-1.0, -500.0), ARRIVE);
        assert!((intent.steer - 1.0).abs() < 1e-4, "steer {}", intent.steer);
        // Still crawls forward so it can spin around.
        assert!(
            (intent.throttle - MIN_THROTTLE).abs() < 1e-4,
            "throttle {}",
            intent.throttle
        );
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
            bounty: 100,
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
                bounty: 25,
            },
            PickupTarget {
                position: Vec2::new(75.0, 0.0),
                bounty: 100,
            },
        ];
        let target = choose_driving_target(
            Vec2::ZERO,
            choices(&waypoints, 0, None, &pickups, None, 0.0),
        );

        assert_eq!(target, Some(DrivingTarget::Pickup(Vec2::new(75.0, 0.0))));
    }

    #[test]
    fn ignores_pickups_outside_pursuit_radius() {
        let waypoint = Vec2::new(500.0, 0.0);
        let waypoints = [waypoint];
        let pickups = [PickupTarget {
            position: Vec2::new(250.0, 0.0),
            bounty: 100,
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
            bounty: 25,
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
            position: Vec2::new(25.0, 0.0),
            bounty: 100,
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

        assert_eq!(target, Some(DrivingTarget::Pickup(Vec2::new(25.0, 0.0))));
    }

    #[test]
    fn flag_carrier_ignores_pickup_detours() {
        let waypoints = [Vec2::new(0.0, 500.0)];
        let pickups = [PickupTarget {
            position: Vec2::new(25.0, 0.0),
            bounty: 100,
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
    fn defender_ignores_pickup_detours() {
        let waypoints = [Vec2::new(0.0, 500.0)];
        let pickups = [PickupTarget {
            position: Vec2::new(25.0, 0.0),
            bounty: 100,
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
        );

        assert_eq!(target, Some(DrivingTarget::HomeBase(Vec2::new(500.0, 0.0))));
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
        );

        assert_eq!(
            target,
            Some(DrivingTarget::StolenHomeFlag(Vec2::new(100.0, 0.0)))
        );
    }

    #[test]
    fn ignores_enemy_flag_carried_by_someone_else() {
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
        );

        assert_eq!(target, None);
    }
}
