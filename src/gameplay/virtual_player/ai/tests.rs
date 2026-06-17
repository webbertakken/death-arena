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
        pickup_pursuit_radius: BASELINE_PICKUP_PURSUIT_RADIUS,
        player_position,
        player_pursuit_radius,
        closing_time_discipline: false,
    }
}

fn assert_vec2_near(actual: Vec2, expected: Vec2) {
    assert!(
        actual.distance(expected) <= EPSILON,
        "actual={actual}, expected={expected}"
    );
}

#[test]
fn compare_positions_orders_by_x_then_y() {
    use std::cmp::Ordering;

    assert_eq!(
        compare_positions(Vec2::new(0.0, 9.0), Vec2::new(1.0, -9.0)),
        Ordering::Less,
        "smaller x sorts first regardless of y"
    );
    assert_eq!(
        compare_positions(Vec2::new(2.0, -50.0), Vec2::new(1.0, 50.0)),
        Ordering::Greater
    );
    assert_eq!(
        compare_positions(Vec2::new(3.0, -1.0), Vec2::new(3.0, 4.0)),
        Ordering::Less,
        "equal x falls back to y"
    );
    assert_eq!(
        compare_positions(Vec2::new(3.0, 4.0), Vec2::new(3.0, 4.0)),
        Ordering::Equal
    );
}

#[test]
fn compare_positions_treats_nan_coordinates_as_equal() {
    use std::cmp::Ordering;

    assert_eq!(
        compare_positions(Vec2::new(f32::NAN, 0.0), Vec2::new(1.0, 0.0)),
        Ordering::Equal,
        "a NaN coordinate must never panic the comparator"
    );
}

#[test]
fn idles_when_within_arrive_radius() {
    let intent = compute_steering(
        Vec2::ZERO,
        Vec2::Y,
        Vec2::new(0.0, 5.0),
        ARRIVE,
        MIN_THROTTLE,
    );
    assert_eq!(intent, SteeringIntent::IDLE);
}

#[test]
fn drives_straight_forward_when_target_is_dead_ahead() {
    // Facing +Y, target far along +Y.
    let intent = compute_steering(
        Vec2::ZERO,
        Vec2::Y,
        Vec2::new(0.0, 500.0),
        ARRIVE,
        MIN_THROTTLE,
    );
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
    let intent = compute_steering(
        Vec2::ZERO,
        Vec2::Y,
        Vec2::new(-500.0, 0.0),
        ARRIVE,
        MIN_THROTTLE,
    );
    assert!(
        intent.steer > 0.0,
        "expected positive steer, got {}",
        intent.steer
    );
}

#[test]
fn steers_right_when_target_is_to_the_right() {
    // Facing +Y; target to the right is +X.
    let intent = compute_steering(
        Vec2::ZERO,
        Vec2::Y,
        Vec2::new(500.0, 0.0),
        ARRIVE,
        MIN_THROTTLE,
    );
    assert!(
        intent.steer < 0.0,
        "expected negative steer, got {}",
        intent.steer
    );
}

