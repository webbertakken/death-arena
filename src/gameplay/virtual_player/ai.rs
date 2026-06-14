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

/// Distance at which a home-flag threat is too close to ignore for pickups.
pub const URGENT_HOME_FLAG_THREAT_RADIUS: f32 = 220.0;

/// Maximum sideways distance from a CTF push where a pickup still counts as
/// being on the flag lane.
pub const CTF_PICKUP_LANE_WIDTH: f32 = 60.0;

/// Wider detour lane for high-value pickups that are worth a short gamble.
pub const CTF_HIGH_VALUE_PICKUP_LANE_WIDTH: f32 = 100.0;

/// Distance from home at which a flag carrier stops gambling on pickup detours
/// and commits to finishing the capture.
pub const FLAG_CARRIER_CAPTURE_COMMIT_DISTANCE: f32 = 180.0;

/// Distance around home base where an enemy blocks a carried-flag capture.
pub const HOME_BASE_CONTEST_RADIUS: f32 = 160.0;

/// Distance around a friendly flag carrier where enemies count as pursuers.
pub const FLAG_CARRIER_PURSUER_RADIUS: f32 = 260.0;

/// Distance from home where a flag carrier waits while the base is contested.
pub const CONTESTED_HOME_BASE_STAGING_DISTANCE: f32 = 240.0;

/// Minimum pickup priority that justifies interrupting a CTF objective.
pub const CTF_PICKUP_DETOUR_MIN_PRIORITY: u32 = 50;

/// Priority at which a pickup justifies the wider CTF detour lane.
pub const CTF_WIDE_DETOUR_MIN_PRIORITY: u32 = 150;

/// Team durability fraction (`0.0`..=`1.0`) at or below which a battered team
/// breaks one car off the field and sends it home to pit-recover.
///
/// Sits in the same "actively battered" band the integrity-scaled repair and
/// shield pickup tiers already react to (both treat `<= 0.35` as hard-pressed),
/// so a team ground this low patches up at its own base even when no repair
/// pickup is on its lane, the reliable recovery the home-base pit opened up.
pub const PIT_RETREAT_INTEGRITY_FRACTION: f32 = 0.30;

/// Enemy team durability fraction (`0.0`..=`1.0`) at or below which a *healthier*
/// team presses one car off the objective to hunt the reeling enemy down.
///
/// The offensive mirror of [`PIT_RETREAT_INTEGRITY_FRACTION`]: the integrity
/// system grinds a team down, and once an enemy is this battered (yet not already
/// wrecked, so there is still a pool to grind to zero) a team that is itself in
/// better shape sends its keenest car to finish the kill, banking the wreck
/// bounty, the spin-out, the surge, and any flag turnover. Set to the same
/// "actively battered" band the retreat and the integrity-scaled repair/shield
/// tiers already react to, so the press begins exactly when the enemy is on the
/// ropes. The "we must be healthier" guard keeps a team that is itself reeling
/// from over-committing into a mutual wreck instead of recovering, though a team
/// *trailing on captures* relaxes it to an even-health gamble (see
/// [`finish_off_car`]) to hunt the leader it is paid extra to wreck.
pub const FINISH_OFF_ENEMY_INTEGRITY_FRACTION: f32 = 0.30;

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
    ContestedHomeBaseStaging(Vec2),
    DefendHomeBase(Vec2),
    HomeBase(Vec2),
    BlockFlagCarrierPursuer(Vec2),
    EnemyFlag(Vec2),
    EnemyFlagFlank(Vec2),
    EscortFlagCarrier(Vec2),
    FinishWreck(Vec2),
    MidfieldInterceptor(Vec2),
    PatrolWaypoint(Vec2),
    Pickup(Vec2),
    Player(Vec2),
    StolenHomeFlag(Vec2),
    StolenHomeFlagRouteGuard(Vec2),
    UrgentDefendHomeBase(Vec2),
}

