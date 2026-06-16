//! Slipstreaming (drafting): the classic racing tow a car earns by tucking into
//! the wake of a car ahead.
//!
//! A staple of the Death Rally feel: sit on a rival's tail down a straight and
//! the hole it punches in the air slingshots you past. Modelled here as a small,
//! capped speed bonus a car receives while it is closely following a car ahead,
//! travelling the same way. The bonus is read by both movement systems, the human's
//! `car_movement_system` and the field's `virtual_player_drive_system`, so the human
//! and the AI draft on the identical terms.
//!
//! A flag carrier never catches a slipstream: the bulky flag spoils the tow. That
//! is flavour, but it is also what keeps the mechanic balance-safe. Slipstream can
//! only ever speed a *non-carrier*, so it can never let a flag run outpace the
//! field; the tuned "even the slowest chaser outpaces the fastest carrier" chase
//! balance is left fully intact, and the tow only ever *helps* a chaser close on a
//! carrier.

use bevy::prelude::*;

/// Largest fraction a perfect slipstream adds to a car's speed.
///
/// A modest tow, not a power item: pitched well below a nitro burst's raw boost
/// so drafting is an edge a clean racing line earns, never a substitute for a
/// pickup. The bonus scales down with the gap (see [`slipstream_speed_multiplier`]),
/// so this top rate is reached only nose-to-tail.
pub const DRAFT_MAX_SPEED_BONUS: f32 = 0.12;

/// A draft must be a real tow yet stay a modest edge, never a nitro-grade boost,
/// enforced at compile time so the tow can never drift into a power item.
const _: () = assert!(DRAFT_MAX_SPEED_BONUS > 0.0 && DRAFT_MAX_SPEED_BONUS < 0.25);

/// Furthest a car can sit behind a leader and still catch its wake.
///
/// Anchored a clear step beyond ram range ([`crate::gameplay::combat::RAM_RADIUS`])
/// so a car drafts from a couple of lengths back, before it ever closes into
/// trading paint, and the tow fades to nothing by this distance.
pub const DRAFT_RADIUS: f32 = 280.0;

/// A car must be able to catch a wake from beyond ramming range, enforced at
/// compile time, so drafting is a following manoeuvre rather than a side effect of
/// a collision.
const _: () = assert!(DRAFT_RADIUS > crate::gameplay::combat::RAM_RADIUS);

/// Half-width of the wake lane: how far off a leader's tail a follower may sit and
/// still draft.
///
/// Kept tight so only a car genuinely tucked in behind a leader earns the tow, not
/// one merely alongside or cutting across. A follower drifts out of the slipstream
/// the moment it pulls level.
pub const DRAFT_LANE_HALF_WIDTH: f32 = 70.0;

/// The wake lane must be a real but narrow corridor, enforced at compile time, so a
/// draft demands a tidy tow line rather than rewarding any nearby car.
const _: () = assert!(DRAFT_LANE_HALF_WIDTH > 0.0 && DRAFT_LANE_HALF_WIDTH < DRAFT_RADIUS);

/// Smallest heading agreement (dot product of unit headings) between a follower and
/// its leader for a draft to count.
///
/// At `0.5` the two must be travelling within sixty degrees of the same direction,
/// so a car only drafts a leader genuinely running its way: an oncoming or a
/// crossing car punches no usable hole in the air and grants no tow. The tow does
/// not snap on at this gate but fades in from it: a leader yawed right at the gate
/// lends nothing and the wake builds to full only as the two line up squarely (see
/// the alignment falloff in [`slipstream_speed_multiplier`]).
pub const DRAFT_MIN_ALIGNMENT: f32 = 0.5;

/// The alignment gate must be a real heading test, never trivially open or shut,
/// enforced at compile time.
const _: () = assert!(DRAFT_MIN_ALIGNMENT > 0.0 && DRAFT_MIN_ALIGNMENT < 1.0);

/// How far up a leader's centre tail line, toward the leader, a wake-seeking chaser
/// aims as it tucks in behind to draft.
///
/// The active counterpart to the passive tow [`slipstream_speed_multiplier`] hands
/// a car already in the wake: a chaser behind a leader running its way aims not at
/// the leader itself but at a point on the leader's tail line this far ahead of
/// where the chaser currently sits along that line, so the single aim both pulls
/// the chaser sideways onto the wake line and draws it forward to close the gap.
/// Anchored a touch over ram range so a chaser tucks right in close, where the tow
/// is strongest, rather than hanging back at the fringe of the draft. The aim is
/// capped so it never reaches past the leader's own centre (see
/// [`draft_seeking_aim`]), so seeking only ever tucks a chaser *in behind*.
pub const DRAFT_SEEK_LOOKAHEAD: f32 = 150.0;

