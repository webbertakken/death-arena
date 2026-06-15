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

/// Baseline pickup-scavenging greed: the all-rounder's
/// [`crate::gameplay::virtual_player::VirtualPlayer::pickup_pursuit_radius`] (the
/// former uniform global, also the human's mirror). A driver with exactly this
/// greed detours for a CTF pickup within the unscaled [`CTF_PICKUP_LANE_WIDTH`] /
/// [`CTF_HIGH_VALUE_PICKUP_LANE_WIDTH`] lane; the lane then scales with a driver's
/// greed relative to this baseline, so a greedier driver swings wider off its
/// objective line for loot while a disciplined one keeps a tighter line (see
/// [`pickup_lane_width`]). The in-objective mirror of the trackside scavenging
/// reach the same greed axis already sets.
pub const BASELINE_PICKUP_PURSUIT_RADIUS: f32 = 450.0;

/// Floor and ceiling on the greed-driven detour-lane scale: a safety net so a
/// degenerate `pickup_pursuit_radius` can never collapse the lane to nothing nor
/// blow it out across the arena. The asserted roster greed band (`340..=580`, see
/// [`crate::gameplay::virtual_player::spawn`]) maps strictly inside this band, so
/// the clamp only ever guards a garbage radius, never a legal driver's
/// personality.
const GREED_LANE_SCALE_MIN: f32 = 0.70;
const GREED_LANE_SCALE_MAX: f32 = 1.35;

/// The greed scale is centred on the baseline driver (scale `1.0`, an unscaled
/// lane) and never inverts discipline: a greedier driver always gets at least as
/// wide a detour lane as a more disciplined one. Enforced at compile time.
const _: () = assert!(GREED_LANE_SCALE_MIN < 1.0 && GREED_LANE_SCALE_MAX > 1.0);

/// Distance from home at which a flag carrier stops gambling on pickup detours
/// and commits to finishing the capture.
pub const FLAG_CARRIER_CAPTURE_COMMIT_DISTANCE: f32 = 180.0;

/// Half-width of the lane, measured from a flag carrier's straight line home,
/// within which an enemy counts as planted on the racing line and worth juking
/// around. Sits just inside the `CTF_HIGH_VALUE_PICKUP_LANE_WIDTH` detour lane so
/// the carrier only swerves for a foe genuinely in the way, not one merely off to
/// the shoulder.
pub const CARRIER_JUKE_LANE_WIDTH: f32 = 90.0;

/// Sideways distance a flag carrier swings its aim away from a lane blocker, so
/// it arcs around the roadblock on its run home rather than ramming straight into
/// it and eating the doubled [`crate::gameplay::combat::FLAG_CARRIER_RAM_DAMAGE_PER_FRAME`]
/// a carrier takes. The aim snaps back to the base the moment the blocker clears
/// the lane (it is recomputed every frame) and is dropped entirely inside
/// [`FLAG_CARRIER_CAPTURE_COMMIT_DISTANCE`], so the final approach is always a
/// straight commit to the capture.
pub const CARRIER_JUKE_OFFSET: f32 = 160.0;

/// How far a driver's run-home juke line tightens per unit of cornering
/// commitment away from the neutral [`MIN_THROTTLE`] baseline.
///
/// A reckless driver (a higher
/// [`crate::gameplay::virtual_player::VirtualPlayer::corner_throttle`]) commits to a
/// *tighter* line home: a smaller juke offset, so it squeezes past a roadblock on a
/// shorter, faster arc that shaves the time its fragile flag run stays exposed, at
/// the cost of passing closer to the very ram it is dodging. A disciplined driver
/// swings a wider, safer berth. The run-home mirror of the same commitment axis that
/// sets how hard a driver stays on the gas through a corner and how deep it noses a
/// kill home (`pursuit_arrive_radius`), so a keen driver commits to its line
/// everywhere: into a bend, onto a kill, and past a blocker on the scoring run.
const CARRIER_JUKE_COMMITMENT_GAIN: f32 = 100.0;

/// Tightest run-home line the clamp will allow: a safety net so a degenerate or
/// extreme-reckless `corner_throttle` can never collapse the juke into aiming
/// straight through the blocker. It sits at or outside true ram range (asserted
/// below), so even a fully-clamped squeeze still arcs around the blocker. The
/// keenest line the roster actually fields, the sprinter's reckless `0.42`, still
/// maps inside this floor (see [`carrier_juke_offset`]), so the clamp only ever
/// guards a throttle past the whole roster, never a real driver's line.
const CARRIER_JUKE_OFFSET_MIN: f32 = 145.0;

/// Widest run-home berth the clamp will allow: the disciplined counterpart to the
/// floor, so a degenerate or extreme-timid `corner_throttle` tops out here instead
/// of swinging an absurdly wide arc. The safest line the roster actually fields, the
/// technician's careful `0.20`, still maps inside this ceiling, so it too only ever
/// guards a throttle past the whole roster, never a real driver's line.
const CARRIER_JUKE_OFFSET_MAX: f32 = 180.0;

/// The keenest juke must still aim at or outside true ram range, enforced at compile
/// time, so even the most reckless squeeze arcs around the blocker rather than
/// straight into it.
const _: () = assert!(CARRIER_JUKE_OFFSET_MIN >= crate::gameplay::combat::RAM_RADIUS);