impl DrivingTarget {
    #[must_use]
    pub const fn position(self) -> Vec2 {
        match self {
            Self::ContestedHomeBaseStaging(position)
            | Self::DefendHomeBase(position)
            | Self::HomeBase(position)
            | Self::BlockFlagCarrierPursuer(position)
            | Self::EnemyFlag(position)
            | Self::EnemyFlagFlank(position)
            | Self::EscortFlagCarrier(position)
            | Self::FinishWreck(position)
            | Self::MidfieldInterceptor(position)
            | Self::PatrolWaypoint(position)
            | Self::Pickup(position)
            | Self::Player(position)
            | Self::StolenHomeFlag(position)
            | Self::StolenHomeFlagRouteGuard(position)
            | Self::UrgentDefendHomeBase(position) => position,
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
    /// Closing-time clutch play: when set, a car on a CTF objective only breaks
    /// off for a pickup genuinely worth a wide detour (nitro or a battered team's
    /// survival grab), never a mere cash bag. Holds for every team racing the
    /// clock, whether it is committing to attack (not ahead) or protecting a lead
    /// (ahead), so neither side sightsees for cash. See
    /// [`closing_time_detour_min_priority`].
    pub closing_time_discipline: bool,
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

    if enemy_flag.holder == Some(ai_entity) {
        if own_flag_is_dropped(own_flag) {
            return Some(DrivingTarget::StolenHomeFlag(own_flag.position));
        }
        if own_flag.holder.is_some() {
            return Some(DrivingTarget::StolenHomeFlag(stolen_flag_intercept_point(
                own_flag.position,
                enemy_flag.home,
            )));
        }
        if let Some(threat) = closest_home_base_contester(team, own_flag.home, threats) {
            return Some(DrivingTarget::ContestedHomeBaseStaging(
                contested_home_base_staging_point(own_flag.home, threat.position),
            ));
        }
        return Some(DrivingTarget::HomeBase(own_flag.home));
    }

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

    if enemy_flag.holder.is_some() {
        if let Some(threat) = closest_home_base_contester(team, own_flag.home, threats) {
            return Some(DrivingTarget::UrgentDefendHomeBase(threat.position));
        }
        if let Some(threat) = closest_flag_carrier_pursuer(team, enemy_flag.position, threats) {
            return Some(DrivingTarget::BlockFlagCarrierPursuer(threat.position));
        }
        return Some(DrivingTarget::EscortFlagCarrier(escort_lead_point(
            enemy_flag.position,
            own_flag.home,
        )));
    }

    if let Some(threat) = closest_home_flag_threat(team, own_flag, threats) {
        let target = defensive_intercept_point(own_flag.position, threat.position);
        if threat.position.distance_squared(own_flag.position)
            <= URGENT_HOME_FLAG_THREAT_RADIUS * URGENT_HOME_FLAG_THREAT_RADIUS
        {
            return Some(DrivingTarget::UrgentDefendHomeBase(target));
        }
        return Some(DrivingTarget::DefendHomeBase(target));
    }

    enemy_flag
        .holder
        .is_none()
        .then_some(DrivingTarget::EnemyFlag(enemy_flag.position))
}

fn own_flag_is_dropped(own_flag: &FlagTarget) -> bool {
    own_flag.holder.is_none() && own_flag.position.distance_squared(own_flag.home) > f32::EPSILON
}

/// A virtual player a battered team could send home to pit-recover.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PitRetreatCandidate {
    pub entity: Entity,
    pub position: Vec2,
    pub home: Vec2,
    pub carries_enemy_flag: bool,
}

/// Picks the single car a battered team breaks off to its home-base pit, or
/// `None` when no retreat is warranted.
///
/// Closes the loop the home-base pit recovery opened: rather than only healing
/// passively when an objective happens to bring a car home, a team ground to
/// [`PIT_RETREAT_INTEGRITY_FRACTION`] or below actively sends its home-most car
/// to the pit to patch up while the rest keep playing. Stateless and
/// deterministic: the car nearest its own base is chosen each frame (it is the
/// cheapest to pull and, once it commits homeward, stays nearest, so the choice
/// is stable), with `x` then `y` as the tie-breaker, mirroring the fallback-role
/// coordination.
///
/// Two guards keep the retreat from backfiring:
/// - a flag carrier is never pulled off its capture run (it already heals at
///   home as it scores), so only non-carriers are eligible;
/// - at least one car must stay on duty, so a lone car never abandons the field
///   just to heal.
#[must_use]
pub fn pit_retreat_car(
    integrity_fraction: f32,
    candidates: &[PitRetreatCandidate],
) -> Option<Entity> {
    if integrity_fraction > PIT_RETREAT_INTEGRITY_FRACTION {
        return None;
    }
    if candidates.len() < 2 {
        return None;
    }
    candidates
        .iter()
        .filter(|candidate| !candidate.carries_enemy_flag)
        .min_by(|a, b| {
            a.position
                .distance_squared(a.home)
                .partial_cmp(&b.position.distance_squared(b.home))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| compare_positions(a.position, b.position))
        })
        .map(|candidate| candidate.entity)
}