/// The seek lookahead must draw a chaser forward yet stay within the wake's reach,
/// enforced at compile time, so seeking is a tuck-in-behind rather than a lunge
/// past the leader.
const _: () = assert!(DRAFT_SEEK_LOOKAHEAD > 0.0 && DRAFT_SEEK_LOOKAHEAD < DRAFT_RADIUS);

/// Tightest heading agreement (dot of unit directions) between a wake-seeking
/// chaser's nudged aim and the straight bearing to its objective for the nudge to
/// stand.
///
/// The safety rail on active drafting: tucking into a wake must never bend a chaser
/// meaningfully off the line it would otherwise drive, so a seek aim that would
/// deflect its bearing past this gate is dropped and the chaser commits straight to
/// its objective. At `0.70` the nudge can never swing the aim more than about
/// forty-five degrees off the objective bearing, so a draft stays an edge a chaser
/// takes on its way, never a detour that pulls it off task.
const DRAFT_SEEK_MIN_AIM_COURSE_DOT: f32 = 0.70;

/// The seek deflection gate must be a real cone, never trivially open or shut, and
/// never looser than the wake's own alignment gate, enforced at compile time.
const _: () = assert!(
    DRAFT_SEEK_MIN_AIM_COURSE_DOT > DRAFT_MIN_ALIGNMENT && DRAFT_SEEK_MIN_AIM_COURSE_DOT < 1.0
);

/// Smallest distance the objective must lie beyond a pace car, measured along the
/// chaser's course, for the chaser to bother tucking into that car's wake.
///
/// Active drafting trades a sliver of forward progress, aiming onto the leader's
/// tail line rather than straight at the objective, for the tow. That trade only
/// pays when a real run still lies ahead: a car sitting *at* the objective (the car
/// a chaser is running straight down, or a teammate already parked on the base) is
/// no pace car, and tucking in behind it would only aim short of the goal and delay
/// the arrival. Pinned to a full wake reach [`DRAFT_RADIUS`] so a chaser only drafts
/// a leader the objective clearly sits beyond; since a wake is catchable only within
/// [`DRAFT_RADIUS`] anyway, the objective is then at least as far off as the deepest
/// wake a chaser could ever ride. This is what keeps the same routine safe whether
/// the aim is a distant base or the very car a chaser is hunting down: with nothing
/// beyond the hunted car, its wake is never sought and the chase line is untouched.
const DRAFT_SEEK_MIN_OBJECTIVE_LEAD: f32 = DRAFT_RADIUS;

/// A car ahead whose wake a follower might catch: its world position and the unit
/// (or near-unit) direction it is travelling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeadingCar {
    pub position: Vec2,
    pub heading: Vec2,
}

/// Speed multiplier a car at `position` heading `heading` earns from slipstreaming
/// the closest qualifying car ahead of it.
///
/// Returns `1.0` (no tow) unless a `leader` is genuinely in the follower's wake
/// corridor: ahead of it, within [`DRAFT_LANE_HALF_WIDTH`] of its tail line, no
/// further than [`DRAFT_RADIUS`], and travelling within [`DRAFT_MIN_ALIGNMENT`] of
/// the same direction. The tow scales with three things: how close the gap is
/// (full nose-to-tail, fading to nothing at [`DRAFT_RADIUS`]), how centred the
/// follower sits in the lane (full dead on the tail line, fading to nothing at the
/// lane edge), and how squarely the two run the same way (full heading-aligned,
/// fading to nothing as the leader yaws out to [`DRAFT_MIN_ALIGNMENT`]), so the
/// full [`DRAFT_MAX_SPEED_BONUS`] is earned only by tucking in close, on a tidy
/// line, squarely behind; when several cars qualify, the strongest wake wins.
///
/// The caller passes only *other* cars (it excludes the follower itself) and omits
/// any car that should not receive a tow (a flag carrier). A degenerate heading
/// yields no draft, so the result is always in `1.0..=1.0 + DRAFT_MAX_SPEED_BONUS`.
#[must_use]
pub fn slipstream_speed_multiplier(position: Vec2, heading: Vec2, leaders: &[LeadingCar]) -> f32 {
    let Some(heading) = heading.try_normalize() else {
        return 1.0;
    };
    let strength = leaders
        .iter()
        .filter_map(|leader| draft_strength(position, heading, *leader))
        .fold(0.0_f32, f32::max);
    DRAFT_MAX_SPEED_BONUS.mul_add(strength, 1.0)
}