/// The neutral baseline must sit inside the commitment band, enforced at compile
/// time, so a reckless rival flexes a tighter line and a disciplined one a wider
/// berth.
const _: () = assert!(
    CARRIER_JUKE_OFFSET_MIN < CARRIER_JUKE_OFFSET && CARRIER_JUKE_OFFSET < CARRIER_JUKE_OFFSET_MAX
);

/// The juke must genuinely tighten with commitment, never widen or stay flat,
/// enforced at compile time.
const _: () = assert!(CARRIER_JUKE_COMMITMENT_GAIN > 0.0);

/// Sideways distance a car swings its aim away from a lane blocker on its run home,
/// flexed by the driver's cornering commitment.
///
/// A driver cornering on the neutral [`MIN_THROTTLE`] floor swings the exact
/// [`CARRIER_JUKE_OFFSET`] baseline, so the human's mirror and every pre-commitment
/// juke are unchanged. A reckless driver (a higher `corner_throttle`) tightens it
/// toward [`CARRIER_JUKE_OFFSET_MIN`]; a disciplined one widens it toward
/// [`CARRIER_JUKE_OFFSET_MAX`]. The affine map is clamped to the
/// [[`CARRIER_JUKE_OFFSET_MIN`], [`CARRIER_JUKE_OFFSET_MAX`]] band as a safety net
/// for a degenerate throttle.
#[must_use]
fn carrier_juke_offset(corner_throttle: f32) -> f32 {
    let tighten = (corner_throttle - MIN_THROTTLE) * CARRIER_JUKE_COMMITMENT_GAIN;
    (CARRIER_JUKE_OFFSET - tighten).clamp(CARRIER_JUKE_OFFSET_MIN, CARRIER_JUKE_OFFSET_MAX)
}

/// Distance around home base where an enemy blocks a carried-flag capture.
pub const HOME_BASE_CONTEST_RADIUS: f32 = 160.0;

/// Distance around a friendly flag carrier where enemies count as pursuers.
pub const FLAG_CARRIER_PURSUER_RADIUS: f32 = 260.0;

/// Distance from the flag carrier at which a blocking teammate interposes against
/// an incoming pursuer.
///
/// The carrier-side mirror of [`HOME_FLAG_DEFENSE_DISTANCE`]: where a home-flag
/// defender meets a thief on a ring around the *flag*, a carrier's blocker meets a
/// pursuer on a ring around the *carrier*. Anchored to
/// [`crate::gameplay::combat::RAM_RADIUS`] so the block lands exactly as the
/// pursuer closes into ramming range of the fragile carrier (which bleeds at the
/// doubled [`crate::gameplay::combat::FLAG_CARRIER_RAM_DAMAGE_PER_FRAME`]), rather
/// than once it is already trading paint.
pub const FLAG_CARRIER_PURSUER_BLOCK_STANDOFF: f32 = crate::gameplay::combat::RAM_RADIUS;

/// The block ring must sit inside the pursuer detection radius, enforced at compile
/// time, so a detected pursuer outside it can actually be led to the ring instead
/// of always being met head-on at the spot it has already left.
const _: () = assert!(FLAG_CARRIER_PURSUER_BLOCK_STANDOFF < FLAG_CARRIER_PURSUER_RADIUS);

/// Distance from home where a flag carrier waits while the base is contested.
pub const CONTESTED_HOME_BASE_STAGING_DISTANCE: f32 = 240.0;

/// Minimum pickup priority that justifies interrupting a CTF objective.
pub const CTF_PICKUP_DETOUR_MIN_PRIORITY: u32 = 50;

/// Priority at which a pickup justifies the wider CTF detour lane.
pub const CTF_WIDE_DETOUR_MIN_PRIORITY: u32 = 150;

/// Closing-time detour bar for a *greedy* driver (its
/// [`crate::gameplay::virtual_player::VirtualPlayer::pickup_pursuit_radius`] a
/// clear step above the baseline).
///
/// In the round's closing stretch every team disciplines its detours up to the
/// wide [`CTF_WIDE_DETOUR_MIN_PRIORITY`]; on top of that the bar is nudged by the
/// driver's greed, the same personality axis that already sets how far afield a
/// car scavenges ([`BASELINE_PICKUP_PURSUIT_RADIUS`]) and how wide its detour lane
/// runs ([`pickup_lane_width`]). A greedy driver keeps gambling on a cheaper grab
/// even at the death, so its bar sits a notch *below* the neutral wide bar, yet
/// stays above the normal-play [`CTF_PICKUP_DETOUR_MIN_PRIORITY`] so closing time
/// always disciplines a detour somewhat (a mere cash bag is still left). At `130`
/// a greedy driver still breaks off for a sabotage-grade grab the neutral driver
/// leaves on the track.
pub const CLOSING_TIME_GREEDY_DETOUR_MIN_PRIORITY: u32 = 130;

/// Closing-time detour bar for a *disciplined* driver (its greed a clear step
/// below the baseline).
///
/// The mirror of [`CLOSING_TIME_GREEDY_DETOUR_MIN_PRIORITY`]: a disciplined driver
/// locks its line down even tighter than the neutral wide bar, so its bar sits
/// *above* [`CTF_WIDE_DETOUR_MIN_PRIORITY`]. At `170` even a nitro is left on the
/// track to race the flag home; only a battered team's integrity-scaled survival
/// grab still tempts it (a wrecked-team repair tops out at `175`, see
/// [`crate::gameplay::pickup::PickupKind::virtual_player_priority_for_integrity`]),
/// so commitment never tips into suicide.
pub const CLOSING_TIME_DISCIPLINED_DETOUR_MIN_PRIORITY: u32 = 170;

