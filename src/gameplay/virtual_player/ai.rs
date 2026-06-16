use crate::gameplay::slipstream::{
    DRAFT_SEEK_DISCIPLINED_MIN_AIM_COURSE_DOT, DRAFT_SEEK_GREEDY_MIN_AIM_COURSE_DOT,
    DRAFT_SEEK_MIN_AIM_COURSE_DOT,
};
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

/// Greed gap from the baseline driver at which a driver's active-drafting cone steps
/// out to the greedy (or in to the disciplined) tier. Shares
/// [`CLOSING_TIME_GREED_STEP`] so the same roster reads greedy or disciplined the
/// same way across both greed levers.
const DRAFT_CONE_GREED_STEP: f32 = CLOSING_TIME_GREED_STEP;

/// The draft-cone greed step must be a real gap, enforced at compile time.
const _: () = assert!(DRAFT_CONE_GREED_STEP > 0.0);

/// The active-drafting deflection cone a driver of the given greed uses: the minimum
/// dot between a wake-seek nudge and the straight course to the objective for the
/// nudge to stand (a smaller dot is a wider cone). Handed to
/// [`crate::gameplay::slipstream::draft_seeking_aim`].
///
/// The off-objective-line mirror of [`pickup_lane_width`]: the same greed axis
/// (`pickup_pursuit_radius` relative to [`BASELINE_PICKUP_PURSUIT_RADIUS`]) that lets
/// a greedy driver swing wider off its line for a pickup also lets it swing wider to
/// tuck into a tow, while a disciplined one keeps the straightest line. Stepped tiers
/// keep the mapping legible and free of float-to-int casts, mirroring
/// [`closing_time_detour_min_priority`]; the baseline driver (delta 0, and the human
/// that mirrors it) keeps the exact baseline cone, so its drafting is unchanged.
pub const fn draft_seek_cone(pickup_pursuit_radius: f32) -> f32 {
    let greed_delta = pickup_pursuit_radius - BASELINE_PICKUP_PURSUIT_RADIUS;
    if greed_delta >= DRAFT_CONE_GREED_STEP {
        DRAFT_SEEK_GREEDY_MIN_AIM_COURSE_DOT
    } else if greed_delta <= -DRAFT_CONE_GREED_STEP {
        DRAFT_SEEK_DISCIPLINED_MIN_AIM_COURSE_DOT
    } else {
        DRAFT_SEEK_MIN_AIM_COURSE_DOT
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
mod tests;