/// Wake strength (`0.0`..=`1.0`) a single `leader` lends a follower at `position`
/// heading the unit `heading`, or `None` when the leader is not in the wake
/// corridor at all.
///
/// The corridor is a narrow forward channel: the leader must sit ahead of the
/// follower, no more than [`DRAFT_LANE_HALF_WIDTH`] off its tail line, within
/// [`DRAFT_RADIUS`], and travelling within [`DRAFT_MIN_ALIGNMENT`] of the same
/// heading. Strength is the product of three linear falloffs: how close the gap is
/// (full nose-to-tail, nothing at [`DRAFT_RADIUS`]), how centred the follower sits
/// in the lane (full dead on the tail line, nothing at the lane edge), and how
/// squarely the two run the same way (full heading-aligned, nothing as the leader
/// yaws out to the [`DRAFT_MIN_ALIGNMENT`] gate). A car must therefore tuck in
/// close, hold a tidy line *and* sit squarely behind to win the strongest tow, and
/// the wake fades smoothly to nothing at each edge of the corridor rather than
/// cutting off at a cliff.
fn draft_strength(position: Vec2, heading: Vec2, leader: LeadingCar) -> Option<f32> {
    let to_leader = leader.position - position;
    let ahead = to_leader.dot(heading);
    if ahead <= 0.0 {
        return None;
    }
    let lateral = heading.perp_dot(to_leader).abs();
    if lateral > DRAFT_LANE_HALF_WIDTH {
        return None;
    }
    let distance = to_leader.length();
    if distance > DRAFT_RADIUS {
        return None;
    }
    let leader_heading = leader.heading.try_normalize()?;
    let alignment = leader_heading.dot(heading);
    if alignment < DRAFT_MIN_ALIGNMENT {
        return None;
    }
    let gap_falloff = (1.0 - distance / DRAFT_RADIUS).clamp(0.0, 1.0);
    let lane_centring = (1.0 - lateral / DRAFT_LANE_HALF_WIDTH).clamp(0.0, 1.0);
    let alignment_centring =
        ((alignment - DRAFT_MIN_ALIGNMENT) / (1.0 - DRAFT_MIN_ALIGNMENT)).clamp(0.0, 1.0);
    Some(gap_falloff * lane_centring * alignment_centring)
}

/// Aim point that steers a chaser at `position`, bound for `base_aim`, onto the
/// tail line of the best leader running its way, so it actively tucks into the
/// wake and is towed toward its objective.
///
/// The active counterpart to [`slipstream_speed_multiplier`], which only rewards a
/// car already sitting in a wake: this is what makes a virtual driver *seek* one.
/// It returns `base_aim` unchanged unless a leader genuinely lies in front to be
/// drafted, namely one travelling within [`DRAFT_MIN_ALIGNMENT`] of the chaser's
/// bearing to `base_aim` (so tucking in behind it carries the chaser the way it
/// already wants to go), sitting ahead of the chaser along its own heading, and no
/// further back than [`DRAFT_RADIUS`] (within the wake's reach). When one
/// qualifies, the aim is drawn onto that leader's centre tail line a
/// [`DRAFT_SEEK_LOOKAHEAD`] step ahead of where the chaser sits along it, capped so
/// it never reaches past the leader's own centre, so the single nudge pulls the
/// chaser sideways onto the wake line and forward to close the gap without ever
/// overrunning the car it drafts. The nudge is dropped (and `base_aim` returned)
/// whenever it would swing the chaser's bearing past the
/// [`DRAFT_SEEK_MIN_AIM_COURSE_DOT`] cone off its objective, so a draft never pulls
/// a chaser off task. The caller passes only *other* cars (it excludes the chaser
/// itself) and omits any car that should not seek a draft (a flag carrier earns no
/// tow). When several leaders qualify, the closest, best-aligned wake wins.
#[must_use]
pub fn draft_seeking_aim(position: Vec2, base_aim: Vec2, leaders: &[LeadingCar]) -> Vec2 {
    let to_objective = base_aim - position;
    let Some(course) = to_objective.try_normalize() else {
        return base_aim;
    };
    let objective_along = to_objective.length();
    leaders
        .iter()
        .filter_map(|leader| draft_seek_aim(position, course, objective_along, *leader))
        .max_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.x.total_cmp(&b.0.x).then(a.0.y.total_cmp(&b.0.y)))
        })
        .map_or(base_aim, |(aim, _)| aim)
}