#[test]
fn reverses_left_when_target_is_in_left_rear_quarter() {
    let intent = compute_steering(
        Vec2::ZERO,
        Vec2::Y,
        Vec2::new(-500.0, -500.0),
        ARRIVE,
        MIN_THROTTLE,
    );

    assert!((intent.steer + 1.0).abs() < 1e-4, "steer {}", intent.steer);
    assert!(
        (intent.throttle + std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-4,
        "throttle {}",
        intent.throttle
    );
}

#[test]
fn reverses_when_target_is_directly_behind() {
    let intent = compute_steering(
        Vec2::ZERO,
        Vec2::Y,
        Vec2::new(0.0, -500.0),
        ARRIVE,
        MIN_THROTTLE,
    );

    assert!(
        intent.throttle < 0.0,
        "expected reverse throttle, got {}",
        intent.throttle
    );
    assert!(intent.steer.abs() < 1e-4, "steer {}", intent.steer);
}

#[test]
fn never_stalls_when_target_is_perpendicular() {
    let intent = compute_steering(
        Vec2::ZERO,
        Vec2::Y,
        Vec2::new(500.0, 0.0),
        ARRIVE,
        MIN_THROTTLE,
    );
    assert!(intent.throttle >= MIN_THROTTLE);
}

#[test]
fn degenerate_forward_vector_crawls_forward() {
    let intent = compute_steering(
        Vec2::ZERO,
        Vec2::ZERO,
        Vec2::new(0.0, 500.0),
        ARRIVE,
        MIN_THROTTLE,
    );
    assert!(intent.steer.abs() < 1e-4);
    assert!((intent.throttle - MIN_THROTTLE).abs() < 1e-4);
}

#[test]
fn keeps_each_drivers_own_throttle_floor_through_a_corner() {
    // Target square to the side: the car locks to full steer and how much gas
    // it keeps through that turn is its cornering commitment. A reckless driver
    // holds a higher floor than a disciplined one, so each rival takes the
    // corner with its own throttle.
    let target = Vec2::new(500.0, 0.0);
    let reckless = compute_steering(Vec2::ZERO, Vec2::Y, target, ARRIVE, 0.45);
    let disciplined = compute_steering(Vec2::ZERO, Vec2::Y, target, ARRIVE, 0.20);
    assert!(
        reckless.throttle > disciplined.throttle,
        "reckless={}, disciplined={}",
        reckless.throttle,
        disciplined.throttle
    );
    assert!((reckless.throttle - 0.45).abs() < EPSILON);
    assert!((disciplined.throttle - 0.20).abs() < EPSILON);
}

#[test]
fn a_reckless_corner_throttle_buys_a_wider_line() {
    // The genuine trade-off behind the cornering-commitment axis: both drivers
    // lock to the same steer, so faster through the turn means a larger turning
    // radius, a wider line that overshoots the apex. That wider arc is the cost
    // the extra corner speed is bought with, never a strict upgrade.
    let speed = 400.0;
    let rotation = f32::to_radians(300.0);
    let target = Vec2::new(500.0, 0.0);
    let reckless = compute_steering(Vec2::ZERO, Vec2::Y, target, ARRIVE, 0.45);
    let disciplined = compute_steering(Vec2::ZERO, Vec2::Y, target, ARRIVE, 0.20);
    // Both saturate to full lock, so the only difference is throttle; turning
    // radius is forward speed divided by angular speed.
    let turning_radius =
        |intent: SteeringIntent| (intent.throttle * speed) / (intent.steer.abs() * rotation);
    assert!(
        turning_radius(reckless) > turning_radius(disciplined),
        "reckless radius {} must exceed disciplined {}",
        turning_radius(reckless),
        turning_radius(disciplined)
    );
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
fn equal_value_and_distance_pickup_choice_is_stable() {
    let waypoints = [Vec2::new(500.0, 0.0)];
    let pickups = [
        PickupTarget {
            position: Vec2::new(0.0, 50.0),
            priority: 100,
        },
        PickupTarget {
            position: Vec2::new(0.0, -50.0),
            priority: 100,
        },
    ];
    let target = choose_driving_target(
        Vec2::ZERO,
        choices(&waypoints, 0, None, &pickups, None, 0.0),
    );

    assert_eq!(target, Some(DrivingTarget::Pickup(Vec2::new(0.0, -50.0))));
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
    let waypoint = Vec2::new(700.0, 0.0);
    let waypoints = [waypoint];
    // The bag sits beyond the baseline scavenging reach (500 >
    // BASELINE_PICKUP_PURSUIT_RADIUS), so the car leaves it and patrols on.
    let pickups = [PickupTarget {
        position: Vec2::new(500.0, 0.0),
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
fn attacker_detours_wider_for_high_value_pickup_on_flag_push() {
    let waypoints = [Vec2::new(0.0, 500.0)];
    let pickups = [PickupTarget {
        position: Vec2::new(50.0, 70.0),
        priority: 150,
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

    assert_eq!(target, Some(DrivingTarget::Pickup(Vec2::new(50.0, 70.0))));
}

#[test]
fn a_greedy_driver_swings_wider_off_the_flag_lane_than_the_baseline() {
    // A pickup sitting just outside the baseline detour lane (lateral 65 >
    // CTF_PICKUP_LANE_WIDTH 60) is left on the track by a neutral driver but
    // grabbed by a greedier one, whose wider greed widens its in-objective
    // detour lane just as it widens its trackside scavenging reach.
    let waypoints = [Vec2::new(0.0, 500.0)];
    let flag = DrivingTarget::EnemyFlag(Vec2::new(300.0, 0.0));
    let pickups = [PickupTarget {
        position: Vec2::new(40.0, 65.0),
        priority: CTF_PICKUP_DETOUR_MIN_PRIORITY,
    }];

    let baseline = choose_driving_target(
        Vec2::ZERO,
        DrivingChoices {
            pickup_pursuit_radius: 450.0,
            ..choices(&waypoints, 0, Some(flag), &pickups, None, 0.0)
        },
    );
    assert_eq!(
        baseline,
        Some(flag),
        "a neutral driver keeps its line and leaves a bag just off the lane"
    );

    let greedy = choose_driving_target(
        Vec2::ZERO,
        DrivingChoices {
            pickup_pursuit_radius: 520.0,
            ..choices(&waypoints, 0, Some(flag), &pickups, None, 0.0)
        },
    );
    assert_eq!(
        greedy,
        Some(DrivingTarget::Pickup(Vec2::new(40.0, 65.0))),
        "a greedier driver swings wider off its line to scoop the same bag"
    );
}

#[test]
fn a_disciplined_driver_keeps_a_tighter_flag_lane_than_the_baseline() {
    // A pickup just inside the baseline detour lane (lateral 55 <
    // CTF_PICKUP_LANE_WIDTH 60) is taken by a neutral driver but left by a
    // more disciplined one, whose lower greed narrows its in-objective detour
    // lane so it stays committed to the flag run.
    let waypoints = [Vec2::new(0.0, 500.0)];
    let flag = DrivingTarget::EnemyFlag(Vec2::new(300.0, 0.0));
    let pickups = [PickupTarget {
        position: Vec2::new(40.0, 55.0),
        priority: CTF_PICKUP_DETOUR_MIN_PRIORITY,
    }];

    let baseline = choose_driving_target(
        Vec2::ZERO,
        DrivingChoices {
            pickup_pursuit_radius: 450.0,
            ..choices(&waypoints, 0, Some(flag), &pickups, None, 0.0)
        },
    );
    assert_eq!(
        baseline,
        Some(DrivingTarget::Pickup(Vec2::new(40.0, 55.0))),
        "a neutral driver detours for a bag sitting inside its lane"
    );

    let disciplined = choose_driving_target(
        Vec2::ZERO,
        DrivingChoices {
            pickup_pursuit_radius: 380.0,
            ..choices(&waypoints, 0, Some(flag), &pickups, None, 0.0)
        },
    );
    assert_eq!(
        disciplined,
        Some(flag),
        "a disciplined driver keeps a tighter line and stays on the flag run"
    );
}

#[test]
fn the_baseline_driver_keeps_the_unscaled_detour_lane() {
    // A driver with exactly the baseline greed detours within the original
    // fixed lane widths, so the all-rounder and the human (both at the
    // baseline) are untouched by the greed scaling.
    assert!(
        (pickup_lane_width(
            CTF_PICKUP_DETOUR_MIN_PRIORITY,
            BASELINE_PICKUP_PURSUIT_RADIUS
        ) - CTF_PICKUP_LANE_WIDTH)
            .abs()
            <= EPSILON
    );
    assert!(
        (pickup_lane_width(CTF_WIDE_DETOUR_MIN_PRIORITY, BASELINE_PICKUP_PURSUIT_RADIUS)
            - CTF_HIGH_VALUE_PICKUP_LANE_WIDTH)
            .abs()
            <= EPSILON
    );
}

#[test]
fn greed_widens_and_discipline_narrows_the_detour_lane() {
    // Greed scales the detour lane the same way it scales the trackside reach:
    // the greediest driver swings widest off its objective line, the most
    // disciplined keeps the tightest, with the baseline between.
    let greedy = pickup_lane_width(CTF_PICKUP_DETOUR_MIN_PRIORITY, 520.0);
    let baseline = pickup_lane_width(
        CTF_PICKUP_DETOUR_MIN_PRIORITY,
        BASELINE_PICKUP_PURSUIT_RADIUS,
    );
    let disciplined = pickup_lane_width(CTF_PICKUP_DETOUR_MIN_PRIORITY, 380.0);
    assert!(
        greedy > baseline && baseline > disciplined,
        "expected greed to order the narrow lane, got greedy={greedy}, \
             baseline={baseline}, disciplined={disciplined}"
    );

    let greedy_wide = pickup_lane_width(CTF_WIDE_DETOUR_MIN_PRIORITY, 520.0);
    let disciplined_wide = pickup_lane_width(CTF_WIDE_DETOUR_MIN_PRIORITY, 380.0);
    assert!(
        greedy_wide > CTF_HIGH_VALUE_PICKUP_LANE_WIDTH
            && CTF_HIGH_VALUE_PICKUP_LANE_WIDTH > disciplined_wide,
        "the high-value lane scales with greed too, got greedy_wide={greedy_wide}, \
             disciplined_wide={disciplined_wide}"
    );
}

#[test]
fn the_lane_scale_clamps_a_degenerate_greed() {
    // The clamp is a pure safety net: a zero or absurd radius can never
    // collapse the lane to nothing nor blow it out across the arena.
    assert!((greed_lane_scale(0.0) - GREED_LANE_SCALE_MIN).abs() <= EPSILON);
    assert!((greed_lane_scale(100_000.0) - GREED_LANE_SCALE_MAX).abs() <= EPSILON);
    assert!(pickup_lane_width(CTF_PICKUP_DETOUR_MIN_PRIORITY, 0.0) > 0.0);
}

#[test]
fn the_roster_greed_band_never_trips_the_lane_clamp() {
    // The asserted roster greed band (340..=580, see spawn.rs) maps strictly
    // inside the safety clamp, so personality is fully expressed across the
    // whole roster and the clamp only ever guards a degenerate radius.
    for greed in [340.0_f32, 580.0_f32] {
        let raw = greed / BASELINE_PICKUP_PURSUIT_RADIUS;
        assert!(
            (greed_lane_scale(greed) - raw).abs() <= EPSILON,
            "roster greed {greed} should scale unclamped, raw={raw}, clamped={}",
            greed_lane_scale(greed)
        );
    }
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
fn closing_time_raises_the_detour_bar_to_the_wide_threshold() {
    assert_eq!(
        closing_time_detour_min_priority(false, BASELINE_PICKUP_PURSUIT_RADIUS),
        CTF_PICKUP_DETOUR_MIN_PRIORITY,
        "normal play breaks off for any pickup worth the base detour"
    );
    assert_eq!(
        closing_time_detour_min_priority(true, BASELINE_PICKUP_PURSUIT_RADIUS),
        CTF_WIDE_DETOUR_MIN_PRIORITY,
        "a baseline driver in closing time only breaks off for a wide-detour grab"
    );
}

#[test]
fn normal_play_keeps_the_base_detour_bar_for_every_personality() {
    // Outside closing time the bar never depends on greed: every driver, from
    // the greediest sprinter to the most disciplined technician, breaks off for
    // any pickup worth the base detour.
    for greed in [380.0_f32, BASELINE_PICKUP_PURSUIT_RADIUS, 520.0_f32] {
        assert_eq!(
            closing_time_detour_min_priority(false, greed),
            CTF_PICKUP_DETOUR_MIN_PRIORITY,
            "greed {greed} should not move the normal-play bar"
        );
    }
}

#[test]
fn greed_scales_the_closing_time_detour_bar_around_the_baseline() {
    // In closing time a greedy driver keeps a lower bar (still gambles) and a
    // disciplined one a higher bar (locks down), with the baseline driver
    // exactly on the neutral wide bar so it and the human are unchanged.
    let greedy = closing_time_detour_min_priority(true, 520.0);
    let baseline = closing_time_detour_min_priority(true, BASELINE_PICKUP_PURSUIT_RADIUS);
    let disciplined = closing_time_detour_min_priority(true, 380.0);

    assert_eq!(baseline, CTF_WIDE_DETOUR_MIN_PRIORITY);
    assert_eq!(greedy, CLOSING_TIME_GREEDY_DETOUR_MIN_PRIORITY);
    assert_eq!(disciplined, CLOSING_TIME_DISCIPLINED_DETOUR_MIN_PRIORITY);
    assert!(
        greedy < baseline && baseline < disciplined,
        "greed must order the closing-time bar without inverting discipline, got \
             greedy={greedy}, baseline={baseline}, disciplined={disciplined}"
    );
}

#[test]
fn a_baseline_driver_drafts_with_the_baseline_cone() {
    // The all-rounder's greed (and the human that mirrors it) keeps the exact
    // baseline cone, so its active drafting is unchanged.
    assert!(
        (draft_seek_cone(BASELINE_PICKUP_PURSUIT_RADIUS) - DRAFT_SEEK_MIN_AIM_COURSE_DOT).abs()
            <= EPSILON
    );
}

#[test]
fn a_greedy_driver_drafts_with_a_wider_cone() {
    // The sprinter's roster greed (520, well above the baseline 450) buys the
    // widest cone, so it tolerates a larger swing off its line to bank a tow.
    let cone = draft_seek_cone(520.0);
    assert!((cone - DRAFT_SEEK_GREEDY_MIN_AIM_COURSE_DOT).abs() <= EPSILON);
    assert!(
        cone < DRAFT_SEEK_MIN_AIM_COURSE_DOT,
        "a greedy cone must be wider (a smaller dot) than the baseline: {cone}"
    );
}

#[test]
fn a_disciplined_driver_drafts_with_a_tighter_cone() {
    // The technician's roster greed (380, below the baseline) holds the tightest
    // cone, keeping the straightest line to its objective.
    let cone = draft_seek_cone(380.0);
    assert!((cone - DRAFT_SEEK_DISCIPLINED_MIN_AIM_COURSE_DOT).abs() <= EPSILON);
    assert!(
        cone > DRAFT_SEEK_MIN_AIM_COURSE_DOT,
        "a disciplined cone must be tighter (a larger dot) than the baseline: {cone}"
    );
}

#[test]
fn greed_never_inverts_the_draft_cone() {
    // Across the asserted roster greed band, a greedier driver never gets a
    // tighter cone than a more disciplined one: greed only ever widens a draft.
    let radii = [340.0_f32, 380.0, 400.0, 450.0, 520.0, 580.0];
    for pair in radii.windows(2) {
        let (less_greedy, greedier) = (pair[0], pair[1]);
        assert!(
            draft_seek_cone(greedier) <= draft_seek_cone(less_greedy),
            "greedier {greedier} cone {} must not exceed less-greedy {less_greedy} cone {}",
            draft_seek_cone(greedier),
            draft_seek_cone(less_greedy),
        );
    }
}

#[test]
fn the_closing_time_bar_never_drops_below_normal_play_discipline() {
    // Even the greediest legal driver still disciplines its detours in closing
    // time: its bar stays above the normal-play bar, so a cash bag is always
    // left on the track when the clock is running down.
    let greedy = closing_time_detour_min_priority(true, 580.0);
    assert!(
        greedy > closing_time_detour_min_priority(false, 580.0),
        "closing time must always raise the bar above normal play, got {greedy}"
    );
}

#[test]
fn a_greedy_driver_still_gambles_on_a_sabotage_grab_in_closing_time() {
    use crate::gameplay::pickup::PickupKind;

    // A sabotage-grade pickup (130) sitting square on the flag lane: the
    // neutral driver leaves it (its closing-time bar is the wide 150) while a
    // greedy sprinter still breaks off for it (its bar drops to 130).
    let waypoints = [Vec2::new(0.0, 500.0)];
    let flag = DrivingTarget::EnemyFlag(Vec2::new(0.0, 300.0));
    let on_lane = Vec2::new(0.0, 80.0);
    let sabotage = [PickupTarget {
        position: on_lane,
        priority: PickupKind::Sabotage.virtual_player_priority(),
    }];

    assert_eq!(
        choose_driving_target(
            Vec2::ZERO,
            DrivingChoices {
                closing_time_discipline: true,
                pickup_pursuit_radius: BASELINE_PICKUP_PURSUIT_RADIUS,
                ..choices(&waypoints, 0, Some(flag), &sabotage, None, 0.0)
            },
        ),
        Some(flag),
        "a neutral driver in closing time leaves a sabotage grab and commits"
    );
    assert_eq!(
        choose_driving_target(
            Vec2::ZERO,
            DrivingChoices {
                closing_time_discipline: true,
                pickup_pursuit_radius: 520.0,
                ..choices(&waypoints, 0, Some(flag), &sabotage, None, 0.0)
            },
        ),
        Some(DrivingTarget::Pickup(on_lane)),
        "a greedy driver in closing time still gambles on the sabotage grab"
    );
}

#[test]
fn a_disciplined_driver_leaves_even_a_nitro_in_closing_time() {
    use crate::gameplay::pickup::PickupKind;

    // A nitro (150) square on the flag lane: the neutral driver grabs it (its
    // closing-time bar is the wide 150) while a disciplined technician leaves
    // it to race the flag home (its bar rises to 170).
    let waypoints = [Vec2::new(0.0, 500.0)];
    let flag = DrivingTarget::EnemyFlag(Vec2::new(0.0, 300.0));
    let on_lane = Vec2::new(0.0, 80.0);
    let nitro = [PickupTarget {
        position: on_lane,
        priority: PickupKind::Nitro.virtual_player_priority(),
    }];

    assert_eq!(
        choose_driving_target(
            Vec2::ZERO,
            DrivingChoices {
                closing_time_discipline: true,
                pickup_pursuit_radius: BASELINE_PICKUP_PURSUIT_RADIUS,
                ..choices(&waypoints, 0, Some(flag), &nitro, None, 0.0)
            },
        ),
        Some(DrivingTarget::Pickup(on_lane)),
        "a neutral driver in closing time still grabs a nitro that speeds the push"
    );
    assert_eq!(
        choose_driving_target(
            Vec2::ZERO,
            DrivingChoices {
                closing_time_discipline: true,
                pickup_pursuit_radius: 380.0,
                ..choices(&waypoints, 0, Some(flag), &nitro, None, 0.0)
            },
        ),
        Some(flag),
        "a disciplined driver in closing time leaves even a nitro and commits"
    );
}

#[test]
fn committed_attacker_leaves_cash_but_still_grabs_nitro() {
    use crate::gameplay::pickup::PickupKind;

    let waypoints = [Vec2::new(0.0, 500.0)];
    let flag = DrivingTarget::EnemyFlag(Vec2::new(0.0, 300.0));
    let on_lane = Vec2::new(0.0, 80.0);

    let cash = [PickupTarget {
        position: on_lane,
        priority: PickupKind::Cash.virtual_player_priority(),
    }];
    assert_eq!(
        choose_driving_target(
            Vec2::ZERO,
            choices(&waypoints, 0, Some(flag), &cash, None, 0.0),
        ),
        Some(DrivingTarget::Pickup(on_lane)),
        "normal play still grabs a cash bag sitting on the flag lane"
    );
    assert_eq!(
        choose_driving_target(
            Vec2::ZERO,
            DrivingChoices {
                closing_time_discipline: true,
                ..choices(&waypoints, 0, Some(flag), &cash, None, 0.0)
            },
        ),
        Some(flag),
        "a team racing the clock leaves the cash and commits to the flag"
    );

    let nitro = [PickupTarget {
        position: on_lane,
        priority: PickupKind::Nitro.virtual_player_priority(),
    }];
    assert_eq!(
        choose_driving_target(
            Vec2::ZERO,
            DrivingChoices {
                closing_time_discipline: true,
                ..choices(&waypoints, 0, Some(flag), &nitro, None, 0.0)
            },
        ),
        Some(DrivingTarget::Pickup(on_lane)),
        "a committed team still grabs nitro that speeds the push home"
    );
}

#[test]
fn committed_battered_attacker_still_grabs_a_survival_repair() {
    use crate::gameplay::pickup::PickupKind;

    let waypoints = [Vec2::new(0.0, 500.0)];
    let flag = DrivingTarget::EnemyFlag(Vec2::new(0.0, 300.0));
    let repair = [PickupTarget {
        position: Vec2::new(0.0, 80.0),
        // A wrecked team rates a repair above the wide-detour bar.
        priority: PickupKind::Repair.virtual_player_priority_for_integrity(0.0),
    }];

    assert_eq!(
        choose_driving_target(
            Vec2::ZERO,
            DrivingChoices {
                closing_time_discipline: true,
                ..choices(&waypoints, 0, Some(flag), &repair, None, 0.0)
            },
        ),
        Some(DrivingTarget::Pickup(Vec2::new(0.0, 80.0))),
        "commitment must not suicide: a wrecked car still patches up to finish the run"
    );
}

#[test]
fn battered_attacker_detours_for_a_repair_a_healthy_one_ignores() {
    use crate::gameplay::pickup::PickupKind;

    let waypoints = [Vec2::new(0.0, 500.0)];
    let repair = Vec2::new(0.0, 80.0);
    let flag = DrivingTarget::EnemyFlag(Vec2::new(0.0, 300.0));

    let healthy = [PickupTarget {
        position: repair,
        priority: PickupKind::Repair.virtual_player_priority_for_integrity(1.0),
    }];
    assert_eq!(
        choose_driving_target(
            Vec2::ZERO,
            choices(&waypoints, 0, Some(flag), &healthy, None, 0.0),
        ),
        Some(flag),
        "a pristine attacker should stay on the flag run"
    );

    let battered = [PickupTarget {
        position: repair,
        priority: PickupKind::Repair.virtual_player_priority_for_integrity(0.0),
    }];
    assert_eq!(
        choose_driving_target(
            Vec2::ZERO,
            choices(&waypoints, 0, Some(flag), &battered, None, 0.0),
        ),
        Some(DrivingTarget::Pickup(repair)),
        "a wrecked attacker should break off to patch up"
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
fn midfield_interceptor_detours_for_pickup_on_intercept_lane() {
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
            Some(DrivingTarget::MidfieldInterceptor(Vec2::new(300.0, 0.0))),
            &pickups,
            None,
            0.0,
        ),
    );

    assert_eq!(target, Some(DrivingTarget::Pickup(Vec2::new(80.0, 0.0))));
}

#[test]
fn stolen_flag_route_guard_detours_for_pickup_on_guard_lane() {
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
            Some(DrivingTarget::StolenHomeFlagRouteGuard(Vec2::new(
                300.0, 0.0,
            ))),
            &pickups,
            None,
            0.0,
        ),
    );

    assert_eq!(target, Some(DrivingTarget::Pickup(Vec2::new(80.0, 0.0))));
}

#[test]
fn escort_detours_for_pickup_on_flag_carrier_lane() {
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
            Some(DrivingTarget::EscortFlagCarrier(Vec2::new(300.0, 0.0))),
            &pickups,
            None,
            0.0,
        ),
    );

    assert_eq!(target, Some(DrivingTarget::Pickup(Vec2::new(80.0, 0.0))));
}

#[test]
fn urgent_home_defender_ignores_pickup_detours() {
    let waypoints = [Vec2::new(0.0, 500.0)];
    let pickups = [PickupTarget {
        position: Vec2::new(80.0, 0.0),
        priority: 150,
    }];
    let target = choose_driving_target(
        Vec2::ZERO,
        choices(
            &waypoints,
            0,
            Some(DrivingTarget::UrgentDefendHomeBase(Vec2::new(300.0, 0.0))),
            &pickups,
            None,
            0.0,
        ),
    );

    assert_eq!(
        target,
        Some(DrivingTarget::UrgentDefendHomeBase(Vec2::new(300.0, 0.0)))
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
        MIN_THROTTLE,
    );

    assert_eq!(target, Some(DrivingTarget::HomeBase(Vec2::new(500.0, 0.0))));
}

#[test]
fn carrier_home_run_aim_targets_base_when_the_lane_is_clear() {
    let aim = carrier_home_run_aim(
        Vec2::new(0.0, 0.0),
        Vec2::new(400.0, 0.0),
        AiTeam::Red,
        &[],
        MIN_THROTTLE,
    );

    assert_vec2_near(aim, Vec2::new(400.0, 0.0));
}

#[test]
fn carrier_home_run_aim_jukes_around_an_enemy_dead_on_the_line() {
    let aim = carrier_home_run_aim(
        Vec2::new(0.0, 0.0),
        Vec2::new(400.0, 0.0),
        AiTeam::Red,
        &[ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(200.0, 0.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    // A blocker dead on the line picks a deterministic side so the carrier
    // still commits to a dodge rather than stalling head-on into it.
    assert_vec2_near(aim, Vec2::new(400.0, CARRIER_JUKE_OFFSET));
}

#[test]
fn carrier_juke_offset_at_the_neutral_floor_is_the_baseline() {
    // A driver cornering on the neutral MIN_THROTTLE floor (the all-rounder and
    // the human's mirror) swings the exact, unchanged baseline berth.
    assert!((carrier_juke_offset(MIN_THROTTLE) - CARRIER_JUKE_OFFSET).abs() <= f32::EPSILON);
}

#[test]
fn carrier_juke_offset_tightens_with_commitment() {
    // A reckless driver (a higher corner_throttle) squeezes a tighter line home;
    // a disciplined one swings a wider berth. Sampled across the roster band
    // (technician 0.20 .. all-rounder 0.30 .. sprinter 0.42).
    let reckless = carrier_juke_offset(0.42);
    let neutral = carrier_juke_offset(MIN_THROTTLE);
    let disciplined = carrier_juke_offset(0.20);
    assert!(
        reckless < neutral && neutral < disciplined,
        "reckless={reckless}, neutral={neutral}, disciplined={disciplined}"
    );
}

#[test]
fn carrier_juke_offset_always_aims_at_or_outside_ram_range() {
    // Even a degenerate throttle must keep the aim at or outside true ram range
    // (so the arc rounds the blocker, never points straight through it) and
    // inside the sane berth band.
    let ram_radius = crate::gameplay::combat::RAM_RADIUS;
    for throttle in [-5.0, 0.0, 0.15, 0.2, MIN_THROTTLE, 0.42, 0.5, 1.0, 5.0] {
        let offset = carrier_juke_offset(throttle);
        assert!(
            offset >= ram_radius,
            "throttle={throttle} gave {offset}, inside ram range {ram_radius}"
        );
        assert!(
            (CARRIER_JUKE_OFFSET_MIN..=CARRIER_JUKE_OFFSET_MAX).contains(&offset),
            "throttle={throttle} gave {offset}, outside the berth band"
        );
    }
}

#[test]
fn carrier_juke_offset_clamps_a_degenerate_throttle_to_the_band() {
    // The clamp is a safety net: an absurdly reckless throttle bottoms out at the
    // tightest berth, an absurdly timid one tops out at the widest.
    assert!((carrier_juke_offset(99.0) - CARRIER_JUKE_OFFSET_MIN).abs() <= f32::EPSILON);
    assert!((carrier_juke_offset(-99.0) - CARRIER_JUKE_OFFSET_MAX).abs() <= f32::EPSILON);
}

#[test]
fn carrier_juke_offset_never_clamps_a_real_drivers_line() {
    // Every driver the roster actually fields, from the technician's careful 0.20
    // through the ambusher's 0.38 to the sprinter's reckless 0.42, flexes its line
    // by the raw affine map, landing strictly inside the clamp band and so left
    // untouched by it. The clamp is a pure safety net for a throttle past the
    // whole roster, never a real driver's line, so the floor and ceiling docs hold.
    for corner_throttle in [0.20_f32, MIN_THROTTLE, 0.38, 0.42] {
        let offset = carrier_juke_offset(corner_throttle);
        assert!(
            offset > CARRIER_JUKE_OFFSET_MIN && offset < CARRIER_JUKE_OFFSET_MAX,
            "throttle={corner_throttle} gave {offset}, which the clamp touched"
        );
    }
}

#[test]
fn a_committed_carrier_jukes_a_tighter_line_than_a_disciplined_one() {
    // Same dead-on roadblock, two personalities: the reckless carrier squeezes a
    // tighter, faster arc (a smaller swing off the line home) than the
    // disciplined one, proving the commitment flex threads through the carrier
    // run-home aim.
    let juke = |corner_throttle| {
        carrier_home_run_aim(
            Vec2::new(0.0, 0.0),
            Vec2::new(400.0, 0.0),
            AiTeam::Red,
            &[ThreatTarget {
                team: AiTeam::Blue,
                position: Vec2::new(200.0, 0.0),
                velocity: Vec2::ZERO,
            }],
            corner_throttle,
        )
        .y
        .abs()
    };
    let reckless = juke(0.42);
    let disciplined = juke(0.20);
    assert!(
        reckless < disciplined,
        "reckless swing={reckless}, disciplined swing={disciplined}"
    );
}

#[test]
fn a_committed_pit_retreat_jukes_a_tighter_line_than_a_disciplined_one() {
    // The retreating car weaves home on the same commitment-flexed line as a
    // flag carrier, so the reckless retreat also squeezes a tighter arc.
    let juke = |corner_throttle| {
        pit_retreat_home_run_aim(
            Vec2::new(0.0, 0.0),
            Vec2::new(400.0, 0.0),
            AiTeam::Red,
            &[ThreatTarget {
                team: AiTeam::Blue,
                position: Vec2::new(200.0, 0.0),
                velocity: Vec2::ZERO,
            }],
            corner_throttle,
        )
        .y
        .abs()
    };
    assert!(juke(0.42) < juke(0.20));
}

#[test]
fn carrier_home_run_aim_swings_away_from_an_enemy_off_to_one_side() {
    let aim = carrier_home_run_aim(
        Vec2::new(0.0, 0.0),
        Vec2::new(400.0, 0.0),
        AiTeam::Red,
        &[ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(200.0, 40.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    // Blocker is to the left of the run home, so the carrier swings right.
    assert_vec2_near(aim, Vec2::new(400.0, -CARRIER_JUKE_OFFSET));
}

#[test]
fn carrier_home_run_aim_commits_straight_when_close_to_base() {
    let aim = carrier_home_run_aim(
        Vec2::new(0.0, 0.0),
        Vec2::new(150.0, 0.0),
        AiTeam::Red,
        &[ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(75.0, 0.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    assert_vec2_near(aim, Vec2::new(150.0, 0.0));
}

#[test]
fn carrier_home_run_aim_ignores_friendly_cars_on_the_line() {
    let aim = carrier_home_run_aim(
        Vec2::new(0.0, 0.0),
        Vec2::new(400.0, 0.0),
        AiTeam::Red,
        &[ThreatTarget {
            team: AiTeam::Red,
            position: Vec2::new(200.0, 0.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    assert_vec2_near(aim, Vec2::new(400.0, 0.0));
}

#[test]
fn carrier_home_run_aim_ignores_enemies_behind_or_beyond_the_base() {
    let behind = carrier_home_run_aim(
        Vec2::new(0.0, 0.0),
        Vec2::new(400.0, 0.0),
        AiTeam::Red,
        &[ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(-100.0, 0.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );
    assert_vec2_near(behind, Vec2::new(400.0, 0.0));

    let beyond = carrier_home_run_aim(
        Vec2::new(0.0, 0.0),
        Vec2::new(400.0, 0.0),
        AiTeam::Red,
        &[ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(500.0, 0.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );
    assert_vec2_near(beyond, Vec2::new(400.0, 0.0));
}

#[test]
fn carrier_home_run_aim_ignores_enemies_outside_the_lane() {
    let aim = carrier_home_run_aim(
        Vec2::new(0.0, 0.0),
        Vec2::new(400.0, 0.0),
        AiTeam::Red,
        &[ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(200.0, CARRIER_JUKE_LANE_WIDTH + 10.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    assert_vec2_near(aim, Vec2::new(400.0, 0.0));
}

#[test]
fn pit_retreat_home_run_aim_targets_base_when_the_lane_is_clear() {
    let aim = pit_retreat_home_run_aim(
        Vec2::new(0.0, 0.0),
        Vec2::new(400.0, 0.0),
        AiTeam::Red,
        &[],
        MIN_THROTTLE,
    );

    assert_vec2_near(aim, Vec2::new(400.0, 0.0));
}

#[test]
fn pit_retreat_home_run_aim_jukes_around_an_enemy_dead_on_the_line() {
    let aim = pit_retreat_home_run_aim(
        Vec2::new(0.0, 0.0),
        Vec2::new(400.0, 0.0),
        AiTeam::Red,
        &[ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(200.0, 0.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    // A blocker dead on the limp home picks a deterministic side, so the
    // battered car still commits to a dodge rather than stalling head-on into
    // the very foe it is trying to escape.
    assert_vec2_near(aim, Vec2::new(400.0, CARRIER_JUKE_OFFSET));
}

#[test]
fn pit_retreat_home_run_aim_swings_away_from_an_enemy_off_to_one_side() {
    let aim = pit_retreat_home_run_aim(
        Vec2::new(0.0, 0.0),
        Vec2::new(400.0, 0.0),
        AiTeam::Red,
        &[ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(200.0, 40.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    // Blocker is to the left of the run home, so the limping car swings right.
    assert_vec2_near(aim, Vec2::new(400.0, -CARRIER_JUKE_OFFSET));
}

#[test]
fn pit_retreat_home_run_aim_commits_straight_inside_the_recovery_zone() {
    // Once within its base recovery zone the car straightens up and parks in
    // the pit, ignoring a blocker it has effectively already cleared, so it
    // never circles home dodging a tail.
    let close = PIT_RETREAT_HOME_COMMIT_DISTANCE - 10.0;
    let aim = pit_retreat_home_run_aim(
        Vec2::new(0.0, 0.0),
        Vec2::new(close, 0.0),
        AiTeam::Red,
        &[ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(close / 2.0, 0.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    assert_vec2_near(aim, Vec2::new(close, 0.0));
}

#[test]
fn pit_retreat_home_run_aim_ignores_friendly_cars_on_the_line() {
    let aim = pit_retreat_home_run_aim(
        Vec2::new(0.0, 0.0),
        Vec2::new(400.0, 0.0),
        AiTeam::Red,
        &[ThreatTarget {
            team: AiTeam::Red,
            position: Vec2::new(200.0, 0.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    assert_vec2_near(aim, Vec2::new(400.0, 0.0));
}

#[test]
fn pit_retreat_home_run_aim_ignores_enemies_outside_the_lane() {
    let aim = pit_retreat_home_run_aim(
        Vec2::new(0.0, 0.0),
        Vec2::new(400.0, 0.0),
        AiTeam::Red,
        &[ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(200.0, CARRIER_JUKE_LANE_WIDTH + 10.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    assert_vec2_near(aim, Vec2::new(400.0, 0.0));
}

#[test]
fn carrier_jukes_around_an_enemy_planted_on_the_run_home() {
    let ai = Entity::from_raw(7);
    let target = choose_capture_the_flag_target(
        ai,
        AiTeam::Red,
        &[
            FlagTarget {
                team: AiTeam::Blue,
                home: Vec2::new(-500.0, 0.0),
                position: Vec2::new(0.0, 0.0),
                holder: Some(ai),
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
            position: Vec2::new(250.0, 0.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    assert_eq!(
        target,
        Some(DrivingTarget::HomeBase(Vec2::new(
            500.0,
            CARRIER_JUKE_OFFSET
        )))
    );
}

#[test]
fn carrier_stages_outside_contested_home_base_before_scoring() {
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
        &[ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(430.0, 0.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    assert_eq!(
        target,
        Some(DrivingTarget::ContestedHomeBaseStaging(Vec2::new(
            740.0, 0.0
        )))
    );
}

#[test]
fn carrier_staging_boundary_tracks_the_capture_blocking_radius() {
    // The carrier's stage-or-commit read must match the rule that actually denies
    // the score: an enemy inside the capture-blocking radius blocks the capture, so
    // the carrier stages outside; one just beyond it cannot, so the carrier commits
    // home. The boundary is computed from the capture rule's own
    // `BASE_CAPTURE_RADIUS`, so this fails the instant the AI's contest reach drifts
    // from it in either direction, locking the two together end to end.
    use crate::gameplay::ctf::BASE_CAPTURE_RADIUS;

    let ai = Entity::from_raw(7);
    let home = Vec2::new(500.0, 0.0);
    let carrier = Vec2::new(300.0, 0.0);

    let decide = |contester_x: f32| {
        choose_capture_the_flag_target(
            ai,
            AiTeam::Red,
            &[
                FlagTarget {
                    team: AiTeam::Blue,
                    home: Vec2::new(-500.0, 0.0),
                    position: carrier,
                    holder: Some(ai),
                },
                FlagTarget {
                    team: AiTeam::Red,
                    home,
                    position: home,
                    holder: None,
                },
            ],
            &[ThreatTarget {
                team: AiTeam::Blue,
                position: Vec2::new(contester_x, 0.0),
                velocity: Vec2::ZERO,
            }],
            MIN_THROTTLE,
        )
    };

    let blocked = decide(home.x - (BASE_CAPTURE_RADIUS - 1.0));
    assert!(
        matches!(blocked, Some(DrivingTarget::ContestedHomeBaseStaging(_))),
        "an enemy inside the capture-blocking radius must make the carrier stage, got {blocked:?}"
    );

    let clear = decide(home.x - (BASE_CAPTURE_RADIUS + 1.0));
    assert!(
        matches!(clear, Some(DrivingTarget::HomeBase(_))),
        "an enemy beyond the capture-blocking radius must let the carrier commit, got {clear:?}"
    );
}

#[test]
fn carrier_intercepts_stolen_home_flag_before_scoring() {
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
        MIN_THROTTLE,
    );

    assert_eq!(
        target,
        Some(DrivingTarget::StolenHomeFlag(Vec2::new(-340.0, 0.0)))
    );
}

#[test]
fn carrier_recovers_dropped_home_flag_before_scoring() {
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
                position: Vec2::new(260.0, 0.0),
                holder: None,
            },
        ],
        &[],
        MIN_THROTTLE,
    );

    assert_eq!(
        target,
        Some(DrivingTarget::StolenHomeFlag(Vec2::new(260.0, 0.0)))
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
        MIN_THROTTLE,
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
        MIN_THROTTLE,
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
        MIN_THROTTLE,
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
        MIN_THROTTLE,
    );

    assert_eq!(
        target,
        Some(DrivingTarget::StolenHomeFlag(Vec2::new(100.0, 0.0)))
    );
}

#[test]
fn teammate_clears_contested_home_base_for_flag_carrier() {
    let carrier = Entity::from_raw(8);
    let target = choose_capture_the_flag_target(
        Entity::from_raw(7),
        AiTeam::Red,
        &[
            FlagTarget {
                team: AiTeam::Blue,
                home: Vec2::new(-500.0, 0.0),
                position: Vec2::new(300.0, 0.0),
                holder: Some(carrier),
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
            position: Vec2::new(430.0, 0.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    assert_eq!(
        target,
        Some(DrivingTarget::UrgentDefendHomeBase(Vec2::new(430.0, 0.0)))
    );
}

#[test]
fn teammate_blocks_flag_carrier_pursuer_before_escorting() {
    let carrier = Entity::from_raw(8);
    let target = choose_capture_the_flag_target(
        Entity::from_raw(7),
        AiTeam::Red,
        &[
            FlagTarget {
                team: AiTeam::Blue,
                home: Vec2::new(-500.0, 0.0),
                position: Vec2::new(100.0, 0.0),
                holder: Some(carrier),
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
            position: Vec2::new(-40.0, 0.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    assert_eq!(
        target,
        Some(DrivingTarget::BlockFlagCarrierPursuer(Vec2::new(
            -40.0, 0.0
        )))
    );
}

#[test]
fn teammate_leads_a_moving_flag_carrier_pursuer() {
    let carrier = Entity::from_raw(8);
    let pursuer_position = Vec2::new(200.0, 0.0);
    let pursuer_velocity = Vec2::new(-200.0, 50.0);
    let target = choose_capture_the_flag_target(
        Entity::from_raw(7),
        AiTeam::Red,
        &[
            FlagTarget {
                team: AiTeam::Blue,
                home: Vec2::new(-500.0, 0.0),
                position: Vec2::ZERO,
                holder: Some(carrier),
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
            position: pursuer_position,
            velocity: pursuer_velocity,
        }],
        MIN_THROTTLE,
    );

    let Some(DrivingTarget::BlockFlagCarrierPursuer(block)) = target else {
        panic!("expected a block-pursuer target, got {target:?}");
    };
    // The carrier sits at the origin, so the block must interpose on the
    // pursuer's line of approach at ram range, not body-block the spot it has
    // already left.
    assert!(
        block.distance(pursuer_position) > EPSILON,
        "should lead the moving pursuer, not block its vacated spot: {block}"
    );
    assert!(
        (block.length() - crate::gameplay::combat::RAM_RADIUS).abs() <= EPSILON,
        "block should sit on the carrier's ram-range ring: {block}"
    );
    assert!(
        (block - pursuer_position).dot(pursuer_velocity) > 0.0,
        "block should lead ahead along the pursuer's heading: {block}"
    );
    assert!(
        block.y > 0.0,
        "block should shift onto the side the pursuer is heading: {block}"
    );
}

#[test]
fn block_pursuer_meets_a_close_pursuer_head_on() {
    // Inside the standoff ring the pursuer is already at the carrier, so it is
    // met head-on at its current spot rather than led.
    let block = block_pursuer_intercept_point(
        Vec2::ZERO,
        ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(100.0, 0.0),
            velocity: Vec2::new(-50.0, 0.0),
        },
    );
    assert_vec2_near(block, Vec2::new(100.0, 0.0));
}

#[test]
fn block_pursuer_stands_off_at_the_ring_for_a_stationary_pursuer() {
    // A stationary (or velocity-less, e.g. the human) pursuer outside the ring
    // never breaches it, so the blocker interposes on the ring between the
    // carrier and the pursuer rather than charging the pursuer's spot.
    let block = block_pursuer_intercept_point(
        Vec2::ZERO,
        ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(200.0, 0.0),
            velocity: Vec2::ZERO,
        },
    );
    assert_vec2_near(block, Vec2::new(FLAG_CARRIER_PURSUER_BLOCK_STANDOFF, 0.0));
}

#[test]
fn block_pursuer_holds_the_ring_for_a_pursuer_veering_away() {
    // A pursuer driving away from the carrier never breaches the ring, so the
    // lead falls back to the static interpose on the ring.
    let block = block_pursuer_intercept_point(
        Vec2::ZERO,
        ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(200.0, 0.0),
            velocity: Vec2::new(50.0, 0.0),
        },
    );
    assert_vec2_near(block, Vec2::new(FLAG_CARRIER_PURSUER_BLOCK_STANDOFF, 0.0));
}

#[test]
fn block_pursuer_leads_a_crossing_pursuer_onto_its_approach_line() {
    // A pursuer sweeping in from the north toward the east is led onto the side
    // it is heading for, on the ring, ahead of the spot it has already left.
    let position = Vec2::new(0.0, 200.0);
    let velocity = Vec2::new(60.0, -200.0);
    let block = block_pursuer_intercept_point(
        Vec2::ZERO,
        ThreatTarget {
            team: AiTeam::Blue,
            position,
            velocity,
        },
    );
    assert!(
        (block.length() - FLAG_CARRIER_PURSUER_BLOCK_STANDOFF).abs() <= EPSILON,
        "led point should sit on the carrier's block ring: {block}"
    );
    assert!(
        block.distance(position) > EPSILON,
        "led point should differ from the vacated spot: {block}"
    );
    assert!(
        (block - position).dot(velocity) > 0.0,
        "led point should sit ahead along the pursuer's heading: {block}"
    );
    assert!(
        block.x > 0.0,
        "led point should shift onto the side the pursuer is heading: {block}"
    );
}

#[test]
fn teammate_escorts_flag_carrier_when_pursuer_is_distant() {
    let carrier = Entity::from_raw(8);
    let target = choose_capture_the_flag_target(
        Entity::from_raw(7),
        AiTeam::Red,
        &[
            FlagTarget {
                team: AiTeam::Blue,
                home: Vec2::new(-500.0, 0.0),
                position: Vec2::new(100.0, 0.0),
                holder: Some(carrier),
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
            position: Vec2::new(-200.0, 0.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    let Some(DrivingTarget::EscortFlagCarrier(position)) = target else {
        panic!("expected escort target, got {target:?}");
    };
    assert_vec2_near(position, Vec2::new(220.0, 0.0));
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
        MIN_THROTTLE,
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
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    assert_eq!(
        target,
        Some(DrivingTarget::UrgentDefendHomeBase(Vec2::new(360.0, 0.0)))
    );
}

#[test]
fn close_home_flag_threat_triggers_urgent_defence() {
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
            position: Vec2::new(360.0, 0.0),
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    assert_eq!(
        target,
        Some(DrivingTarget::UrgentDefendHomeBase(Vec2::new(360.0, 0.0)))
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
                velocity: Vec2::ZERO,
            },
            ThreatTarget {
                team: AiTeam::Blue,
                position: Vec2::new(500.0, 90.0),
                velocity: Vec2::ZERO,
            },
        ],
        MIN_THROTTLE,
    );

    assert_eq!(
        target,
        Some(DrivingTarget::UrgentDefendHomeBase(Vec2::new(500.0, 90.0)))
    );
}

#[test]
fn ring_breach_time_solves_a_head_on_approach() {
    // A thief 300 out, driving straight at the flag at 200 u/s, breaches the
    // 140 ring after travelling 160 units: t = 160 / 200 = 0.8.
    let time = ring_breach_time(Vec2::new(300.0, 0.0), Vec2::new(-200.0, 0.0), 140.0);
    let time = time.expect("a head-on thief must breach the ring");
    assert!((time - 0.8).abs() <= EPSILON, "expected t=0.8, got {time}");
}

#[test]
fn ring_breach_time_ignores_a_stationary_thief() {
    assert_eq!(
        ring_breach_time(Vec2::new(300.0, 0.0), Vec2::ZERO, 140.0),
        None,
        "a parked thief never breaches the ring"
    );
}

#[test]
fn ring_breach_time_ignores_a_thief_veering_away() {
    assert_eq!(
        ring_breach_time(Vec2::new(300.0, 0.0), Vec2::new(200.0, 0.0), 140.0),
        None,
        "a thief driving away from the flag never breaches the ring"
    );
}

#[test]
fn ring_breach_time_ignores_a_thief_that_misses_the_ring() {
    // Sweeping sideways 300 units above the flag, the thief's path never comes
    // within the 140 ring, so there is no breach to lead.
    assert_eq!(
        ring_breach_time(Vec2::new(0.0, 300.0), Vec2::new(-200.0, 0.0), 140.0),
        None,
        "a thief whose line never reaches the ring is no breach"
    );
}

#[test]
fn defensive_intercept_meets_a_head_on_thief_at_the_plain_body_block() {
    // A thief running straight at the flag crosses the ring on its current
    // bearing, so the lead must coincide with the plain body-block: leading only
    // matters for an angled approach, never a head-on run.
    let point = defensive_intercept_point(
        Vec2::ZERO,
        ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(300.0, 0.0),
            velocity: Vec2::new(-200.0, 0.0),
        },
    );
    assert_vec2_near(point, Vec2::new(HOME_FLAG_DEFENSE_DISTANCE, 0.0));
}

#[test]
fn defensive_intercept_leads_a_sweeping_thief_onto_its_own_lane() {
    // The thief sweeps straight across at y = 60, outside the ring. A plain
    // body-block would meet it down on its current bearing (y ~= 27); leading
    // meets it where it will actually cross the ring, out on its own y = 60 lane.
    let point = defensive_intercept_point(
        Vec2::ZERO,
        ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(300.0, 60.0),
            velocity: Vec2::new(-200.0, 0.0),
        },
    );
    let expected_x = HOME_FLAG_DEFENSE_DISTANCE
        .mul_add(HOME_FLAG_DEFENSE_DISTANCE, -(60.0 * 60.0))
        .sqrt();
    assert_vec2_near(point, Vec2::new(expected_x, 60.0));
}

#[test]
fn defensive_intercept_falls_back_to_the_body_block_for_a_stationary_thief() {
    // With no velocity to lead, the meet-point is the plain body-block on the
    // thief's current bearing, exactly as before the lead existed.
    let point = defensive_intercept_point(
        Vec2::ZERO,
        ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(300.0, 60.0),
            velocity: Vec2::ZERO,
        },
    );
    let expected = Vec2::new(300.0, 60.0).normalize() * HOME_FLAG_DEFENSE_DISTANCE;
    assert_vec2_near(point, expected);
}

#[test]
fn defensive_intercept_depends_on_heading_not_speed() {
    // The breach point is fixed by the thief's line of approach, not how fast it
    // travels it: a crawling and a flying thief on the same heading are met at the
    // same ring crossing, so the rough top-speed velocity estimate pins it exactly.
    let crawling = defensive_intercept_point(
        Vec2::ZERO,
        ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(300.0, 60.0),
            velocity: Vec2::new(-50.0, 0.0),
        },
    );
    let flying = defensive_intercept_point(
        Vec2::ZERO,
        ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(300.0, 60.0),
            velocity: Vec2::new(-400.0, 0.0),
        },
    );
    assert_vec2_near(crawling, flying);
}

#[test]
fn defensive_intercept_meets_a_thief_inside_the_ring_head_on() {
    // A thief already inside the defensive ring is met at its current spot, so
    // the defender closes head-on rather than backing out onto the ring.
    let point = defensive_intercept_point(
        Vec2::ZERO,
        ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(100.0, 0.0),
            velocity: Vec2::new(-200.0, 0.0),
        },
    );
    assert_vec2_near(point, Vec2::new(100.0, 0.0));
}

#[test]
fn home_defender_leads_a_sweeping_thief_to_the_ring_crossing() {
    // End to end through the brain: a Red home defender facing a Blue thief
    // sweeping across its flag lane is sent to where the thief will breach the
    // defensive ring, not the spot it has already left.
    let target = choose_capture_the_flag_target(
        Entity::from_raw(7),
        AiTeam::Red,
        &[
            FlagTarget {
                team: AiTeam::Blue,
                home: Vec2::new(-1000.0, 0.0),
                position: Vec2::new(-1000.0, 0.0),
                holder: None,
            },
            FlagTarget {
                team: AiTeam::Red,
                home: Vec2::new(0.0, 0.0),
                position: Vec2::new(0.0, 0.0),
                holder: None,
            },
        ],
        &[ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(300.0, 60.0),
            velocity: Vec2::new(-200.0, 0.0),
        }],
        MIN_THROTTLE,
    );

    let Some(DrivingTarget::DefendHomeBase(point)) = target else {
        panic!("expected a led DefendHomeBase, got {target:?}");
    };
    let expected_x = HOME_FLAG_DEFENSE_DISTANCE
        .mul_add(HOME_FLAG_DEFENSE_DISTANCE, -(60.0 * 60.0))
        .sqrt();
    assert_vec2_near(point, Vec2::new(expected_x, 60.0));
}

#[test]
fn closest_enemy_threat_within_picks_nearest_and_ignores_allies_and_range() {
    let threats = [
        ThreatTarget {
            team: AiTeam::Red,
            position: Vec2::new(40.0, 0.0),
            velocity: Vec2::ZERO,
        },
        ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(60.0, 0.0),
            velocity: Vec2::ZERO,
        },
        ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(10.0, 0.0),
            velocity: Vec2::ZERO,
        },
        ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(5000.0, 0.0),
            velocity: Vec2::ZERO,
        },
    ];

    let nearest = closest_enemy_threat_within(AiTeam::Red, Vec2::ZERO, 200.0, &threats);

    assert_eq!(
        nearest,
        Some(ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(10.0, 0.0),
            velocity: Vec2::ZERO,
        })
    );
}

#[test]
fn closest_enemy_threat_within_breaks_ties_by_position() {
    let threats = [
        ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(0.0, 50.0),
            velocity: Vec2::ZERO,
        },
        ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(0.0, -50.0),
            velocity: Vec2::ZERO,
        },
        ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(-50.0, 0.0),
            velocity: Vec2::ZERO,
        },
    ];

    let nearest = closest_enemy_threat_within(AiTeam::Red, Vec2::ZERO, 200.0, &threats);

    assert_eq!(
        nearest,
        Some(ThreatTarget {
            team: AiTeam::Blue,
            position: Vec2::new(-50.0, 0.0),
            velocity: Vec2::ZERO,
        })
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
            velocity: Vec2::ZERO,
        }],
        MIN_THROTTLE,
    );

    assert_eq!(
        target,
        Some(DrivingTarget::EnemyFlag(Vec2::new(-450.0, 20.0)))
    );
}

fn pit_candidate(entity: u32, position: Vec2, home: Vec2) -> PitRetreatCandidate {
    PitRetreatCandidate {
        entity: Entity::from_raw(entity),
        position,
        home,
        carries_enemy_flag: false,
    }
}

#[test]
fn pit_retreat_sends_no_one_when_the_team_is_healthy() {
    let home = Vec2::new(500.0, 0.0);
    let candidates = [
        pit_candidate(1, Vec2::new(450.0, 0.0), home),
        pit_candidate(2, Vec2::new(-200.0, 0.0), home),
    ];

    assert_eq!(pit_retreat_car(0.5, false, false, &candidates), None);
}

#[test]
fn pit_retreat_sends_the_home_most_car_when_battered() {
    let home = Vec2::new(500.0, 0.0);
    let candidates = [
        pit_candidate(1, Vec2::new(-200.0, 0.0), home),
        pit_candidate(2, Vec2::new(450.0, 0.0), home),
    ];

    assert_eq!(
        pit_retreat_car(PIT_RETREAT_INTEGRITY_FRACTION, false, false, &candidates),
        Some(Entity::from_raw(2))
    );
}

#[test]
fn pit_retreat_triggers_exactly_at_the_threshold() {
    let home = Vec2::new(500.0, 0.0);
    let candidates = [
        pit_candidate(1, Vec2::new(450.0, 0.0), home),
        pit_candidate(2, Vec2::new(-200.0, 0.0), home),
    ];

    assert_eq!(
        pit_retreat_car(PIT_RETREAT_INTEGRITY_FRACTION, false, false, &candidates),
        Some(Entity::from_raw(1))
    );
    let just_above = PIT_RETREAT_INTEGRITY_FRACTION + 0.001;
    assert_eq!(pit_retreat_car(just_above, false, false, &candidates), None);
}

#[test]
fn pit_retreat_never_pulls_a_flag_carrier() {
    let home = Vec2::new(500.0, 0.0);
    let carrier = PitRetreatCandidate {
        entity: Entity::from_raw(1),
        position: Vec2::new(480.0, 0.0),
        home,
        carries_enemy_flag: true,
    };
    let defender = pit_candidate(2, Vec2::new(-100.0, 0.0), home);

    // The carrier is closer to home, but it keeps hauling: the non-carrier
    // is the one sent to the pit.
    assert_eq!(
        pit_retreat_car(0.1, false, false, &[carrier, defender]),
        Some(Entity::from_raw(2))
    );
}

#[test]
fn pit_retreat_keeps_the_last_car_on_duty() {
    let home = Vec2::new(500.0, 0.0);
    let lone = [pit_candidate(1, Vec2::new(480.0, 0.0), home)];

    assert_eq!(pit_retreat_car(0.05, false, false, &lone), None);
}

#[test]
fn pit_retreat_returns_none_when_every_car_carries_a_flag() {
    let home = Vec2::new(500.0, 0.0);
    let carriers = [
        PitRetreatCandidate {
            entity: Entity::from_raw(1),
            position: Vec2::new(480.0, 0.0),
            home,
            carries_enemy_flag: true,
        },
        PitRetreatCandidate {
            entity: Entity::from_raw(2),
            position: Vec2::new(-100.0, 0.0),
            home,
            carries_enemy_flag: true,
        },
    ];

    assert_eq!(pit_retreat_car(0.05, false, false, &carriers), None);
}

#[test]
fn pit_retreat_breaks_distance_ties_deterministically() {
    let home = Vec2::ZERO;
    // Both cars sit the same distance from home; the lower `x` then `y`
    // wins, matching `compare_positions`.
    let candidates = [
        pit_candidate(1, Vec2::new(100.0, 0.0), home),
        pit_candidate(2, Vec2::new(-100.0, 0.0), home),
    ];

    assert_eq!(
        pit_retreat_car(0.2, false, false, &candidates),
        Some(Entity::from_raw(2))
    );
}

/// A battered team trailing on captures in the closing stretch has no time to
/// heal: every car must stay on the equalising push, so the clutch attack
/// cancels the pit stop even though the integrity gate would otherwise fire.
#[test]
fn pit_retreat_holds_off_when_behind_in_closing_time() {
    let home = Vec2::new(500.0, 0.0);
    let candidates = [
        pit_candidate(1, Vec2::new(450.0, 0.0), home),
        pit_candidate(2, Vec2::new(-200.0, 0.0), home),
    ];

    assert_eq!(pit_retreat_car(0.05, true, true, &candidates), None);
}

/// The suppression is the trailing team's clutch play alone: a team that is not
/// behind (level or ahead) in the closing stretch is not racing an equaliser, so
/// a battered car still limps home to recover.
#[test]
fn pit_retreat_still_pits_a_team_not_behind_in_closing_time() {
    let home = Vec2::new(500.0, 0.0);
    let candidates = [
        pit_candidate(1, Vec2::new(450.0, 0.0), home),
        pit_candidate(2, Vec2::new(-200.0, 0.0), home),
    ];

    assert_eq!(
        pit_retreat_car(0.05, false, true, &candidates),
        Some(Entity::from_raw(1))
    );
}

/// The suppression is closing-time only: a team trailing earlier in the round
/// has time for a car to heal and rejoin, so it still pit-retreats normally.
#[test]
fn pit_retreat_still_pits_a_team_behind_outside_closing_time() {
    let home = Vec2::new(500.0, 0.0);
    let candidates = [
        pit_candidate(1, Vec2::new(450.0, 0.0), home),
        pit_candidate(2, Vec2::new(-200.0, 0.0), home),
    ];

    assert_eq!(
        pit_retreat_car(0.05, true, false, &candidates),
        Some(Entity::from_raw(1))
    );
}

fn lead_defence_candidate(entity: u32, position: Vec2, home: Vec2) -> LeadDefenceCandidate {
    LeadDefenceCandidate {
        entity: Entity::from_raw(entity),
        position,
        home,
        carries_enemy_flag: false,
    }
}

#[test]
fn lead_defence_recalls_no_one_when_not_protecting_a_lead() {
    let home = Vec2::new(500.0, 0.0);
    let candidates = [
        lead_defence_candidate(1, Vec2::new(450.0, 0.0), home),
        lead_defence_candidate(2, Vec2::new(-200.0, 0.0), home),
    ];

    assert_eq!(lead_defence_car(false, &candidates), None);
}

#[test]
fn lead_defence_recalls_the_home_most_car_when_protecting_a_lead() {
    let home = Vec2::new(500.0, 0.0);
    let candidates = [
        lead_defence_candidate(1, Vec2::new(-200.0, 0.0), home),
        lead_defence_candidate(2, Vec2::new(450.0, 0.0), home),
    ];

    assert_eq!(
        lead_defence_car(true, &candidates),
        Some(Entity::from_raw(2))
    );
}

#[test]
fn lead_defence_never_pulls_a_flag_carrier() {
    let home = Vec2::new(500.0, 0.0);
    let carrier = LeadDefenceCandidate {
        entity: Entity::from_raw(1),
        position: Vec2::new(480.0, 0.0),
        home,
        carries_enemy_flag: true,
    };
    let defender = lead_defence_candidate(2, Vec2::new(-100.0, 0.0), home);

    // The carrier sits closer to home, but it keeps hauling toward a sealing
    // capture: the non-carrier is the one recalled to guard.
    assert_eq!(
        lead_defence_car(true, &[carrier, defender]),
        Some(Entity::from_raw(2))
    );
}

#[test]
fn lead_defence_keeps_the_last_car_on_duty() {
    let home = Vec2::new(500.0, 0.0);
    let lone = [lead_defence_candidate(1, Vec2::new(480.0, 0.0), home)];

    assert_eq!(lead_defence_car(true, &lone), None);
}

#[test]
fn lead_defence_returns_none_when_every_car_carries_a_flag() {
    let home = Vec2::new(500.0, 0.0);
    let carriers = [
        LeadDefenceCandidate {
            entity: Entity::from_raw(1),
            position: Vec2::new(480.0, 0.0),
            home,
            carries_enemy_flag: true,
        },
        LeadDefenceCandidate {
            entity: Entity::from_raw(2),
            position: Vec2::new(-100.0, 0.0),
            home,
            carries_enemy_flag: true,
        },
    ];

    assert_eq!(lead_defence_car(true, &carriers), None);
}

#[test]
fn lead_defence_breaks_distance_ties_deterministically() {
    let home = Vec2::ZERO;
    // Both cars sit the same distance from home; the lower `x` then `y`
    // wins, matching `compare_positions`.
    let candidates = [
        lead_defence_candidate(1, Vec2::new(100.0, 0.0), home),
        lead_defence_candidate(2, Vec2::new(-100.0, 0.0), home),
    ];

    assert_eq!(
        lead_defence_car(true, &candidates),
        Some(Entity::from_raw(2))
    );
}

fn finish_off_candidate(entity: u32, position: Vec2) -> FinishOffCandidate {
    FinishOffCandidate {
        entity: Entity::from_raw(entity),
        position,
        carries_enemy_flag: false,
    }
}

#[test]
fn finish_off_presses_no_one_when_the_enemy_is_healthy() {
    let candidates = [
        finish_off_candidate(1, Vec2::new(100.0, 0.0)),
        finish_off_candidate(2, Vec2::new(-300.0, 0.0)),
    ];
    let enemies = [Vec2::new(500.0, 0.0)];

    // Enemy above the reeling band: nothing to finish off yet.
    assert_eq!(
        finish_off_car(1.0, 0.6, false, &candidates, &enemies, None, false),
        None
    );
}

#[test]
fn finish_off_presses_no_one_when_the_enemy_is_already_wrecked() {
    let candidates = [
        finish_off_candidate(1, Vec2::new(100.0, 0.0)),
        finish_off_candidate(2, Vec2::new(-300.0, 0.0)),
    ];
    let enemies = [Vec2::new(500.0, 0.0)];

    // A wreck already paid out; a stunned enemy has no pool left to grind.
    assert_eq!(
        finish_off_car(1.0, 0.0, false, &candidates, &enemies, None, false),
        None
    );
}

#[test]
fn finish_off_presses_no_one_when_we_are_not_healthier() {
    let candidates = [
        finish_off_candidate(1, Vec2::new(100.0, 0.0)),
        finish_off_candidate(2, Vec2::new(-300.0, 0.0)),
    ];
    let enemies = [Vec2::new(500.0, 0.0)];

    // Both reeling and level: a team that is not behind recovers instead of
    // trading into a mutual wreck.
    assert_eq!(
        finish_off_car(0.2, 0.2, false, &candidates, &enemies, None, false),
        None
    );
    // We are the more battered: pressing would be suicidal, behind or not.
    assert_eq!(
        finish_off_car(0.1, 0.25, false, &candidates, &enemies, None, false),
        None
    );
}

#[test]
fn finish_off_sends_the_car_nearest_a_kill() {
    let candidates = [
        finish_off_candidate(1, Vec2::new(-300.0, 0.0)),
        finish_off_candidate(2, Vec2::new(380.0, 0.0)),
    ];
    let enemies = [Vec2::new(500.0, 0.0)];

    // Car 2 is closest to the prey, so it breaks off to finish the kill.
    assert_eq!(
        finish_off_car(0.8, 0.2, false, &candidates, &enemies, None, false),
        Some((Entity::from_raw(2), Vec2::new(500.0, 0.0)))
    );
}

#[test]
fn finish_off_aims_at_the_enemy_car_nearest_the_hunter() {
    let candidates = [
        finish_off_candidate(1, Vec2::new(0.0, 0.0)),
        finish_off_candidate(2, Vec2::new(900.0, 0.0)),
    ];
    let enemies = [Vec2::new(200.0, 0.0), Vec2::new(-50.0, 0.0)];

    // The closest hunter (car 1) targets the nearer of the two enemy cars.
    assert_eq!(
        finish_off_car(0.9, 0.15, false, &candidates, &enemies, None, false),
        Some((Entity::from_raw(1), Vec2::new(-50.0, 0.0)))
    );
}

#[test]
fn finish_off_triggers_at_the_threshold() {
    let candidates = [
        finish_off_candidate(1, Vec2::new(100.0, 0.0)),
        finish_off_candidate(2, Vec2::new(-300.0, 0.0)),
    ];
    let enemies = [Vec2::new(500.0, 0.0)];

    assert!(finish_off_car(
        0.9,
        FINISH_OFF_ENEMY_INTEGRITY_FRACTION,
        false,
        &candidates,
        &enemies,
        None,
        false,
    )
    .is_some());
    let just_above = FINISH_OFF_ENEMY_INTEGRITY_FRACTION + 0.001;
    assert_eq!(
        finish_off_car(0.9, just_above, false, &candidates, &enemies, None, false),
        None
    );
}

#[test]
fn finish_off_never_pulls_a_flag_carrier() {
    let carrier = FinishOffCandidate {
        entity: Entity::from_raw(1),
        position: Vec2::new(480.0, 0.0),
        carries_enemy_flag: true,
    };
    let hunter = finish_off_candidate(2, Vec2::new(100.0, 0.0));
    let enemies = [Vec2::new(500.0, 0.0)];

    // The carrier is nearest the prey, but it keeps hauling: the non-carrier
    // is the one sent to finish the kill.
    assert_eq!(
        finish_off_car(0.8, 0.2, false, &[carrier, hunter], &enemies, None, false),
        Some((Entity::from_raw(2), Vec2::new(500.0, 0.0)))
    );
}

#[test]
fn finish_off_keeps_the_last_car_on_duty() {
    let lone = [finish_off_candidate(1, Vec2::new(100.0, 0.0))];
    let enemies = [Vec2::new(500.0, 0.0)];

    assert_eq!(
        finish_off_car(0.8, 0.2, false, &lone, &enemies, None, false),
        None
    );
}

#[test]
fn finish_off_returns_none_with_no_enemy_to_hunt() {
    let candidates = [
        finish_off_candidate(1, Vec2::new(100.0, 0.0)),
        finish_off_candidate(2, Vec2::new(-300.0, 0.0)),
    ];

    assert_eq!(
        finish_off_car(0.8, 0.2, false, &candidates, &[], None, false),
        None
    );
}

#[test]
fn finish_off_returns_none_when_every_car_carries_a_flag() {
    let carriers = [
        FinishOffCandidate {
            entity: Entity::from_raw(1),
            position: Vec2::new(480.0, 0.0),
            carries_enemy_flag: true,
        },
        FinishOffCandidate {
            entity: Entity::from_raw(2),
            position: Vec2::new(-100.0, 0.0),
            carries_enemy_flag: true,
        },
    ];
    let enemies = [Vec2::new(500.0, 0.0)];

    assert_eq!(
        finish_off_car(0.8, 0.2, false, &carriers, &enemies, None, false),
        None
    );
}

#[test]
fn finish_off_breaks_hunter_ties_deterministically() {
    // Both cars sit the same distance from the lone enemy; the lower `x`
    // then `y` wins, matching `compare_positions`.
    let candidates = [
        finish_off_candidate(1, Vec2::new(100.0, 0.0)),
        finish_off_candidate(2, Vec2::new(-100.0, 0.0)),
    ];
    let enemies = [Vec2::ZERO];

    assert_eq!(
        finish_off_car(0.8, 0.2, false, &candidates, &enemies, None, false),
        Some((Entity::from_raw(2), Vec2::ZERO))
    );
}

#[test]
fn finish_off_presses_an_even_match_when_behind_on_captures() {
    let candidates = [
        finish_off_candidate(1, Vec2::new(380.0, 0.0)),
        finish_off_candidate(2, Vec2::new(-300.0, 0.0)),
    ];
    let enemies = [Vec2::new(500.0, 0.0)];

    // Level on durability and both reeling: a team that is not behind holds
    // station, but a team chasing the leader takes the even-health gamble to
    // wreck the car it is paid extra to take down.
    assert_eq!(
        finish_off_car(0.2, 0.2, false, &candidates, &enemies, None, false),
        None
    );
    assert_eq!(
        finish_off_car(0.2, 0.2, true, &candidates, &enemies, None, false),
        Some((Entity::from_raw(1), Vec2::new(500.0, 0.0))),
        "a trailing team should hunt the reeling leader at even health"
    );
}

#[test]
fn finish_off_never_over_commits_when_more_battered_even_if_behind() {
    let candidates = [
        finish_off_candidate(1, Vec2::new(100.0, 0.0)),
        finish_off_candidate(2, Vec2::new(-300.0, 0.0)),
    ];
    let enemies = [Vec2::new(500.0, 0.0)];

    // Behind on captures but the more battered side: the comeback relaxation
    // never tips into a suicidal chase, so the team still recovers instead.
    assert_eq!(
        finish_off_car(0.15, 0.25, true, &candidates, &enemies, None, false),
        None
    );
}

#[test]
fn finish_off_still_needs_a_reeling_enemy_when_behind() {
    let candidates = [
        finish_off_candidate(1, Vec2::new(100.0, 0.0)),
        finish_off_candidate(2, Vec2::new(-300.0, 0.0)),
    ];
    let enemies = [Vec2::new(500.0, 0.0)];

    // Being behind relaxes only the health margin, never the requirement that
    // the enemy be reeling: a healthy or already-wrecked leader is no target.
    assert_eq!(
        finish_off_car(0.2, 0.6, true, &candidates, &enemies, None, false),
        None
    );
    assert_eq!(
        finish_off_car(0.2, 0.0, true, &candidates, &enemies, None, false),
        None
    );
}

#[test]
fn finish_off_clutch_presses_a_worn_leader_in_closing_time_when_behind() {
    let candidates = [
        finish_off_candidate(1, Vec2::new(380.0, 0.0)),
        finish_off_candidate(2, Vec2::new(-300.0, 0.0)),
    ];
    let enemies = [Vec2::new(500.0, 0.0)];
    // An enemy worn past the clutch ceiling but above the normal reeling gate:
    // outside the closing-time window it is no target, but a trailing team
    // running out of clock presses it for the clutch wreck that can win the
    // decider.
    let worn = f32::midpoint(
        FINISH_OFF_ENEMY_INTEGRITY_FRACTION,
        CLUTCH_FINISH_OFF_ENEMY_INTEGRITY_FRACTION,
    );

    assert_eq!(
        finish_off_car(0.8, worn, true, &candidates, &enemies, None, false),
        None,
        "outside closing time the normal reeling gate still holds"
    );
    assert_eq!(
        finish_off_car(0.8, worn, true, &candidates, &enemies, None, true),
        Some((Entity::from_raw(1), Vec2::new(500.0, 0.0))),
        "a trailing team should chase the clutch wreck in closing time"
    );
}

#[test]
fn finish_off_clutch_window_only_opens_for_the_trailing_team() {
    let candidates = [
        finish_off_candidate(1, Vec2::new(380.0, 0.0)),
        finish_off_candidate(2, Vec2::new(-300.0, 0.0)),
    ];
    let enemies = [Vec2::new(500.0, 0.0)];
    let worn = f32::midpoint(
        FINISH_OFF_ENEMY_INTEGRITY_FRACTION,
        CLUTCH_FINISH_OFF_ENEMY_INTEGRITY_FRACTION,
    );

    // A level or leading team keeps the strict reeling gate even in closing
    // time: the clutch gamble is the comeback lever, not a leader's tool.
    assert_eq!(
        finish_off_car(0.8, worn, false, &candidates, &enemies, None, true),
        None
    );
}

#[test]
fn finish_off_clutch_still_spares_a_near_pristine_enemy() {
    let candidates = [
        finish_off_candidate(1, Vec2::new(100.0, 0.0)),
        finish_off_candidate(2, Vec2::new(-300.0, 0.0)),
    ];
    let enemies = [Vec2::new(500.0, 0.0)];

    // The clutch window widens the gate but never opens it on a fresh enemy:
    // even a last-ditch press needs a foe genuinely on the back foot.
    assert!(finish_off_car(
        0.9,
        CLUTCH_FINISH_OFF_ENEMY_INTEGRITY_FRACTION,
        true,
        &candidates,
        &enemies,
        None,
        true,
    )
    .is_some());
    let just_above = CLUTCH_FINISH_OFF_ENEMY_INTEGRITY_FRACTION + 0.001;
    assert_eq!(
        finish_off_car(0.9, just_above, true, &candidates, &enemies, None, true),
        None
    );
}

#[test]
fn finish_off_clutch_never_drops_the_health_guard() {
    let candidates = [
        finish_off_candidate(1, Vec2::new(100.0, 0.0)),
        finish_off_candidate(2, Vec2::new(-300.0, 0.0)),
    ];
    let enemies = [Vec2::new(500.0, 0.0)];
    let worn = f32::midpoint(
        FINISH_OFF_ENEMY_INTEGRITY_FRACTION,
        CLUTCH_FINISH_OFF_ENEMY_INTEGRITY_FRACTION,
    );

    // Behind and in closing time, but the more battered side: the clutch
    // window widens only the reeling gate, never the "at least as healthy"
    // guard, so the desperation never tips into a suicidal trade.
    assert_eq!(
        finish_off_car(worn - 0.05, worn, true, &candidates, &enemies, None, true),
        None
    );
}

#[test]
fn finish_off_hunts_a_reeling_enemy_carrier_over_a_nearer_foe() {
    // An empty-handed foe sits right on top of car 1, while our stolen flag is
    // hauled away far to the left. Cutting the carrier down denies the capture,
    // forces the turnover, and banks the carrier-takedown bounty, so the hunter
    // chases the carrier rather than the nearer, less valuable kill.
    let candidates = [
        finish_off_candidate(1, Vec2::new(0.0, 0.0)),
        finish_off_candidate(2, Vec2::new(600.0, 0.0)),
    ];
    let nearer_foe = Vec2::new(40.0, 0.0);
    let carrier = Vec2::new(-500.0, 0.0);
    let enemies = [nearer_foe, carrier];

    assert_eq!(
        finish_off_car(0.8, 0.2, false, &candidates, &enemies, Some(carrier), false),
        Some((Entity::from_raw(1), carrier)),
        "a reeling carrier is the most valuable kill, even when a nearer foe beckons"
    );
}

#[test]
fn finish_off_sends_the_hunter_nearest_the_carrier() {
    // Car 1 is closest to a stray enemy on the right; car 2 is closest to the
    // flag thief fleeing left. Without our flag in flight the nearer-kill rule
    // would pick car 1; with it stolen the carrier is the prize, so the
    // carrier-side hunter (car 2) is committed instead.
    let candidates = [
        finish_off_candidate(1, Vec2::new(450.0, 0.0)),
        finish_off_candidate(2, Vec2::new(-400.0, 0.0)),
    ];
    let stray = Vec2::new(500.0, 0.0);
    let carrier = Vec2::new(-500.0, 0.0);
    let enemies = [stray, carrier];

    assert_eq!(
        finish_off_car(0.8, 0.2, false, &candidates, &enemies, None, false),
        Some((Entity::from_raw(1), stray)),
        "with no flag stolen the nearer kill still wins"
    );
    assert_eq!(
        finish_off_car(0.8, 0.2, false, &candidates, &enemies, Some(carrier), false),
        Some((Entity::from_raw(2), carrier)),
        "a stolen flag redirects the hunt to the thief"
    );
}

#[test]
fn finish_off_carrier_hunt_still_spares_our_own_carrier() {
    // The car nearest the thief is itself running the enemy flag home, so it
    // keeps its capture run; the next-nearest non-carrier gives chase instead.
    let our_carrier = FinishOffCandidate {
        entity: Entity::from_raw(1),
        position: Vec2::new(-480.0, 0.0),
        carries_enemy_flag: true,
    };
    let hunter = finish_off_candidate(2, Vec2::new(-200.0, 0.0));
    let carrier = Vec2::new(-500.0, 0.0);
    let enemies = [Vec2::new(300.0, 0.0), carrier];

    assert_eq!(
        finish_off_car(
            0.8,
            0.2,
            false,
            &[our_carrier, hunter],
            &enemies,
            Some(carrier),
            false,
        ),
        Some((Entity::from_raw(2), carrier))
    );
}

#[test]
fn finish_off_carrier_hunt_breaks_ties_deterministically() {
    // Two hunters equidistant from the thief: the lower `x` then `y` wins,
    // matching `compare_positions`.
    let candidates = [
        finish_off_candidate(1, Vec2::new(100.0, 0.0)),
        finish_off_candidate(2, Vec2::new(-100.0, 0.0)),
    ];
    let carrier = Vec2::ZERO;
    let enemies = [carrier];

    assert_eq!(
        finish_off_car(0.8, 0.2, false, &candidates, &enemies, Some(carrier), false),
        Some((Entity::from_raw(2), Vec2::ZERO))
    );
}

#[test]
fn finish_off_ignores_a_carrier_when_the_enemy_is_not_reeling() {
    // The reeling and health guards run before the carrier branch: a healthy
    // enemy team is no target even while it hauls our flag.
    let candidates = [
        finish_off_candidate(1, Vec2::new(0.0, 0.0)),
        finish_off_candidate(2, Vec2::new(-300.0, 0.0)),
    ];
    let carrier = Vec2::new(-500.0, 0.0);
    let enemies = [carrier];

    assert_eq!(
        finish_off_car(0.8, 0.6, false, &candidates, &enemies, Some(carrier), false),
        None
    );
}

#[test]
fn finish_off_carrier_hunt_keeps_the_last_car_on_duty() {
    // Even with our flag stolen, a lone car never abandons the field just to
    // chase: the stolen-flag defensive role already covers the solo case.
    let lone = [finish_off_candidate(1, Vec2::new(0.0, 0.0))];
    let carrier = Vec2::new(-500.0, 0.0);
    let enemies = [carrier];

    assert_eq!(
        finish_off_car(0.8, 0.2, false, &lone, &enemies, Some(carrier), false),
        None
    );
}

fn finish_off_carrier_candidate(entity: u32, position: Vec2) -> FinishOffCandidate {
    FinishOffCandidate {
        entity: Entity::from_raw(entity),
        position,
        carries_enemy_flag: true,
    }
}

#[test]
fn pincer_partner_sends_the_next_nearest_spare_car() {
    // Car 1 is already hunting the prey; of the two spare cars the nearer one
    // (car 2) piles in to spring the gang-up, leaving car 3 on the objective.
    let prey = Vec2::new(500.0, 0.0);
    let candidates = [
        finish_off_candidate(1, Vec2::new(400.0, 0.0)),
        finish_off_candidate(2, Vec2::new(300.0, 0.0)),
        finish_off_candidate(3, Vec2::new(-300.0, 0.0)),
    ];

    assert_eq!(
        pincer_partner(Entity::from_raw(1), prey, &candidates),
        Some(Entity::from_raw(2))
    );
}

#[test]
fn pincer_partner_never_re_sends_the_primary_hunter() {
    // The primary sits right on the prey, so it would be the nearest car of
    // all, yet it is already committed: the partner must be a different car.
    let prey = Vec2::new(500.0, 0.0);
    let candidates = [
        finish_off_candidate(1, Vec2::new(500.0, 0.0)),
        finish_off_candidate(2, Vec2::new(490.0, 0.0)),
        finish_off_candidate(3, Vec2::new(-300.0, 0.0)),
    ];

    assert_eq!(
        pincer_partner(Entity::from_raw(1), prey, &candidates),
        Some(Entity::from_raw(2))
    );
}

#[test]
fn pincer_partner_spares_the_objective_with_too_few_cars() {
    // Only the primary plus one spare: pulling the spare would empty the
    // objective, so no pincer springs and the lone hunter presses alone.
    let prey = Vec2::new(500.0, 0.0);
    let candidates = [
        finish_off_candidate(1, Vec2::new(400.0, 0.0)),
        finish_off_candidate(2, Vec2::new(300.0, 0.0)),
    ];

    assert_eq!(pincer_partner(Entity::from_raw(1), prey, &candidates), None);
}

#[test]
fn pincer_partner_never_pulls_a_flag_carrier() {
    // The nearest spare car is hauling the enemy flag home, so it is skipped:
    // the partner is the next eligible non-carrier instead.
    let prey = Vec2::new(500.0, 0.0);
    let candidates = [
        finish_off_candidate(1, Vec2::new(400.0, 0.0)),
        finish_off_carrier_candidate(2, Vec2::new(450.0, 0.0)),
        finish_off_candidate(3, Vec2::new(-300.0, 0.0)),
    ];

    assert_eq!(
        pincer_partner(Entity::from_raw(1), prey, &candidates),
        Some(Entity::from_raw(3))
    );
}

#[test]
fn pincer_partner_returns_none_when_only_the_primary_can_join() {
    // Three cars, but the other two are both carriers: no spare non-carrier is
    // free to gang up, so the kill press stays a lone hunt.
    let prey = Vec2::new(500.0, 0.0);
    let candidates = [
        finish_off_candidate(1, Vec2::new(400.0, 0.0)),
        finish_off_carrier_candidate(2, Vec2::new(450.0, 0.0)),
        finish_off_carrier_candidate(3, Vec2::new(460.0, 0.0)),
    ];

    assert_eq!(pincer_partner(Entity::from_raw(1), prey, &candidates), None);
}

#[test]
fn pincer_partner_breaks_ties_deterministically() {
    // Two spare cars equidistant from the prey settle the pick by the shared
    // x-then-y tie-break, so the partner choice is stable frame to frame.
    let prey = Vec2::ZERO;
    let candidates = [
        finish_off_candidate(1, Vec2::new(1000.0, 0.0)),
        finish_off_candidate(2, Vec2::new(0.0, 50.0)),
        finish_off_candidate(3, Vec2::new(0.0, -50.0)),
    ];

    assert_eq!(
        pincer_partner(Entity::from_raw(1), prey, &candidates),
        Some(Entity::from_raw(3))
    );
}

fn flank_shield_candidate(entity: u32, position: Vec2) -> FlankShieldCandidate {
    FlankShieldCandidate {
        entity: Entity::from_raw(entity),
        position,
        carries_enemy_flag: false,
    }
}

fn flank_shield_carrier(entity: u32, position: Vec2) -> FlankShieldCandidate {
    FlankShieldCandidate {
        entity: Entity::from_raw(entity),
        position,
        carries_enemy_flag: true,
    }
}

/// A stationary enemy (blue) pursuer of a red team's flag carrier, so a test's
/// block point is the static interpose on the ring rather than a lead.
fn blue_pursuer(position: Vec2) -> ThreatTarget {
    ThreatTarget {
        team: AiTeam::Blue,
        position,
        velocity: Vec2::ZERO,
    }
}

#[test]
fn carrier_flank_shield_sends_a_second_car_to_the_second_pursuer() {
    // The carrier sits at the origin with two blue chasers: the nearer one
    // (east) is the primary block's job, so a second free car peels off to
    // interpose on the second chaser (north) on the carrier's ram-range ring.
    let carrier = Vec2::ZERO;
    let second = blue_pursuer(Vec2::new(0.0, 220.0));
    let threats = [blue_pursuer(Vec2::new(200.0, 0.0)), second];
    let candidates = [
        flank_shield_carrier(1, carrier),
        flank_shield_candidate(2, Vec2::new(300.0, 0.0)),
        flank_shield_candidate(3, Vec2::new(0.0, 300.0)),
    ];

    assert_eq!(
        carrier_flank_shield(
            carrier,
            Vec2::new(0.0, 1000.0),
            AiTeam::Red,
            &threats,
            &candidates
        ),
        Some((
            Entity::from_raw(3),
            block_pursuer_intercept_point(carrier, second)
        ))
    );
}

#[test]
fn carrier_flank_shield_needs_a_second_pursuer() {
    // A lone chaser is already covered by the primary block, so no flank shield
    // springs.
    let carrier = Vec2::ZERO;
    let threats = [blue_pursuer(Vec2::new(200.0, 0.0))];
    let candidates = [
        flank_shield_carrier(1, carrier),
        flank_shield_candidate(2, Vec2::new(300.0, 0.0)),
        flank_shield_candidate(3, Vec2::new(0.0, 300.0)),
    ];

    assert_eq!(
        carrier_flank_shield(
            carrier,
            Vec2::new(0.0, 1000.0),
            AiTeam::Red,
            &threats,
            &candidates
        ),
        None
    );
}

#[test]
fn carrier_flank_shield_spares_the_carriers_primary_protection_with_too_few_cars() {
    // Only the carrier plus one free car: peeling that car would strip the
    // carrier's primary block, so no flank shield springs even with two chasers.
    let carrier = Vec2::ZERO;
    let threats = [
        blue_pursuer(Vec2::new(200.0, 0.0)),
        blue_pursuer(Vec2::new(0.0, 220.0)),
    ];
    let candidates = [
        flank_shield_carrier(1, carrier),
        flank_shield_candidate(2, Vec2::new(300.0, 0.0)),
    ];

    assert_eq!(
        carrier_flank_shield(
            carrier,
            Vec2::new(0.0, 1000.0),
            AiTeam::Red,
            &threats,
            &candidates
        ),
        None
    );
}

#[test]
fn carrier_flank_shield_never_pulls_the_flag_carrier() {
    // The carrier sits dead on the ring, nearest both intercepts, yet it keeps
    // hauling the flag home: a non-carrier is the one peeled off to shield it.
    let carrier = Vec2::ZERO;
    let threats = [
        blue_pursuer(Vec2::new(200.0, 0.0)),
        blue_pursuer(Vec2::new(0.0, 220.0)),
    ];
    let candidates = [
        flank_shield_carrier(1, carrier),
        flank_shield_candidate(2, Vec2::new(300.0, 0.0)),
        flank_shield_candidate(3, Vec2::new(0.0, 300.0)),
    ];

    let (entity, _) = carrier_flank_shield(
        carrier,
        Vec2::new(0.0, 1000.0),
        AiTeam::Red,
        &threats,
        &candidates,
    )
    .expect("a flank shield");
    assert_ne!(
        entity,
        Entity::from_raw(1),
        "the carrier is never pulled off its run"
    );
}

#[test]
fn carrier_flank_shield_ignores_a_distant_second_pursuer() {
    // Only one chaser is within pursuer range; the far one is no threat to the
    // run yet, so no flank shield springs.
    let carrier = Vec2::ZERO;
    let threats = [
        blue_pursuer(Vec2::new(200.0, 0.0)),
        blue_pursuer(Vec2::new(0.0, FLAG_CARRIER_PURSUER_RADIUS + 50.0)),
    ];
    let candidates = [
        flank_shield_carrier(1, carrier),
        flank_shield_candidate(2, Vec2::new(300.0, 0.0)),
        flank_shield_candidate(3, Vec2::new(0.0, 300.0)),
    ];

    assert_eq!(
        carrier_flank_shield(
            carrier,
            Vec2::new(0.0, 1000.0),
            AiTeam::Red,
            &threats,
            &candidates
        ),
        None
    );
}

#[test]
fn carrier_flank_shield_excludes_the_primary_blocker() {
    // Both chasers close from the east, so one car is nearest to both
    // intercepts; it takes the primary block, and the flank goes to the next
    // car, never the same one twice.
    let carrier = Vec2::ZERO;
    let threats = [
        blue_pursuer(Vec2::new(200.0, 0.0)),
        blue_pursuer(Vec2::new(210.0, 20.0)),
    ];
    let candidates = [
        flank_shield_carrier(1, carrier),
        flank_shield_candidate(2, Vec2::new(300.0, 0.0)),
        flank_shield_candidate(3, Vec2::new(330.0, 0.0)),
    ];

    let (entity, _) = carrier_flank_shield(
        carrier,
        Vec2::new(0.0, 1000.0),
        AiTeam::Red,
        &threats,
        &candidates,
    )
    .expect("a flank shield");
    assert_eq!(
        entity,
        Entity::from_raw(3),
        "the nearest car blocks the closest chaser; the flank is the next car"
    );
}

#[test]
fn carrier_flank_shield_breaks_ties_deterministically() {
    // Two free cars sit equidistant from the flank intercept; the shared
    // x-then-y tie-break settles the pick so it never wavers frame to frame.
    let carrier = Vec2::ZERO;
    let threats = [
        blue_pursuer(Vec2::new(200.0, 0.0)),
        blue_pursuer(Vec2::new(0.0, 220.0)),
    ];
    let flank_point = block_pursuer_intercept_point(carrier, threats[1]);
    let candidates = [
        flank_shield_carrier(1, carrier),
        flank_shield_candidate(2, Vec2::new(500.0, 0.0)),
        flank_shield_candidate(3, flank_point + Vec2::new(40.0, 0.0)),
        flank_shield_candidate(4, flank_point + Vec2::new(-40.0, 0.0)),
    ];

    let (entity, _) = carrier_flank_shield(
        carrier,
        Vec2::new(0.0, 1000.0),
        AiTeam::Red,
        &threats,
        &candidates,
    )
    .expect("a flank shield");
    assert_eq!(
        entity,
        Entity::from_raw(4),
        "equidistant flank candidates settle on the lower x, matching compare_positions"
    );
}

#[test]
fn carrier_flank_shield_stands_down_while_the_home_base_is_contested() {
    // Two chasers hound the carrier, but an enemy is also sitting on the team's
    // own base: defending the steal there outranks shielding the run, so the
    // urgent home defence keeps the car rather than this overlay pulling it.
    let carrier = Vec2::ZERO;
    let home = Vec2::new(0.0, 1000.0);
    let threats = [
        blue_pursuer(Vec2::new(200.0, 0.0)),
        blue_pursuer(Vec2::new(0.0, 220.0)),
        blue_pursuer(home + Vec2::new(0.0, HOME_BASE_CONTEST_RADIUS - 10.0)),
    ];
    let candidates = [
        flank_shield_carrier(1, carrier),
        flank_shield_candidate(2, Vec2::new(300.0, 0.0)),
        flank_shield_candidate(3, Vec2::new(0.0, 300.0)),
    ];

    assert_eq!(
        carrier_flank_shield(carrier, home, AiTeam::Red, &threats, &candidates),
        None
    );
}

#[test]
fn wall_crush_aim_drives_a_wall_pinned_prey_into_the_boundary() {
    let half = Vec2::new(1000.0, 600.0);
    // A prey hugging the +x side wall, well inside the crush band and clear of
    // the y walls: the hunter aims past it into the wall to spring the crush.
    let prey = Vec2::new(960.0, 0.0);
    let aim = finish_off_wall_crush_aim(prey, half);

    assert!(
        aim.x > prey.x,
        "the aim must sit beyond the prey toward the wall so the charge shoves it in: {aim:?}"
    );
    // The pinned x is shoved past the wall; the unpinned y is left on the prey.
    assert_eq!(
        aim,
        Vec2::new(half.x + FINISH_OFF_WALL_CRUSH_OVERSHOOT, prey.y)
    );
}

#[test]
fn wall_crush_aim_handles_the_negative_side_wall() {
    let half = Vec2::new(1000.0, 600.0);
    // Mirror of the +x case: a prey pinned to the -x wall is shoved the other
    // way, the aim sitting beyond the negative boundary.
    let prey = Vec2::new(-960.0, 0.0);
    let aim = finish_off_wall_crush_aim(prey, half);

    assert!(
        aim.x < prey.x,
        "a prey on the negative wall is shoved the other way: {aim:?}"
    );
    assert_eq!(
        aim,
        Vec2::new(-(half.x + FINISH_OFF_WALL_CRUSH_OVERSHOOT), prey.y)
    );
}

#[test]
fn wall_crush_aim_wedges_a_cornered_prey_into_both_walls() {
    let half = Vec2::new(1000.0, 600.0);
    // A prey trapped in the bottom-left corner sits inside both crush bands, so
    // the aim points diagonally past it into the corner, springing the corner
    // crush on top of the wall crush.
    let prey = Vec2::new(-950.0, -550.0);
    let aim = finish_off_wall_crush_aim(prey, half);

    assert_eq!(
        aim,
        Vec2::new(
            -(half.x + FINISH_OFF_WALL_CRUSH_OVERSHOOT),
            -(half.y + FINISH_OFF_WALL_CRUSH_OVERSHOOT)
        )
    );
}

#[test]
fn wall_crush_aim_leaves_an_open_field_prey_untouched() {
    let half = Vec2::new(1000.0, 600.0);
    // A prey out in the open has no wall to pin it against, so the hunter just
    // drives at it as before, no aim shift.
    let prey = Vec2::new(100.0, -50.0);

    assert_eq!(finish_off_wall_crush_aim(prey, half), prey);
}

#[test]
fn wall_crush_aim_matches_the_crush_band_edge() {
    let half = Vec2::new(1000.0, 600.0);
    let margin = crate::gameplay::combat::WALL_CRUSH_MARGIN;
    // Exactly on the band edge the prey is pinned (the same `>=` boundary the
    // combat crush uses), so the aim shifts to the wall.
    let on_edge = Vec2::new(half.x - margin, 0.0);
    assert_eq!(
        finish_off_wall_crush_aim(on_edge, half),
        Vec2::new(half.x + FINISH_OFF_WALL_CRUSH_OVERSHOOT, on_edge.y)
    );
    // A whisker outside the band the prey is open-field, the aim left on it, so
    // the AI never shoves a foe that the combat layer would not crush.
    let just_outside = Vec2::new(half.x - margin - 1.0, 0.0);
    assert_eq!(finish_off_wall_crush_aim(just_outside, half), just_outside);
}

#[test]
fn lead_aim_holds_on_a_stationary_prey() {
    // A prey that is not moving has nowhere to be led to, so the hunter just
    // drives at it: pure pursuit, no aim shift.
    let aim = finish_off_lead_aim(Vec2::ZERO, Vec2::new(100.0, 0.0), Vec2::ZERO, 500.0);
    assert_vec2_near(aim, Vec2::new(100.0, 0.0));
}

#[test]
fn lead_aim_cuts_off_a_crossing_prey() {
    // A reeling prey crosses the hunter's sight line to the right; the hunter
    // aims ahead of it, at the point on its path both reach at once, rather than
    // at the spot it has already left. With a hunter twice the prey's speed the
    // geometry resolves to a 30-60-90 lead: 100*sqrt(3) to the side.
    let aim = finish_off_lead_aim(
        Vec2::ZERO,
        Vec2::new(0.0, 300.0),
        Vec2::new(250.0, 0.0),
        500.0,
    );
    assert_vec2_near(aim, Vec2::new(3.0_f32.sqrt() * 100.0, 300.0));
    assert!(
        aim.x > 0.0,
        "the hunter must lead a rightward-crossing prey to its right: {aim}"
    );
}

#[test]
fn lead_aim_extends_ahead_of_a_slower_fleeing_prey() {
    // A reeling prey flees straight away at half the hunter's speed; the cut-off
    // sits further down its escape line than the prey itself, so the hunter
    // charges through to run it down rather than trailing the spot it left.
    let aim = finish_off_lead_aim(
        Vec2::ZERO,
        Vec2::new(0.0, 300.0),
        Vec2::new(0.0, 250.0),
        500.0,
    );
    assert_vec2_near(aim, Vec2::new(0.0, 600.0));
}

#[test]
fn lead_aim_never_shortens_against_a_closing_prey() {
    // A prey charging straight back onto the hunter would yield an interception
    // point nearer than the prey, which would stall the charge short of contact.
    // The extend-only rule keeps the aim on the prey so the hunter drives through.
    let prey = Vec2::new(0.0, 300.0);
    let aim = finish_off_lead_aim(Vec2::ZERO, prey, Vec2::new(0.0, -250.0), 500.0);
    assert_vec2_near(aim, prey);
}

#[test]
fn lead_aim_tail_chases_a_prey_it_cannot_catch() {
    // A prey fleeing dead away at the hunter's own speed can never be intercepted,
    // so the lead falls back to a plain tail chase at the prey's current spot.
    let prey = Vec2::new(0.0, 300.0);
    let aim = finish_off_lead_aim(Vec2::ZERO, prey, Vec2::new(0.0, 500.0), 500.0);
    assert_vec2_near(aim, prey);
}

#[test]
fn finish_off_aim_crushes_a_wall_pinned_prey_regardless_of_its_flight() {
    // A prey pinned against a wall has nowhere to run, so the aim ignores its
    // velocity and shoves it into the boundary exactly as the wall crush would.
    let half = Vec2::new(1000.0, 600.0);
    let prey = Vec2::new(half.x, 0.0);
    let aim = finish_off_aim(
        Vec2::new(half.x, -400.0),
        prey,
        Vec2::new(0.0, 500.0),
        500.0,
        half,
    );
    assert_eq!(aim, finish_off_wall_crush_aim(prey, half));
}

#[test]
fn finish_off_aim_leads_a_prey_loose_in_the_open() {
    // Out in the open there is no wall to crush against, so the aim heads the
    // prey off with the lead interception instead.
    let half = Vec2::new(1000.0, 600.0);
    let hunter = Vec2::new(0.0, -100.0);
    let prey = Vec2::new(100.0, -50.0);
    let velocity = Vec2::new(250.0, 0.0);
    assert_eq!(
        finish_off_aim(hunter, prey, velocity, 500.0, half),
        finish_off_lead_aim(hunter, prey, velocity, 500.0)
    );
}