/// Greed delta from [`BASELINE_PICKUP_PURSUIT_RADIUS`] beyond which a driver leaves
/// the neutral closing-time band for the greedy or disciplined bar.
///
/// Set so the whole roster is expressed and the baseline driver (and the human,
/// which mirrors it) keeps the exact neutral wide bar: the asserted roster greed
/// band (`340..=580`, see [`crate::gameplay::virtual_player::spawn`]) maps the
/// all-rounder (`450`, delta `0`) to the neutral band, the sprinter (`520`, delta
/// `+70`) to the greedy bar, and the brawler (`400`) and technician (`380`, deltas
/// `-50`/`-70`) to the disciplined bar.
const CLOSING_TIME_GREED_STEP: f32 = 40.0;

/// Greed must never invert closing-time discipline: a greedy driver still
/// disciplines more than normal play (its bar above the base bar) yet gambles more
/// than the neutral driver (its bar below the wide bar), and a disciplined driver
/// locks down tighter than the neutral driver (its bar above the wide bar).
/// Enforced at compile time so the ordering can never drift.
const _: () = assert!(CTF_PICKUP_DETOUR_MIN_PRIORITY < CLOSING_TIME_GREEDY_DETOUR_MIN_PRIORITY);
const _: () = assert!(CLOSING_TIME_GREEDY_DETOUR_MIN_PRIORITY < CTF_WIDE_DETOUR_MIN_PRIORITY);
const _: () = assert!(CTF_WIDE_DETOUR_MIN_PRIORITY < CLOSING_TIME_DISCIPLINED_DETOUR_MIN_PRIORITY);
/// The neutral band must be real (a positive greed step), so the baseline driver
/// keeps the unscaled wide bar. Enforced at compile time.
const _: () = assert!(CLOSING_TIME_GREED_STEP > 0.0);

/// Team durability fraction (`0.0`..=`1.0`) at or below which a battered team
/// breaks one car off the field and sends it home to pit-recover.
///
/// Sits in the same "actively battered" band the integrity-scaled repair and
/// shield pickup tiers already react to (both treat `<= 0.35` as hard-pressed),
/// so a team ground this low patches up at its own base even when no repair
/// pickup is on its lane, the reliable recovery the home-base pit opened up.
pub const PIT_RETREAT_INTEGRITY_FRACTION: f32 = 0.30;

/// Distance from its own base within which a limping car on a pit retreat stops
/// weaving and commits straight into the recovery zone.
///
/// The pit-retreat mirror of [`FLAG_CARRIER_CAPTURE_COMMIT_DISTANCE`]: a battered
/// car weaves around an enemy planted on its run home (see
/// [`pit_retreat_home_run_aim`]) right up until it reaches its base, then
/// straightens up so it parks cleanly in the pit rather than circling it dodging
/// a blocker it has already cleared. Set to the base recovery radius
/// ([`crate::gameplay::combat::BASE_REPAIR_RADIUS`], the same zone the car settles
/// into to heal), so the weave ends exactly as the recovery begins.
pub const PIT_RETREAT_HOME_COMMIT_DISTANCE: f32 = crate::gameplay::combat::BASE_REPAIR_RADIUS;

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

/// Wider enemy durability ceiling a team trailing on captures presses up to once
/// the match reaches its closing stretch: the clutch-wreck window.
///
/// The targeting mirror of the combat
/// [`crate::gameplay::combat::clutch_wreck_bonus`]: a wreck landed in the dying
/// seconds both banks the clutch cash and, on a level overtime, swings the
/// wreck tiebreaker ([`crate::gameplay::ctf::CtfMatchWinner`]) that settles the
/// decider. So a trailing team running out of clock stops waiting for an enemy
/// to be *badly* reeling and presses a merely *worn* one, chasing the clutch
/// wreck that can win the round outright. Held a clear step below half integrity
/// so the press still needs an enemy genuinely on the back foot, never a
/// near-pristine one, and the "we must be at least as healthy" guard
/// ([`finish_off_car`]) keeps the gamble off a suicidal trade.
pub const CLUTCH_FINISH_OFF_ENEMY_INTEGRITY_FRACTION: f32 = 0.45;

/// The clutch window must genuinely widen the press, enforced at compile time, so
/// closing-time desperation reaches an enemy the normal reeling gate leaves alone.
const _: () =
    assert!(CLUTCH_FINISH_OFF_ENEMY_INTEGRITY_FRACTION > FINISH_OFF_ENEMY_INTEGRITY_FRACTION);