/// A virtual player a leading team could recall to guard a closing-time lead.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeadDefenceCandidate {
    pub entity: Entity,
    pub position: Vec2,
    pub home: Vec2,
    pub carries_enemy_flag: bool,
}

/// Picks the single car a team protecting a closing-time lead recalls to guard
/// its own base, or `None` when no lead defence is warranted.
///
/// The defensive mirror of the trailing team's closing-time objective
/// commitment: where a side that is *not* ahead races the flag in the final
/// stretch, a side that *is* ahead stops over-extending and stations its
/// home-most non-carrier on the defensive lane, so the equalising capture has to
/// get through a dug-in defender. The classic "protect the lead, run down the
/// clock" play, and the completion of the closing-time arc.
///
/// Stateless and deterministic, mirroring [`pit_retreat_car`]: the car nearest
/// its own base is chosen (cheapest to recall and, once it commits homeward,
/// stays nearest, so the pick is stable), with `x` then `y` as the tie-break.
///
/// Two guards keep the recall from backfiring:
/// - a flag carrier is never pulled off its run, since a capture would seal the
///   match outright, the strongest lead protection of all;
/// - at least one car must stay free, so a lone car never abandons the field
///   just to camp its own base.
#[must_use]
pub fn lead_defence_car(
    protecting_lead: bool,
    candidates: &[LeadDefenceCandidate],
) -> Option<Entity> {
    if !protecting_lead {
        return None;
    }
    if candidates.len() < 2 {
        return None;
    }
    candidates
        .iter()
        .filter(|candidate| !candidate.carries_enemy_flag)
        .min_by(|a, b| {
            a.position
                .distance_squared(a.home)
                .partial_cmp(&b.position.distance_squared(b.home))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| compare_positions(a.position, b.position))
        })
        .map(|candidate| candidate.entity)
}

/// A virtual player a healthier team could send to hunt down a reeling enemy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FinishOffCandidate {
    pub entity: Entity,
    pub position: Vec2,
    pub carries_enemy_flag: bool,
}

/// Picks the single car a team breaks off to finish a reeling enemy, paired with
/// the enemy car it should hunt, or `None` when no kill press is warranted.
///
/// The offensive mirror of [`pit_retreat_car`]: rather than only wearing the
/// enemy down and hoping cars happen to collide, a team that is *winning the
/// attrition* sends its keenest car to grind a battered enemy's integrity to
/// zero, the classic Death Rally "they are on the ropes, go for the kill". The
/// chosen hunter is the non-carrier closest to any live enemy car (nearest to a
/// kill, and once committed it stays nearest, so the pick is stable), with the
/// shared `x`-then-`y` [`compare_positions`] tie-break; it is aimed at whichever
/// enemy car is nearest to *it*.
///
/// When `enemy_flag_carrier` is set, a reeling enemy is hauling this team's flag
/// away, the single most valuable wreck on the board: cutting it down denies the
/// capture, forces the turnover, and banks the
/// [`crate::gameplay::combat::carrier_takedown_wreck_bonus`]. The hunt then
/// redirects to the thief, the keenest non-carrier nearest *it* is sent (same
/// `x`-then-`y` tie-break), so a kill press on a reeling team that has just
/// stolen the flag chases the runner home instead of the merely-nearest foe.
///
/// Guards keep the press from backfiring:
/// - the enemy must be reeling but not already wrecked: above zero yet at or
///   below [`FINISH_OFF_ENEMY_INTEGRITY_FRACTION`]. A wreck already paid out and
///   a stunned enemy has no pool left to grind;
/// - the hunting team must be healthy enough relative to its prey. Normally it
///   must be the *healthier* of the two, so a team that is itself battered
///   recovers (it pit-retreats) instead of trading into a mutual wreck. A team
///   `behind_on_captures`, however, relaxes this to an *even-health* gamble: the
///   leader has a price on its head
///   ([`crate::gameplay::combat::most_wanted_wreck_bonus`] pays the trailing
///   team extra for wrecking it), so taking the even trade to bank the
///   bounty, the flag turnover and slow the leader's snowball is worth it. The
///   team is never the *more battered* side regardless, so the relaxation never
///   tips into a suicidal chase;
/// - a flag carrier is never pulled off its capture run, and at least one car
///   must stay on the objective, so a lone car never abandons the field for a kill.
#[must_use]
pub fn finish_off_car(
    own_integrity_fraction: f32,
    enemy_integrity_fraction: f32,
    behind_on_captures: bool,
    candidates: &[FinishOffCandidate],
    enemy_positions: &[Vec2],
    enemy_flag_carrier: Option<Vec2>,
) -> Option<(Entity, Vec2)> {
    if enemy_integrity_fraction <= 0.0
        || enemy_integrity_fraction > FINISH_OFF_ENEMY_INTEGRITY_FRACTION
    {
        return None;
    }
    // A trailing team takes the even-health gamble to hunt the reeling leader; a
    // level or leading team holds out for a clear durability edge. Neither ever
    // presses while it is the more battered side.
    let healthy_enough = if behind_on_captures {
        own_integrity_fraction >= enemy_integrity_fraction
    } else {
        own_integrity_fraction > enemy_integrity_fraction
    };
    if !healthy_enough {
        return None;
    }
    if candidates.len() < 2 || enemy_positions.is_empty() {
        return None;
    }

    // A reeling enemy hauling our flag is the most valuable kill on the board:
    // redirect the hunt to the thief, sending the keenest non-carrier nearest it.
    if let Some(carrier) = enemy_flag_carrier {
        return candidates
            .iter()
            .filter(|candidate| !candidate.carries_enemy_flag)
            .min_by(|a, b| {
                a.position
                    .distance_squared(carrier)
                    .partial_cmp(&b.position.distance_squared(carrier))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| compare_positions(a.position, b.position))
            })
            .map(|candidate| (candidate.entity, carrier));
    }

    candidates
        .iter()
        .filter(|candidate| !candidate.carries_enemy_flag)
        .filter_map(|candidate| {
            let prey = nearest_position(candidate.position, enemy_positions)?;
            Some((candidate, prey, candidate.position.distance_squared(prey)))
        })
        .min_by(|(a, _, a_dist), (b, _, b_dist)| {
            a_dist
                .partial_cmp(b_dist)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| compare_positions(a.position, b.position))
        })
        .map(|(candidate, prey, _)| (candidate.entity, prey))
}