/// The tuck-in aim a single `leader` offers a chaser at `position` holding the unit
/// bearing `course` to its objective, paired with how strong a seek it is
/// (`0.0`..=`1.0`, higher being a closer, better-aligned wake), or `None` when the
/// leader is not one worth drafting.
///
/// A leader qualifies only when it runs within [`DRAFT_MIN_ALIGNMENT`] of the
/// chaser's course (so drafting it carries the chaser its way), sits ahead of the
/// chaser along its own heading, lies no further back than [`DRAFT_RADIUS`], has the
/// objective at least [`DRAFT_SEEK_MIN_OBJECTIVE_LEAD`] further along the course than
/// itself (a real run still to make, never a car parked on the goal), and the
/// resulting tuck-in aim stays inside the [`DRAFT_SEEK_MIN_AIM_COURSE_DOT`]
/// deflection cone off the course. `objective_along` is the chaser's straight-line
/// distance to its objective along `course`. The aim is a point on the leader's
/// centre tail line, a [`DRAFT_SEEK_LOOKAHEAD`] step nearer the leader than the
/// chaser's own along-line position but never past the leader's centre.
fn draft_seek_aim(
    position: Vec2,
    course: Vec2,
    objective_along: f32,
    leader: LeadingCar,
) -> Option<(Vec2, f32)> {
    let leader_heading = leader.heading.try_normalize()?;
    let alignment = leader_heading.dot(course);
    if alignment < DRAFT_MIN_ALIGNMENT {
        return None;
    }
    // Negative when the chaser sits behind the leader along its heading: the only
    // half-plane from which a wake can be caught.
    let along = leader_heading.dot(position - leader.position);
    if along >= 0.0 {
        return None;
    }
    let gap = -along;
    if gap > DRAFT_RADIUS {
        return None;
    }
    // Only draft a pace car the objective clearly lies beyond: a car at or near the
    // goal is no stepping stone, and tucking in behind it would only aim short.
    if objective_along - (leader.position - position).dot(course) < DRAFT_SEEK_MIN_OBJECTIVE_LEAD {
        return None;
    }
    let aim = leader.position + leader_heading * (along + DRAFT_SEEK_LOOKAHEAD).min(0.0);
    let aim_dir = (aim - position).try_normalize()?;
    if aim_dir.dot(course) < DRAFT_SEEK_MIN_AIM_COURSE_DOT {
        return None;
    }
    let proximity = 1.0 - gap / DRAFT_RADIUS;
    Some((aim, proximity * alignment))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A leader directly ahead, travelling the same way, at a given gap.
    fn leader_ahead(gap: f32) -> LeadingCar {
        LeadingCar {
            position: Vec2::new(0.0, gap),
            heading: Vec2::Y,
        }
    }

    fn assert_near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= f32::EPSILON,
            "actual={actual}, expected={expected}"
        );
    }

    #[test]
    fn no_leaders_gives_no_tow() {
        assert_near(slipstream_speed_multiplier(Vec2::ZERO, Vec2::Y, &[]), 1.0);
    }

    #[test]
    fn a_car_tucked_in_behind_a_leader_earns_a_tow() {
        let multiplier = slipstream_speed_multiplier(Vec2::ZERO, Vec2::Y, &[leader_ahead(150.0)]);
        assert!(
            multiplier > 1.0 && multiplier <= 1.0 + DRAFT_MAX_SPEED_BONUS,
            "expected a capped tow, got {multiplier}"
        );
    }

    #[test]
    fn the_tow_strengthens_as_the_gap_closes() {
        let far = slipstream_speed_multiplier(Vec2::ZERO, Vec2::Y, &[leader_ahead(250.0)]);
        let near = slipstream_speed_multiplier(Vec2::ZERO, Vec2::Y, &[leader_ahead(80.0)]);
        assert!(
            near > far,
            "a closer wake should tow harder: near={near}, far={far}"
        );
    }

    #[test]
    fn a_leader_off_to_the_side_grants_no_tow() {
        let beside = LeadingCar {
            position: Vec2::new(DRAFT_LANE_HALF_WIDTH + 20.0, 100.0),
            heading: Vec2::Y,
        };
        assert_near(
            slipstream_speed_multiplier(Vec2::ZERO, Vec2::Y, &[beside]),
            1.0,
        );
    }

    #[test]
    fn a_leader_beyond_the_wake_reach_grants_no_tow() {
        let distant = leader_ahead(DRAFT_RADIUS + 20.0);
        assert_near(
            slipstream_speed_multiplier(Vec2::ZERO, Vec2::Y, &[distant]),
            1.0,
        );
    }

    #[test]
    fn a_leader_behind_grants_no_tow() {
        let behind = LeadingCar {
            position: Vec2::new(0.0, -120.0),
            heading: Vec2::Y,
        };
        assert_near(
            slipstream_speed_multiplier(Vec2::ZERO, Vec2::Y, &[behind]),
            1.0,
        );
    }

    #[test]
    fn an_oncoming_leader_grants_no_tow() {
        let oncoming = LeadingCar {
            position: Vec2::new(0.0, 120.0),
            heading: Vec2::NEG_Y,
        };
        assert_near(
            slipstream_speed_multiplier(Vec2::ZERO, Vec2::Y, &[oncoming]),
            1.0,
        );
    }

    #[test]
    fn a_crossing_leader_grants_no_tow() {
        let crossing = LeadingCar {
            position: Vec2::new(0.0, 120.0),
            heading: Vec2::X,
        };
        assert_near(
            slipstream_speed_multiplier(Vec2::ZERO, Vec2::Y, &[crossing]),
            1.0,
        );
    }

    #[test]
    fn the_strongest_wake_wins() {
        let leaders = [leader_ahead(250.0), leader_ahead(90.0), leader_ahead(200.0)];
        let best = slipstream_speed_multiplier(Vec2::ZERO, Vec2::Y, &leaders);
        let closest_only = slipstream_speed_multiplier(Vec2::ZERO, Vec2::Y, &[leader_ahead(90.0)]);
        assert!(
            (best - closest_only).abs() <= f32::EPSILON,
            "the closest wake should set the tow: best={best}, closest_only={closest_only}"
        );
    }

    #[test]
    fn the_tow_is_capped_nose_to_tail() {
        let nose_to_tail = slipstream_speed_multiplier(Vec2::ZERO, Vec2::Y, &[leader_ahead(0.5)]);
        assert!(
            nose_to_tail <= 1.0 + DRAFT_MAX_SPEED_BONUS + f32::EPSILON,
            "the tow must never exceed the cap, got {nose_to_tail}"
        );
    }

    #[test]
    fn a_degenerate_heading_grants_no_tow() {
        assert_near(
            slipstream_speed_multiplier(Vec2::ZERO, Vec2::ZERO, &[leader_ahead(100.0)]),
            1.0,
        );
    }

    /// A leader sitting at straight-line distance `distance`, offset `lateral`
    /// units off the follower's tail line, travelling the same way. Solving for the
    /// forward component keeps the straight-line gap fixed, so only the lane offset
    /// changes between fixtures.
    fn leader_off_centre(lateral: f32, distance: f32) -> LeadingCar {
        let forward = distance.mul_add(distance, -(lateral * lateral)).sqrt();
        LeadingCar {
            position: Vec2::new(lateral, forward),
            heading: Vec2::Y,
        }
    }

    #[test]
    fn a_centred_follower_out_tows_an_off_centre_one_at_the_same_gap() {
        // Both leaders sit at the identical straight-line gap, so the distance
        // falloff is the same for each; only the lane offset differs. A tidy,
        // dead-centre tow line must earn a stronger wake than one drifting toward
        // the edge of the lane.
        let distance = 120.0;
        let centred =
            slipstream_speed_multiplier(Vec2::ZERO, Vec2::Y, &[leader_off_centre(0.0, distance)]);
        let off_centre = slipstream_speed_multiplier(
            Vec2::ZERO,
            Vec2::Y,
            &[leader_off_centre(DRAFT_LANE_HALF_WIDTH * 0.6, distance)],
        );
        assert!(
            centred > off_centre,
            "a centred tow line should out-tow an off-centre one at the same gap: \
             centred={centred}, off_centre={off_centre}"
        );
        assert!(
            off_centre > 1.0,
            "a follower still inside the lane should keep some tow: {off_centre}"
        );
    }

    #[test]
    fn the_tow_fades_smoothly_to_nothing_at_the_lane_edge() {
        // The wake must fade to nothing as a follower drifts to the edge of the
        // lane, so crossing the boundary is continuous rather than a cliff where a
        // unit of drift kills a near-full tow outright.
        let distance = 100.0;
        let just_inside = slipstream_speed_multiplier(
            Vec2::ZERO,
            Vec2::Y,
            &[leader_off_centre(DRAFT_LANE_HALF_WIDTH - 1.0, distance)],
        );
        let just_outside = slipstream_speed_multiplier(
            Vec2::ZERO,
            Vec2::Y,
            &[leader_off_centre(DRAFT_LANE_HALF_WIDTH + 1.0, distance)],
        );
        assert_near(just_outside, 1.0);
        assert!(
            just_inside >= just_outside && just_inside - just_outside < 0.01,
            "the tow must fade to nothing at the lane edge, no cliff: \
             inside={just_inside}, outside={just_outside}"
        );
    }

    #[test]
    fn dead_centre_nose_to_tail_still_earns_the_full_cap() {
        // The lane-centring scaling must leave the common case untouched: a
        // follower tucked dead-centre on a leader's tail still earns the full tow.
        let full = slipstream_speed_multiplier(Vec2::ZERO, Vec2::Y, &[leader_ahead(0.5)]);
        assert!(
            (full - (1.0 + DRAFT_MAX_SPEED_BONUS)).abs() < 0.01,
            "a dead-centre nose-to-tail follower should still earn the full cap: {full}"
        );
    }

    /// A leader directly ahead at straight-line `gap`, but travelling at `degrees`
    /// away from the follower's heading (yawed toward +X), so only the heading
    /// agreement differs from a square nose-to-tail tow.
    fn leader_ahead_yawed(gap: f32, degrees: f32) -> LeadingCar {
        let theta = degrees.to_radians();
        LeadingCar {
            position: Vec2::new(0.0, gap),
            heading: Vec2::new(theta.sin(), theta.cos()),
        }
    }

    #[test]
    fn a_squarely_aligned_follower_out_tows_a_yawed_one_at_the_same_gap() {
        // Both leaders sit dead-centre at the identical gap, so the gap and lane
        // falloffs match; only their heading agreement differs. A leader running
        // squarely the follower's way punches a cleaner hole in the air than one
        // peeling off at an angle, so the square tow must be the stronger.
        let gap = 120.0;
        let square =
            slipstream_speed_multiplier(Vec2::ZERO, Vec2::Y, &[leader_ahead_yawed(gap, 0.0)]);
        let yawed =
            slipstream_speed_multiplier(Vec2::ZERO, Vec2::Y, &[leader_ahead_yawed(gap, 45.0)]);
        assert!(
            square > yawed,
            "a squarely aligned tow should beat a yawed one at the same gap: \
             square={square}, yawed={yawed}"
        );
        assert!(
            yawed > 1.0,
            "a leader still inside the alignment gate should keep some tow: {yawed}"
        );
    }

    #[test]
    fn the_tow_fades_smoothly_to_nothing_at_the_alignment_gate() {
        // The wake must fade to nothing as a leader's heading swings out to the
        // alignment gate, so crossing the boundary is continuous rather than a
        // cliff where a degree of yaw kills a near-full tow outright, mirroring how
        // the lane-centring falloff fades to nothing at the lane edge.
        let gap = 100.0;
        let gate_degrees = DRAFT_MIN_ALIGNMENT.acos().to_degrees();
        let just_inside = slipstream_speed_multiplier(
            Vec2::ZERO,
            Vec2::Y,
            &[leader_ahead_yawed(gap, gate_degrees - 1.0)],
        );
        let just_outside = slipstream_speed_multiplier(
            Vec2::ZERO,
            Vec2::Y,
            &[leader_ahead_yawed(gap, gate_degrees + 1.0)],
        );
        assert_near(just_outside, 1.0);
        assert!(
            just_inside >= just_outside && just_inside - just_outside < 0.01,
            "the tow must fade to nothing at the alignment gate, no cliff: \
             inside={just_inside}, outside={just_outside}"
        );
    }

    /// A leader at `position` travelling straight up the arena (`+Y`), the wake a
    /// chaser tries to tuck into.
    fn aligned_leader(position: Vec2) -> LeadingCar {
        LeadingCar {
            position,
            heading: Vec2::Y,
        }
    }

    fn assert_vec_near(actual: Vec2, expected: Vec2) {
        assert!(
            actual.distance(expected) <= 1e-3,
            "actual={actual:?}, expected={expected:?}"
        );
    }

    #[test]
    fn with_no_leaders_a_seeker_keeps_its_base_aim() {
        let base = Vec2::new(40.0, 500.0);
        assert_vec_near(draft_seeking_aim(Vec2::ZERO, base, &[]), base);
    }

    #[test]
    fn a_degenerate_course_keeps_the_base_aim() {
        let here = Vec2::new(10.0, 10.0);
        assert_vec_near(
            draft_seeking_aim(here, here, &[aligned_leader(Vec2::new(0.0, 200.0))]),
            here,
        );
    }

    #[test]
    fn a_chaser_behind_an_aligned_leader_is_drawn_onto_its_tail_line() {
        // The chaser drives straight up `x = 60`, off the leader's `x = 0` tail
        // line. The seek aim must pull it sideways onto that line so it tucks into
        // the wake, while still drawing it forward toward the leader.
        let aim = draft_seeking_aim(
            Vec2::new(60.0, 0.0),
            Vec2::new(60.0, 600.0),
            &[aligned_leader(Vec2::new(0.0, 200.0))],
        );
        assert_vec_near(aim, Vec2::new(0.0, 150.0));
    }

    #[test]
    fn a_leader_crossing_the_course_is_not_drafted() {
        let base = Vec2::new(0.0, 500.0);
        let crossing = LeadingCar {
            position: Vec2::new(0.0, 150.0),
            heading: Vec2::X,
        };
        assert_vec_near(draft_seeking_aim(Vec2::ZERO, base, &[crossing]), base);
    }

    #[test]
    fn a_leader_the_chaser_is_ahead_of_is_not_drafted() {
        let base = Vec2::new(0.0, 500.0);
        assert_vec_near(
            draft_seeking_aim(Vec2::ZERO, base, &[aligned_leader(Vec2::new(0.0, -100.0))]),
            base,
        );
    }

    #[test]
    fn a_leader_beyond_the_wake_reach_is_not_drafted() {
        let base = Vec2::new(0.0, 800.0);
        let distant = aligned_leader(Vec2::new(0.0, DRAFT_RADIUS + 40.0));
        assert_vec_near(draft_seeking_aim(Vec2::ZERO, base, &[distant]), base);
    }

    #[test]
    fn a_leader_off_to_the_side_never_deflects_the_aim_past_the_cone() {
        // A leader sitting far off to the flank is behind-qualifying, but tucking in
        // behind it would swing the chaser hard off its objective bearing. The cone
        // rail must drop the nudge so the chaser commits straight to its objective.
        let base = Vec2::new(0.0, 500.0);
        let off_to_side = aligned_leader(Vec2::new(300.0, 40.0));
        assert_vec_near(draft_seeking_aim(Vec2::ZERO, base, &[off_to_side]), base);
    }

    #[test]
    fn a_drafted_aim_stays_within_the_deflection_cone() {
        let position = Vec2::new(60.0, 0.0);
        let base = Vec2::new(60.0, 600.0);
        let aim = draft_seeking_aim(position, base, &[aligned_leader(Vec2::new(0.0, 200.0))]);
        let course = (base - position).normalize();
        let aim_dir = (aim - position).normalize();
        assert!(
            aim_dir.dot(course) >= DRAFT_SEEK_MIN_AIM_COURSE_DOT,
            "a drafted aim must never deflect past the cone: dot={}",
            aim_dir.dot(course)
        );
    }

    #[test]
    fn the_seek_aim_never_overruns_the_leader() {
        // The chaser is close enough that an uncapped lookahead would aim past the
        // leader's centre; the cap must pin the aim to the leader itself so seeking
        // only ever tucks in behind, never lunges through.
        let leader_pos = Vec2::new(0.0, 100.0);
        let aim = draft_seeking_aim(
            Vec2::new(10.0, 0.0),
            Vec2::new(0.0, 500.0),
            &[aligned_leader(leader_pos)],
        );
        assert_vec_near(aim, leader_pos);
    }

    #[test]
    fn the_closest_aligned_wake_wins() {
        let aim = draft_seeking_aim(
            Vec2::new(5.0, 0.0),
            Vec2::new(0.0, 600.0),
            &[
                aligned_leader(Vec2::new(0.0, 250.0)),
                aligned_leader(Vec2::new(0.0, 90.0)),
            ],
        );
        assert_vec_near(aim, Vec2::new(0.0, 90.0));
    }

    #[test]
    fn a_leader_parked_on_the_objective_is_not_drafted() {
        // The leader sits right on the objective, so there is no run left to draft
        // toward; tucking in behind it would only aim short of the goal. This is what
        // keeps a chaser running a car straight down from ever drafting it.
        let objective = Vec2::new(0.0, 200.0);
        assert_vec_near(
            draft_seeking_aim(
                Vec2::new(40.0, 0.0),
                objective,
                &[aligned_leader(objective)],
            ),
            objective,
        );
    }

    #[test]
    fn a_leader_just_short_of_the_objective_is_not_drafted() {
        // The objective is only a little beyond the leader, less than a wake reach,
        // so the forward progress traded for the tuck-in never pays: no draft.
        let base = Vec2::new(0.0, 360.0);
        assert_vec_near(
            draft_seeking_aim(
                Vec2::new(40.0, 0.0),
                base,
                &[aligned_leader(Vec2::new(0.0, 200.0))],
            ),
            base,
        );
    }

    #[test]
    fn actively_seeking_a_wake_reaches_an_objective_sooner_than_a_naive_line() {
        use crate::gameplay::virtual_player::ai::compute_steering;

        const PACE_SPEED: f32 = 9.3;
        const BASE_SPEED: f32 = 9.0;
        const CORNER_THROTTLE: f32 = 0.6;
        const ROT: f32 = 0.18;
        const FRAMES: usize = 400;

        let objective = Vec2::new(0.0, 6000.0);
        let pace_heading = Vec2::Y;
        let mut pace_pos = Vec2::new(50.0, 200.0);

        let (mut naive_pos, mut seek_pos) = (Vec2::ZERO, Vec2::ZERO);
        let (mut naive_fwd, mut seek_fwd) = (Vec2::Y, Vec2::Y);
        let (mut naive_tow, mut seek_tow) = (0.0_f32, 0.0_f32);

        for _ in 0..FRAMES {
            let pace = aligned_leader(pace_pos);

            let n_intent = compute_steering(naive_pos, naive_fwd, objective, 0.0, CORNER_THROTTLE);
            naive_fwd = Vec2::from_angle(n_intent.steer * ROT)
                .rotate(naive_fwd)
                .normalize_or_zero();
            let n_mult = slipstream_speed_multiplier(naive_pos, naive_fwd, &[pace]);
            naive_tow += n_mult - 1.0;
            naive_pos += naive_fwd * (n_intent.throttle * BASE_SPEED * n_mult);

            let aim = draft_seeking_aim(seek_pos, objective, &[pace]);
            let s_intent = compute_steering(seek_pos, seek_fwd, aim, 0.0, CORNER_THROTTLE);
            seek_fwd = Vec2::from_angle(s_intent.steer * ROT)
                .rotate(seek_fwd)
                .normalize_or_zero();
            let s_mult = slipstream_speed_multiplier(seek_pos, seek_fwd, &[pace]);
            seek_tow += s_mult - 1.0;
            seek_pos += seek_fwd * (s_intent.throttle * BASE_SPEED * s_mult);

            pace_pos += pace_heading * PACE_SPEED;
        }

        // The seeker tucks dead-centre into the pace car's wake and rides it, so it
        // banks far more tow than the naive straight-liner that merely grazes the
        // lane, and that tow carries it measurably further up the course toward the
        // objective: active drafting earns its keep.
        assert!(
            seek_tow > 2.0 * naive_tow,
            "the seeker should bank far more tow by tucking in: \
             seek_tow={seek_tow}, naive_tow={naive_tow}"
        );
        assert!(
            seek_pos.y > naive_pos.y,
            "the drafted run should reach further toward the objective: \
             seek={}, naive={}",
            seek_pos.y,
            naive_pos.y
        );
    }
}