/// The clutch window must stay below half integrity, enforced at compile time, so
/// even a last-ditch press still needs an enemy on the back foot and never trades
/// into a fresh one.
const _: () = assert!(CLUTCH_FINISH_OFF_ENEMY_INTEGRITY_FRACTION < 0.5);

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
    /// Instantaneous velocity (heading times top speed), so a defender can lead a
    /// moving thief to where it will breach the defensive ring rather than
    /// body-block the spot it has already left. The human player carries its own
    /// tracked velocity here too (see
    /// [`crate::gameplay::virtual_player::drive::PlayerVelocity`]); a genuinely
    /// stationary threat, or one whose track is unavailable, carries a zero
    /// velocity, for which the lead falls back to a plain body-block.
    pub velocity: Vec2,
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
    corner_throttle: f32,
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
        // The base is clear of contesters, so commit homeward, juking around any
        // enemy planted on the run home rather than ramming it for doubled damage.
        // A held flag sits at its carrier, so the enemy flag's position is ours.
        return Some(DrivingTarget::HomeBase(carrier_home_run_aim(
            enemy_flag.position,
            own_flag.home,
            team,
            threats,
            corner_throttle,
        )));
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
            return Some(DrivingTarget::BlockFlagCarrierPursuer(
                block_pursuer_intercept_point(enemy_flag.position, threat),
            ));
        }
        return Some(DrivingTarget::EscortFlagCarrier(escort_lead_point(
            enemy_flag.position,
            own_flag.home,
        )));
    }

    if let Some(threat) = closest_home_flag_threat(team, own_flag, threats) {
        let target = defensive_intercept_point(own_flag.position, threat);
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
/// Three guards keep the retreat from backfiring:
/// - a team trailing on captures in the closing stretch (`behind_on_captures` and
///   `closing_time`) never retreats: with the clock running out a heal cannot pay
///   off before the round ends, so every car stays on the equalising push instead
///   of one limping home. The same clutch desperation that widens the kill press
///   to a worn leader (see [`finish_off_car`]) cancels the pit stop here, keeping
///   the whole team's closing-time play coherent;
/// - a flag carrier is never pulled off its capture run (it already heals at
///   home as it scores), so only non-carriers are eligible;
/// - at least one car must stay on duty, so a lone car never abandons the field
///   just to heal.
#[must_use]
pub fn pit_retreat_car(
    integrity_fraction: f32,
    behind_on_captures: bool,
    closing_time: bool,
    candidates: &[PitRetreatCandidate],
) -> Option<Entity> {
    if behind_on_captures && closing_time {
        return None;
    }
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
/// In the match's closing stretch a team `behind_on_captures` widens that reeling
/// gate to [`CLUTCH_FINISH_OFF_ENEMY_INTEGRITY_FRACTION`] (`closing_time`), chasing
/// the clutch wreck that banks the [`crate::gameplay::combat::clutch_wreck_bonus`]
/// and can swing the level-overtime decider: running out of clock, it presses a
/// merely *worn* leader, not only a badly reeling one. The health guard below is
/// untouched, so the desperation never tips into a suicidal trade.
///
/// Guards keep the press from backfiring:
/// - the enemy must be reeling but not already wrecked: above zero yet at or
///   below [`FINISH_OFF_ENEMY_INTEGRITY_FRACTION`] (or, in the closing-time clutch
///   window, [`CLUTCH_FINISH_OFF_ENEMY_INTEGRITY_FRACTION`]). A wreck already paid
///   out and a stunned enemy has no pool left to grind;
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
    closing_time: bool,
) -> Option<(Entity, Vec2)> {
    // In the closing stretch a trailing team chases the clutch wreck that can win
    // the decider, so it presses a merely worn leader, not only a badly reeling
    // one. Every other situation holds to the standard reeling gate.
    let reeling_ceiling = if closing_time && behind_on_captures {
        CLUTCH_FINISH_OFF_ENEMY_INTEGRITY_FRACTION
    } else {
        FINISH_OFF_ENEMY_INTEGRITY_FRACTION
    };
    if enemy_integrity_fraction <= 0.0 || enemy_integrity_fraction > reeling_ceiling {
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

/// Whether `prey` sits within the wall-crush band of an arena wall on either
/// axis, the precondition for [`finish_off_wall_crush_aim`] to shove it in.
///
/// The "any axis pinned" companion to the per-axis shove inside
/// [`finish_off_wall_crush_aim`]: it answers only *whether* a wall crush is on
/// (so [`finish_off_aim`] can pick between the crush and the open-field lead),
/// reading the same [`crate::gameplay::combat::WALL_CRUSH_MARGIN`] band the combat
/// [`crate::gameplay::combat::wall_crush_ram_damage`] rewards.
fn prey_is_wall_pinned(prey: Vec2, half_extents: Vec2) -> bool {
    use crate::gameplay::combat::WALL_CRUSH_MARGIN;

    let pinned = |coordinate: f32, half: f32| coordinate.abs() >= half - WALL_CRUSH_MARGIN;
    pinned(prey.x, half_extents.x) || pinned(prey.y, half_extents.y)
}

/// Picks the aim a [`finish_off_car`] hunter charges at: shove a wall-pinned prey
/// into the boundary, otherwise head a fleeing prey off in the open.
///
/// Composes the two finishing aims so the kill press always sets up the best
/// available finisher. When the prey is pinned against a wall the wall crush
/// out-damages an open ram, so [`finish_off_wall_crush_aim`] wins and the
/// `prey_velocity` is irrelevant (a pinned car has nowhere to run). Out in the
/// open the hunter instead leads the prey with [`finish_off_lead_aim`], cutting
/// the runner off rather than tail-chasing the spot it has already left.
#[must_use]
pub fn finish_off_aim(
    hunter: Vec2,
    prey: Vec2,
    prey_velocity: Vec2,
    hunter_speed: f32,
    half_extents: Vec2,
) -> Vec2 {
    if prey_is_wall_pinned(prey, half_extents) {
        finish_off_wall_crush_aim(prey, half_extents)
    } else {
        finish_off_lead_aim(hunter, prey, prey_velocity, hunter_speed)
    }
}

/// Aims a [`finish_off_car`] hunter at where a fleeing prey is heading so it cuts
/// the runner off instead of tail-chasing the spot it has already left.
///
/// The interception ("lead the target") counterpart to pure pursuit: given the
/// prey's instantaneous `prey_velocity` and the hunter's own `hunter_speed`, it
/// solves for the earliest point along the prey's path the hunter can reach at the
/// same moment (see [`interception_time`]), the classic Death Rally "head them off,
/// you cannot outrun the finisher".
///
/// The lead is deliberately *extend-only*: the cut-off point is taken only when it
/// sits at least as far from the hunter as the prey itself does. Leading a prey
/// that is fleeing or crossing therefore pushes the aim ahead of it, while a prey
/// closing straight onto the hunter (where an interception point would fall short
/// and stall the charge before contact) keeps the pure-pursuit aim at the prey.
/// It also falls back to the prey's current spot whenever the prey is barely moving
/// or no real interception exists (an equal-or-faster prey fleeing dead away), so a
/// kill press is never worse than the tail chase it replaces. The aim is recomputed
/// every frame, so an over-lead (the prey is slowed while reeling, so its true speed
/// is below the `prey_velocity` magnitude) self-corrects as the prey's real position
/// updates.
#[must_use]
pub fn finish_off_lead_aim(
    hunter: Vec2,
    prey: Vec2,
    prey_velocity: Vec2,
    hunter_speed: f32,
) -> Vec2 {
    let Some(time) = interception_time(prey - hunter, prey_velocity, hunter_speed) else {
        return prey;
    };

    let aim = prey + prey_velocity * time;
    // Extend-only: never pull the aim closer than the prey, so the hunter always
    // drives through to contact rather than stalling short of where the prey is.
    if aim.distance_squared(hunter) >= prey.distance_squared(hunter) {
        aim
    } else {
        prey
    }
}

/// Earliest time `t >= 0` at which a pursuer of speed `speed` starting at the
/// origin can reach a target that starts at `offset` and travels at constant
/// `velocity`.
///
/// Solves `|offset + velocity * t| = speed * t` for the smallest non-negative
/// root, the standard constant-velocity interception. Returns `None` when no such
/// time exists (the target outruns the pursuer, or the geometry is degenerate), so
/// the caller can fall back to a plain tail chase.
fn interception_time(offset: Vec2, velocity: Vec2, speed: f32) -> Option<f32> {
    let leading = speed.mul_add(-speed, velocity.length_squared());
    let linear = 2.0 * offset.dot(velocity);
    let constant = offset.length_squared();

    // A near-zero leading term means the target travels at (almost) the pursuer's
    // speed, so the quadratic degenerates to the line `linear * t + constant = 0`.
    if leading.abs() <= f32::EPSILON {
        if linear.abs() <= f32::EPSILON {
            return None;
        }
        let time = -constant / linear;
        return (time >= 0.0).then_some(time);
    }

    let discriminant = linear.mul_add(linear, -4.0 * leading * constant);
    if discriminant < 0.0 {
        return None;
    }

    let root = discriminant.sqrt();
    let denominator = 2.0 * leading;
    earliest_non_negative(
        (-linear - root) / denominator,
        (-linear + root) / denominator,
    )
}

/// The smaller of two candidate interception times that is still non-negative, or
/// `None` when both lie in the past.
fn earliest_non_negative(first: f32, second: f32) -> Option<f32> {
    let (lower, higher) = if first <= second {
        (first, second)
    } else {
        (second, first)
    };

    if lower >= 0.0 {
        Some(lower)
    } else if higher >= 0.0 {
        Some(higher)
    } else {
        None
    }
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

/// Where a home-flag defender meets an incoming thief.
///
/// The defensive mirror of [`finish_off_lead_aim`]: where the offensive finisher
/// leads a fleeing prey to where it is heading, this leads an approaching thief to
/// where it will *breach the defensive ring* around the flag, so the defender
/// body-blocks the crossing instead of the spot the thief has already left. The
/// classic Death Rally "cut them off before they reach the flag".
///
/// - A thief already inside the ring ([`HOME_FLAG_DEFENSE_DISTANCE`]) is met
///   head-on at its current spot, exactly as before.
/// - A thief outside the ring is led to the point on the ring it will cross first
///   (see [`ring_breach_time`]), which sits at the same [`HOME_FLAG_DEFENSE_DISTANCE`]
///   from the flag as the plain body-block but out on the thief's true line of
///   approach. A thief driving straight at the flag crosses the ring on its current
///   bearing, so the lead coincides with the plain body-block and nothing changes;
///   only an angled or sweeping approach shifts the meet-point onto the side the
///   thief is actually heading for.
/// - A stationary thief, or one veering away so it never breaches the ring, falls
///   back to the plain body-block on its current bearing, so the lead is never
///   worse than the static block it replaces.
///
/// Recomputed every frame, so the meet-point tracks the thief as it manoeuvres.
fn defensive_intercept_point(flag_position: Vec2, threat: ThreatTarget) -> Vec2 {
    lead_threat_to_ring(flag_position, threat, HOME_FLAG_DEFENSE_DISTANCE)
}

/// Where a teammate intercepts an enemy chasing down the friendly flag carrier.
///
/// The carrier-side mirror of [`defensive_intercept_point`]: instead of guarding a
/// fixed flag, it guards the *moving* carrier, leading the pursuer to where it will
/// breach a ring of [`FLAG_CARRIER_PURSUER_BLOCK_STANDOFF`] around the carrier so
/// the blocker interposes on the pursuer's true line of approach the instant it
/// closes into ramming range, rather than body-blocking the spot the pursuer has
/// already left. Recomputed every frame, so the meet-point tracks both the weaving
/// pursuer and the fleeing carrier. The classic Death Rally "shield the runner",
/// and the fragile carrier (which bleeds at the doubled
/// [`crate::gameplay::combat::FLAG_CARRIER_RAM_DAMAGE_PER_FRAME`]) needs it most.
fn block_pursuer_intercept_point(carrier_position: Vec2, threat: ThreatTarget) -> Vec2 {
    lead_threat_to_ring(
        carrier_position,
        threat,
        FLAG_CARRIER_PURSUER_BLOCK_STANDOFF,
    )
}

/// Leads a moving `threat` to where it will first breach a ring of `radius` around
/// `centre`, so a defender meets it on its true line of approach rather than the
/// spot it has already left.
///
/// The shared core of both defensive leads: the home-flag defender
/// ([`defensive_intercept_point`], ring around the flag) and the flag-carrier
/// blocker ([`block_pursuer_intercept_point`], ring around the carrier).
///
/// - A threat already inside the ring is met head-on at its current spot.
/// - A threat outside the ring is led to the point it crosses first (see
///   [`ring_breach_time`]). The breach point depends only on the threat's heading,
///   not its speed (scaling the velocity scales the solved time inversely and
///   cancels), so the rough `heading * top speed` estimate the threat carries pins
///   the meet-point exactly.
/// - A stationary threat, or one veering away so it never breaches, falls back to
///   the static body-block on the ring at its current bearing, so the lead is never
///   worse than the block it replaces.
fn lead_threat_to_ring(centre: Vec2, threat: ThreatTarget, radius: f32) -> Vec2 {
    let to_threat = threat.position - centre;
    if to_threat.length() <= radius {
        return threat.position;
    }

    let Some(direction) = to_threat.try_normalize() else {
        return centre;
    };
    let static_block = centre + direction * radius;

    let Some(time) = ring_breach_time(to_threat, threat.velocity, radius) else {
        return static_block;
    };
    threat.position + threat.velocity * time
}

/// Earliest time `t >= 0` at which a thief starting at `offset` from a ring's
/// centre and travelling at constant `velocity` first reaches distance `radius`
/// from that centre.
///
/// Solves `|offset + velocity * t| = radius` for the smallest non-negative root,
/// the body-block counterpart to [`interception_time`]'s pursuit solve. The caller
/// only invokes it for a thief already outside the ring (`|offset| > radius`), so a
/// real breach is the nearer of the two roots; it returns `None` when the thief is
/// stationary or veers away so it never breaches, letting the caller hold a plain
/// body-block.
fn ring_breach_time(offset: Vec2, velocity: Vec2, radius: f32) -> Option<f32> {
    let quadratic = velocity.length_squared();
    // A stationary thief never breaches the ring; hold the static body-block.
    if quadratic <= f32::EPSILON {
        return None;
    }

    let linear = 2.0 * offset.dot(velocity);
    let constant = radius.mul_add(-radius, offset.length_squared());
    let discriminant = linear.mul_add(linear, -4.0 * quadratic * constant);
    if discriminant < 0.0 {
        return None;
    }

    let root = discriminant.sqrt();
    let denominator = 2.0 * quadratic;
    earliest_non_negative(
        (-linear - root) / denominator,
        (-linear + root) / denominator,
    )
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

/// The aim a flag carrier drives at on its run home.
///
/// Usually the base itself, but when an enemy is planted on the carrier's
/// straight line home the aim swings out to one side by [`CARRIER_JUKE_OFFSET`],
/// so the carrier arcs around the roadblock instead of ramming straight into it
/// and eating the doubled [`crate::gameplay::combat::FLAG_CARRIER_RAM_DAMAGE_PER_FRAME`]
/// a carrier takes. The classic Death Rally "shake the roadblock on the way to
/// score".
///
/// The swerve is deliberately minimal so it never fights the rest of the brain:
/// - inside [`FLAG_CARRIER_CAPTURE_COMMIT_DISTANCE`] it commits straight to the
///   capture, so the final approach is always a clean run at the base;
/// - it only reacts to an *enemy* (a teammate on the line is no threat) that is
///   genuinely in the way (ahead, nearer than the base, and within
///   [`CARRIER_JUKE_LANE_WIDTH`] of the line), so it stays straight whenever the
///   lane is clear;
/// - it swerves away from the *nearest* such blocker, and a blocker sitting dead
///   on the line picks a deterministic side so the carrier never stalls head-on.
///
/// Recomputed every frame, so the moment the blocker clears the lane the aim
/// snaps back to the base and the carrier straightens up. Near-base contesters are
/// already handled upstream (the carrier stages outside a contested base), so this
/// only ever jukes a midfield roadblock.
#[must_use]
pub fn carrier_home_run_aim(
    carrier: Vec2,
    home: Vec2,
    team: AiTeam,
    threats: &[ThreatTarget],
    corner_throttle: f32,
) -> Vec2 {
    let to_home = home - carrier;
    if to_home.length_squared()
        <= FLAG_CARRIER_CAPTURE_COMMIT_DISTANCE * FLAG_CARRIER_CAPTURE_COMMIT_DISTANCE
    {
        return home;
    }

    let Some(direction) = to_home.try_normalize() else {
        return home;
    };

    let Some(blocker) = nearest_lane_blocker(carrier, home, team, threats) else {
        return home;
    };

    juke_aim_around_blocker(carrier, home, direction, blocker, corner_throttle)
}

/// Swings the aim to the side opposite a `blocker` so a car arcs around it on its
/// straight run home rather than ramming straight into it.
///
/// Shared by the flag carrier's home run ([`carrier_home_run_aim`]) and a
/// battered car's pit retreat ([`pit_retreat_home_run_aim`]): both weave around an
/// enemy planted on the line home the exact same way. `perp` is the run-home
/// `direction` rotated a quarter turn left, so a blocker on the left (positive
/// side) sends the aim right and vice versa; one dead on the line (side `0.0`)
/// swings left, a deterministic pick so the car never stalls head-on into it. How
/// far it swings out is the driver's commitment-flexed [`carrier_juke_offset`]: a
/// reckless car squeezes a tighter line, a disciplined one a wider berth.
fn juke_aim_around_blocker(
    from: Vec2,
    home: Vec2,
    direction: Vec2,
    blocker: Vec2,
    corner_throttle: f32,
) -> Vec2 {
    let perp = Vec2::new(-direction.y, direction.x);
    let blocker_side = (blocker - from).dot(perp);
    let juke = if blocker_side > 0.0 { -1.0 } else { 1.0 };
    home + perp * juke * carrier_juke_offset(corner_throttle)
}

/// The aim a battered car drives at on its pit retreat home.
///
/// The survival mirror of [`carrier_home_run_aim`]: a team ground down to
/// [`PIT_RETREAT_INTEGRITY_FRACTION`] sends its home-most car back to recover, and
/// rather than limp straight into the enemy that battered it, the car weaves
/// around a foe planted on its run home, arcing out by [`CARRIER_JUKE_OFFSET`] to
/// dodge a ram it can least afford while already on the ropes. The classic Death
/// Rally "shake the tail on the limp home".
///
/// It reuses the carrier's lane-blocker read ([`nearest_lane_blocker`]) and juke
/// geometry ([`juke_aim_around_blocker`]) verbatim, so a retreating car judges "in
/// my way home" and swerves exactly as a carrier does. The only difference is
/// where it straightens up: inside [`PIT_RETREAT_HOME_COMMIT_DISTANCE`] it commits
/// straight into its base recovery zone, so the final approach is always a clean
/// park in the pit. Recomputed every frame, so the aim snaps back to base the
/// moment the blocker clears the lane.
#[must_use]
pub fn pit_retreat_home_run_aim(
    position: Vec2,
    home: Vec2,
    team: AiTeam,
    threats: &[ThreatTarget],
    corner_throttle: f32,
) -> Vec2 {
    let to_home = home - position;
    if to_home.length_squared()
        <= PIT_RETREAT_HOME_COMMIT_DISTANCE * PIT_RETREAT_HOME_COMMIT_DISTANCE
    {
        return home;
    }

    let Some(direction) = to_home.try_normalize() else {
        return home;
    };

    let Some(blocker) = nearest_lane_blocker(position, home, team, threats) else {
        return home;
    };

    juke_aim_around_blocker(position, home, direction, blocker, corner_throttle)
}

/// Nearest enemy planted on a flag carrier's straight line home, between it and
/// the base, or `None` when the run home is clear.
///
/// Reuses the same forward-of and on-lane tests the CTF pickup detour uses, so a
/// carrier judges "in my way home" exactly as it judges "on my flag lane".
fn nearest_lane_blocker(
    carrier: Vec2,
    home: Vec2,
    team: AiTeam,
    threats: &[ThreatTarget],
) -> Option<Vec2> {
    let home_distance_sq = carrier.distance_squared(home);
    threats
        .iter()
        .filter(|threat| threat.team == team.enemy())
        .map(|threat| threat.position)
        .filter(|&position| {
            is_ahead_of_target_push(carrier, position, home)
                && carrier.distance_squared(position) < home_distance_sq
                && is_on_target_lane(carrier, position, home, CARRIER_JUKE_LANE_WIDTH)
        })
        .min_by(|a, b| {
            carrier
                .distance_squared(*a)
                .partial_cmp(&carrier.distance_squared(*b))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| compare_positions(*a, *b))
        })
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
    let min_priority = closing_time_detour_min_priority(
        choices.closing_time_discipline,
        choices.pickup_pursuit_radius,
    );
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
                    pickup_lane_width(pickup.priority, choices.pickup_pursuit_radius),
                )
        },
    )
}

/// Minimum pickup priority that still justifies a CTF detour this frame, with the
/// closing-time bar nudged by the driver's greed.
///
/// In normal play a car breaks off its objective for any pickup worth the base
/// [`CTF_PICKUP_DETOUR_MIN_PRIORITY`], regardless of personality. Once a team
/// disciplines its detours in closing time the bar rises to the wide
/// [`CTF_WIDE_DETOUR_MIN_PRIORITY`], so only a pickup already worth a wide gamble
/// (nitro's race pressure or a battered team's integrity-scaled repair/shield) is
/// worth the time, while a mere cash bag is left on the track in favour of racing
/// the flag home. On top of that the closing-time bar is nudged by the driver's
/// greed (its `pickup_pursuit_radius` relative to [`BASELINE_PICKUP_PURSUIT_RADIUS`]),
/// the same personality axis that already widens its detour lane: a greedy driver
/// keeps gambling on a cheaper grab ([`CLOSING_TIME_GREEDY_DETOUR_MIN_PRIORITY`]),
/// a disciplined one locks down even a nitro ([`CLOSING_TIME_DISCIPLINED_DETOUR_MIN_PRIORITY`]),
/// and the baseline driver keeps the exact wide bar.
const fn closing_time_detour_min_priority(
    closing_time_discipline: bool,
    pickup_pursuit_radius: f32,
) -> u32 {
    if !closing_time_discipline {
        return CTF_PICKUP_DETOUR_MIN_PRIORITY;
    }
    // Nudge the wide bar by the driver's greed relative to the baseline driver.
    // Stepped tiers keep the mapping legible and free of float-to-int casts,
    // mirroring `repair_priority_for_integrity`; the baseline driver (delta 0)
    // keeps the exact wide bar, so it and the human that mirrors it are unchanged.
    let greed_delta = pickup_pursuit_radius - BASELINE_PICKUP_PURSUIT_RADIUS;
    if greed_delta >= CLOSING_TIME_GREED_STEP {
        CLOSING_TIME_GREEDY_DETOUR_MIN_PRIORITY
    } else if greed_delta <= -CLOSING_TIME_GREED_STEP {
        CLOSING_TIME_DISCIPLINED_DETOUR_MIN_PRIORITY
    } else {
        CTF_WIDE_DETOUR_MIN_PRIORITY
    }
}

fn is_ahead_of_target_push(position: Vec2, pickup: Vec2, target: Vec2) -> bool {
    let to_pickup = pickup - position;
    let to_target = target - position;
    to_pickup.dot(to_target) > 0.0
}

/// Half-width of the CTF detour lane for a pickup of the given `priority`, scaled
/// by the driver's `pickup_pursuit_radius` (its greed) relative to
/// [`BASELINE_PICKUP_PURSUIT_RADIUS`].
///
/// The base lane widens for a high-value grab worth a wider gamble
/// ([`CTF_HIGH_VALUE_PICKUP_LANE_WIDTH`] vs [`CTF_PICKUP_LANE_WIDTH`]); on top of
/// that, a greedier driver swings further off its objective line for the same
/// pickup while a disciplined one keeps a tighter line, the in-objective mirror of
/// the same greed axis that sets how far afield a patrolling car scavenges.
fn pickup_lane_width(priority: u32, pickup_pursuit_radius: f32) -> f32 {
    let base = if priority >= CTF_WIDE_DETOUR_MIN_PRIORITY {
        CTF_HIGH_VALUE_PICKUP_LANE_WIDTH
    } else {
        CTF_PICKUP_LANE_WIDTH
    };
    base * greed_lane_scale(pickup_pursuit_radius)
}

/// The driver's detour-lane scale: its greed relative to the baseline driver,
/// clamped to the [`GREED_LANE_SCALE_MIN`]..=[`GREED_LANE_SCALE_MAX`] safety band
/// so a degenerate radius never yields an absurd lane.
fn greed_lane_scale(pickup_pursuit_radius: f32) -> f32 {
    (pickup_pursuit_radius / BASELINE_PICKUP_PURSUIT_RADIUS)
        .clamp(GREED_LANE_SCALE_MIN, GREED_LANE_SCALE_MAX)
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
///
/// `corner_throttle` is the driver's throttle floor: how hard it keeps the gas
/// down when the target is off to the side, i.e. its commitment through a corner.
/// A higher floor barrels through on a wider line, a lower one eases off for a
/// tighter one (see [`crate::gameplay::virtual_player::VirtualPlayer::corner_throttle`]).
/// The neutral baseline is [`MIN_THROTTLE`].
pub fn compute_steering(
    position: Vec2,
    forward: Vec2,
    target: Vec2,
    arrive_radius: f32,
    corner_throttle: f32,
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
            throttle: corner_throttle,
            steer: 0.0,
        };
    };

    // Signed angle from the car's heading to the target direction.
    // Positive => target is to the left (counter-clockwise).
    let angle = heading.perp_dot(dir).atan2(heading.dot(dir));
    let alignment = heading.dot(dir);

    if alignment < REVERSE_DOT_THRESHOLD {
        return reverse_steering_intent(angle, alignment, corner_throttle);
    }

    // Drive hardest when aligned, but never stall: a car cannot strafe, so it
    // must keep rolling to rotate towards a target that is to the side.
    let steer = (angle / STEER_RANGE).clamp(-1.0, 1.0);
    let throttle = alignment.clamp(corner_throttle, 1.0);

    SteeringIntent { throttle, steer }
}

fn reverse_steering_intent(angle: f32, alignment: f32, corner_throttle: f32) -> SteeringIntent {
    let reverse_angle = if angle >= 0.0 {
        angle - std::f32::consts::PI
    } else {
        angle + std::f32::consts::PI
    };

    SteeringIntent {
        throttle: alignment.clamp(-1.0, -corner_throttle),
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
}