/// Cars a healthy team piles onto a single kill once it springs the pincer: the
/// primary [`finish_off_car`] hunter plus one [`pincer_partner`].
///
/// Death Rally's classic "swarm the weakened car". Where [`finish_off_car`] sends
/// one hunter at a reeling enemy, a team with cars to spare sends a second to gang
/// up, and the combat layer pays it off: two cars trading paint with one victim at
/// once spring the [`crate::gameplay::combat::PINCER_RAM_DAMAGE_PER_FRAME`] gang-up
/// that grinds a surrounded car down faster than a lone ram can. The kill press
/// lands harder and quicker, banking the wreck (and any carrier-takedown turnover)
/// before the victim can limp to a repair.
pub const FINISH_OFF_PINCER_HUNTERS: usize = 2;

/// The two hunters must genuinely meet the gang-up threshold, enforced at compile
/// time, so a pincer kill actually springs the bonus it is sent to land.
const _: () = assert!(FINISH_OFF_PINCER_HUNTERS >= crate::gameplay::combat::PINCER_MIN_ATTACKERS);

/// Smallest team car count that still spares one car for the objective while two
/// hunt: the [`FINISH_OFF_PINCER_HUNTERS`] plus the lone car left on duty.
///
/// The pincer's "never abandon the field" guard, the same principle
/// [`finish_off_car`] keeps for a lone hunter (it never pulls a team's last car),
/// raised by one because the pincer pulls a second.
pub const PINCER_MIN_TEAM_CARS: usize = FINISH_OFF_PINCER_HUNTERS + 1;

/// A pincer must leave more cars behind than a lone hunt, enforced at compile time,
/// so committing the second car never empties the objective.
const _: () = assert!(PINCER_MIN_TEAM_CARS > FINISH_OFF_PINCER_HUNTERS);

/// Picks a second car to pile onto the kill the primary [`finish_off_car`] hunter
/// is making, springing the pincer, or `None` when no car can join without
/// abandoning the objective.
///
/// Given the `primary_hunter` already committed to `prey` and the team's full car
/// roster, this sends the next-keenest spare car, the non-carrier nearest the prey
/// after the primary, to gang up on the same victim. Two cars hemming one foe in at
/// once spring the [`crate::gameplay::combat::PINCER_RAM_DAMAGE_PER_FRAME`] gang-up,
/// grinding it down faster than the lone hunter could.
///
/// Stateless and deterministic, mirroring [`finish_off_car`]: the eligible car
/// nearest the prey is chosen with the shared `x`-then-`y` [`compare_positions`]
/// tie-break, so the partner pick never wavers frame to frame.
///
/// Guards keep the gang-up from backfiring:
/// - the primary hunter is never re-sent as its own partner;
/// - a flag carrier is never pulled off its capture run, matching the lone hunt;
/// - the team must field at least [`PINCER_MIN_TEAM_CARS`] cars, so committing two
///   to the kill always leaves at least one on the objective, the same "never
///   abandon the field" principle [`finish_off_car`] keeps for a single hunter.
#[must_use]
pub fn pincer_partner(
    primary_hunter: Entity,
    prey: Vec2,
    candidates: &[FinishOffCandidate],
) -> Option<Entity> {
    if candidates.len() < PINCER_MIN_TEAM_CARS {
        return None;
    }

    candidates
        .iter()
        .filter(|candidate| candidate.entity != primary_hunter && !candidate.carries_enemy_flag)
        .min_by(|a, b| {
            a.position
                .distance_squared(prey)
                .partial_cmp(&b.position.distance_squared(prey))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| compare_positions(a.position, b.position))
        })
        .map(|candidate| candidate.entity)
}

/// Distance beyond the arena wall the [`finish_off_wall_crush_aim`] target sits,
/// so a hunter pressing a wall-pinned prey charges firmly *through* it into the
/// boundary rather than merely up to it.
///
/// A steering target outside the arena is only an aim, never a destination (cars
/// clamp to the bounds), so the overshoot just biases the charge into the wall.
/// One car-length-and-change past the boundary is plenty to keep the hunter's
/// nose on the wall however close to it the prey already sits, even glued to it.
pub const FINISH_OFF_WALL_CRUSH_OVERSHOOT: f32 = 200.0;

/// The overshoot must actually carry the aim past the wall, enforced at compile
/// time, so the charge always points into the boundary.
const _: () = assert!(FINISH_OFF_WALL_CRUSH_OVERSHOOT > 0.0);

/// Aims a [`finish_off_car`] hunter so it shoves a reeling prey *into* the arena
/// wall (or corner) it is pinned against, springing the combat wall crush.
///
/// Death Rally's classic "ram them into the wall to finish them". Where the kill
/// press otherwise drives at the prey's exact spot and hopes the geometry lands a
/// crush, this deliberately sets one up, the offensive mirror of how
/// [`pincer_partner`] deliberately springs the gang-up. On each axis the prey sits
/// within [`crate::gameplay::combat::WALL_CRUSH_MARGIN`] of (the same band the
/// combat [`crate::gameplay::combat::wall_crush_ram_damage`] reads), the aim is
/// pushed past that wall by [`FINISH_OFF_WALL_CRUSH_OVERSHOOT`]. A hunter steering
/// at a point beyond the prey toward the wall charges through it nose-on from the
/// open side, exactly the pin the crush rewards, and a prey wedged in a corner
/// (both axes pinned) gets the aim driven diagonally into the corner, stacking the
/// [`crate::gameplay::combat::corner_crush_ram_damage`] top-up too.
///
/// A prey out in the open (neither axis pinned) is returned untouched, so the kill
/// press only diverts toward a wall when the combat layer would actually pay the
/// crush off.
#[must_use]
pub fn finish_off_wall_crush_aim(prey: Vec2, half_extents: Vec2) -> Vec2 {
    use crate::gameplay::combat::WALL_CRUSH_MARGIN;

    let shove_axis = |coordinate: f32, half: f32| {
        let band = half - WALL_CRUSH_MARGIN;
        if coordinate >= band {
            half + FINISH_OFF_WALL_CRUSH_OVERSHOOT
        } else if coordinate <= -band {
            -(half + FINISH_OFF_WALL_CRUSH_OVERSHOOT)
        } else {
            coordinate
        }
    };

    Vec2::new(
        shove_axis(prey.x, half_extents.x),
        shove_axis(prey.y, half_extents.y),
    )
}

/// Closest position in `positions` to `from`, with the shared deterministic
/// `x`-then-`y` tie-break so the pick never wavers between equidistant targets.
fn nearest_position(from: Vec2, positions: &[Vec2]) -> Option<Vec2> {
    positions.iter().copied().min_by(|a, b| {
        from.distance_squared(*a)
            .partial_cmp(&from.distance_squared(*b))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| compare_positions(*a, *b))
    })
}

/// Nearest enemy threat to `anchor` within `radius`.
///
/// Shared by every CTF threat lookup so they agree on the same tie-breaking:
/// closest first, then by `x`, then by `y` for a deterministic pick.
fn closest_enemy_threat_within(
    team: AiTeam,
    anchor: Vec2,
    radius: f32,
    threats: &[ThreatTarget],
) -> Option<ThreatTarget> {
    let radius_sq = radius * radius;
    threats
        .iter()
        .copied()
        .filter_map(|threat| {
            let distance_sq = threat.position.distance_squared(anchor);
            (threat.team == team.enemy() && distance_sq <= radius_sq)
                .then_some((threat, distance_sq))
        })
        .min_by(|(a_threat, a_dist), (b_threat, b_dist)| {
            a_dist
                .partial_cmp(b_dist)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| compare_positions(a_threat.position, b_threat.position))
        })
        .map(|(threat, _)| threat)
}

fn closest_home_flag_threat(
    team: AiTeam,
    own_flag: &FlagTarget,
    threats: &[ThreatTarget],
) -> Option<ThreatTarget> {
    if own_flag.holder.is_some() {
        return None;
    }

    closest_enemy_threat_within(team, own_flag.position, HOME_FLAG_THREAT_RADIUS, threats)
}

fn closest_home_base_contester(
    team: AiTeam,
    home_base: Vec2,
    threats: &[ThreatTarget],
) -> Option<ThreatTarget> {
    closest_enemy_threat_within(team, home_base, HOME_BASE_CONTEST_RADIUS, threats)
}

fn closest_flag_carrier_pursuer(
    team: AiTeam,
    carrier_position: Vec2,
    threats: &[ThreatTarget],
) -> Option<ThreatTarget> {
    closest_enemy_threat_within(team, carrier_position, FLAG_CARRIER_PURSUER_RADIUS, threats)
}

fn contested_home_base_staging_point(home_base: Vec2, threat_position: Vec2) -> Vec2 {
    let to_home = home_base - threat_position;
    let Some(direction) = to_home.try_normalize() else {
        return home_base + Vec2::X * CONTESTED_HOME_BASE_STAGING_DISTANCE;
    };

    home_base + direction * CONTESTED_HOME_BASE_STAGING_DISTANCE
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
            | DrivingTarget::EscortFlagCarrier(_)
            | DrivingTarget::HomeBase(_)
            | DrivingTarget::MidfieldInterceptor(_)
            | DrivingTarget::StolenHomeFlag(_)
            | DrivingTarget::StolenHomeFlagRouteGuard(_)
    ) {
        return None;
    }

    let target_distance_sq = position.distance_squared(target.position());
    let min_priority = closing_time_detour_min_priority(choices.closing_time_discipline);
    best_pickup(
        position,
        choices.pickups,
        choices.pickup_pursuit_radius,
        |pickup| {
            pickup.priority >= min_priority
                && position.distance_squared(pickup.position) < target_distance_sq
                && is_ahead_of_target_push(position, pickup.position, target.position())
                && is_on_target_lane(
                    position,
                    pickup.position,
                    target.position(),
                    pickup_lane_width(pickup.priority),
                )
        },
    )
}

/// Minimum pickup priority that still justifies a CTF detour this frame.
///
/// In normal play a car breaks off its objective for any pickup worth the base
/// [`CTF_PICKUP_DETOUR_MIN_PRIORITY`]. Once a team disciplines its detours in
/// closing time the bar rises to [`CTF_WIDE_DETOUR_MIN_PRIORITY`], so only a
/// pickup already worth a wide gamble (nitro's race pressure or a battered team's
/// integrity-scaled repair/shield) is worth the time, while a mere cash bag is
/// left on the track in favour of racing the flag home.
const fn closing_time_detour_min_priority(closing_time_discipline: bool) -> u32 {
    if closing_time_discipline {
        CTF_WIDE_DETOUR_MIN_PRIORITY
    } else {
        CTF_PICKUP_DETOUR_MIN_PRIORITY
    }
}

fn is_ahead_of_target_push(position: Vec2, pickup: Vec2, target: Vec2) -> bool {
    let to_pickup = pickup - position;
    let to_target = target - position;
    to_pickup.dot(to_target) > 0.0
}

const fn pickup_lane_width(priority: u32) -> f32 {
    if priority >= CTF_WIDE_DETOUR_MIN_PRIORITY {
        CTF_HIGH_VALUE_PICKUP_LANE_WIDTH
    } else {
        CTF_PICKUP_LANE_WIDTH
    }
}

fn is_on_target_lane(position: Vec2, pickup: Vec2, target: Vec2, lane_width: f32) -> bool {
    let to_target = target - position;
    let Some(direction) = to_target.try_normalize() else {
        return false;
    };

    let lateral_distance = (pickup - position).perp_dot(direction).abs();
    lateral_distance <= lane_width
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
        .min_by(compare_pickups_by_priority_distance_and_position)
        .map(|(pickup, _)| pickup)
}

fn compare_pickups_by_priority_distance_and_position(
    (a_pickup, a_dist): &(PickupTarget, f32),
    (b_pickup, b_dist): &(PickupTarget, f32),
) -> std::cmp::Ordering {
    b_pickup
        .priority
        .cmp(&a_pickup.priority)
        .then_with(|| {
            a_dist
                .partial_cmp(b_dist)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| compare_positions(a_pickup.position, b_pickup.position))
}

/// Deterministic tie-breaker that orders two world positions by `x`, then `y`.
///
/// Shared by every "nearest" lookup in the virtual-player brain so they all
/// settle ties the same way, keeping target selection stable frame to frame.
/// `NaN` coordinates compare as equal so a degenerate position never panics.
#[must_use]
pub fn compare_positions(a: Vec2, b: Vec2) -> std::cmp::Ordering {
    a.x.partial_cmp(&b.x)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
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
            closing_time_detour_min_priority(false),
            CTF_PICKUP_DETOUR_MIN_PRIORITY,
            "normal play breaks off for any pickup worth the base detour"
        );
        assert_eq!(
            closing_time_detour_min_priority(true),
            CTF_WIDE_DETOUR_MIN_PRIORITY,
            "a committed team only breaks off for a wide-detour-worthy grab"
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
        );

        assert_eq!(target, Some(DrivingTarget::HomeBase(Vec2::new(500.0, 0.0))));
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
            }],
        );

        assert_eq!(
            target,
            Some(DrivingTarget::ContestedHomeBaseStaging(Vec2::new(
                740.0, 0.0
            )))
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
            }],
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
            }],
        );

        assert_eq!(
            target,
            Some(DrivingTarget::BlockFlagCarrierPursuer(Vec2::new(
                -40.0, 0.0
            )))
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
            }],
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
            }],
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
                },
                ThreatTarget {
                    team: AiTeam::Blue,
                    position: Vec2::new(500.0, 90.0),
                },
            ],
        );

        assert_eq!(
            target,
            Some(DrivingTarget::UrgentDefendHomeBase(Vec2::new(500.0, 90.0)))
        );
    }

    #[test]
    fn closest_enemy_threat_within_picks_nearest_and_ignores_allies_and_range() {
        let threats = [
            ThreatTarget {
                team: AiTeam::Red,
                position: Vec2::new(40.0, 0.0),
            },
            ThreatTarget {
                team: AiTeam::Blue,
                position: Vec2::new(60.0, 0.0),
            },
            ThreatTarget {
                team: AiTeam::Blue,
                position: Vec2::new(10.0, 0.0),
            },
            ThreatTarget {
                team: AiTeam::Blue,
                position: Vec2::new(5000.0, 0.0),
            },
        ];

        let nearest = closest_enemy_threat_within(AiTeam::Red, Vec2::ZERO, 200.0, &threats);

        assert_eq!(
            nearest,
            Some(ThreatTarget {
                team: AiTeam::Blue,
                position: Vec2::new(10.0, 0.0),
            })
        );
    }

    #[test]
    fn closest_enemy_threat_within_breaks_ties_by_position() {
        let threats = [
            ThreatTarget {
                team: AiTeam::Blue,
                position: Vec2::new(0.0, 50.0),
            },
            ThreatTarget {
                team: AiTeam::Blue,
                position: Vec2::new(0.0, -50.0),
            },
            ThreatTarget {
                team: AiTeam::Blue,
                position: Vec2::new(-50.0, 0.0),
            },
        ];

        let nearest = closest_enemy_threat_within(AiTeam::Red, Vec2::ZERO, 200.0, &threats);

        assert_eq!(
            nearest,
            Some(ThreatTarget {
                team: AiTeam::Blue,
                position: Vec2::new(-50.0, 0.0),
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
            }],
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

        assert_eq!(pit_retreat_car(0.5, &candidates), None);
    }

    #[test]
    fn pit_retreat_sends_the_home_most_car_when_battered() {
        let home = Vec2::new(500.0, 0.0);
        let candidates = [
            pit_candidate(1, Vec2::new(-200.0, 0.0), home),
            pit_candidate(2, Vec2::new(450.0, 0.0), home),
        ];

        assert_eq!(
            pit_retreat_car(PIT_RETREAT_INTEGRITY_FRACTION, &candidates),
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
            pit_retreat_car(PIT_RETREAT_INTEGRITY_FRACTION, &candidates),
            Some(Entity::from_raw(1))
        );
        let just_above = PIT_RETREAT_INTEGRITY_FRACTION + 0.001;
        assert_eq!(pit_retreat_car(just_above, &candidates), None);
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
            pit_retreat_car(0.1, &[carrier, defender]),
            Some(Entity::from_raw(2))
        );
    }

    #[test]
    fn pit_retreat_keeps_the_last_car_on_duty() {
        let home = Vec2::new(500.0, 0.0);
        let lone = [pit_candidate(1, Vec2::new(480.0, 0.0), home)];

        assert_eq!(pit_retreat_car(0.05, &lone), None);
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

        assert_eq!(pit_retreat_car(0.05, &carriers), None);
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

        assert_eq!(pit_retreat_car(0.2, &candidates), Some(Entity::from_raw(2)));
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
            finish_off_car(1.0, 0.6, false, &candidates, &enemies, None),
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
            finish_off_car(1.0, 0.0, false, &candidates, &enemies, None),
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
            finish_off_car(0.2, 0.2, false, &candidates, &enemies, None),
            None
        );
        // We are the more battered: pressing would be suicidal, behind or not.
        assert_eq!(
            finish_off_car(0.1, 0.25, false, &candidates, &enemies, None),
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
            finish_off_car(0.8, 0.2, false, &candidates, &enemies, None),
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
            finish_off_car(0.9, 0.15, false, &candidates, &enemies, None),
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
        )
        .is_some());
        let just_above = FINISH_OFF_ENEMY_INTEGRITY_FRACTION + 0.001;
        assert_eq!(
            finish_off_car(0.9, just_above, false, &candidates, &enemies, None),
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
            finish_off_car(0.8, 0.2, false, &[carrier, hunter], &enemies, None),
            Some((Entity::from_raw(2), Vec2::new(500.0, 0.0)))
        );
    }

    #[test]
    fn finish_off_keeps_the_last_car_on_duty() {
        let lone = [finish_off_candidate(1, Vec2::new(100.0, 0.0))];
        let enemies = [Vec2::new(500.0, 0.0)];

        assert_eq!(finish_off_car(0.8, 0.2, false, &lone, &enemies, None), None);
    }

    #[test]
    fn finish_off_returns_none_with_no_enemy_to_hunt() {
        let candidates = [
            finish_off_candidate(1, Vec2::new(100.0, 0.0)),
            finish_off_candidate(2, Vec2::new(-300.0, 0.0)),
        ];

        assert_eq!(
            finish_off_car(0.8, 0.2, false, &candidates, &[], None),
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
            finish_off_car(0.8, 0.2, false, &carriers, &enemies, None),
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
            finish_off_car(0.8, 0.2, false, &candidates, &enemies, None),
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
            finish_off_car(0.2, 0.2, false, &candidates, &enemies, None),
            None
        );
        assert_eq!(
            finish_off_car(0.2, 0.2, true, &candidates, &enemies, None),
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
            finish_off_car(0.15, 0.25, true, &candidates, &enemies, None),
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
            finish_off_car(0.2, 0.6, true, &candidates, &enemies, None),
            None
        );
        assert_eq!(
            finish_off_car(0.2, 0.0, true, &candidates, &enemies, None),
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
            finish_off_car(0.8, 0.2, false, &candidates, &enemies, Some(carrier)),
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
            finish_off_car(0.8, 0.2, false, &candidates, &enemies, None),
            Some((Entity::from_raw(1), stray)),
            "with no flag stolen the nearer kill still wins"
        );
        assert_eq!(
            finish_off_car(0.8, 0.2, false, &candidates, &enemies, Some(carrier)),
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
            finish_off_car(0.8, 0.2, false, &candidates, &enemies, Some(carrier)),
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
            finish_off_car(0.8, 0.6, false, &candidates, &enemies, Some(carrier)),
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
            finish_off_car(0.8, 0.2, false, &lone, &enemies, Some(carrier)),
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
}
