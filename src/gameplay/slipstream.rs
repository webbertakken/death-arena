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
/// crossing car punches no usable hole in the air and grants no tow.
pub const DRAFT_MIN_ALIGNMENT: f32 = 0.5;

/// The alignment gate must be a real heading test, never trivially open or shut,
/// enforced at compile time.
const _: () = assert!(DRAFT_MIN_ALIGNMENT > 0.0 && DRAFT_MIN_ALIGNMENT < 1.0);

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
/// the same direction. The tow scales with both how close the gap is (full
/// nose-to-tail, fading to nothing at [`DRAFT_RADIUS`]) and how centred the
/// follower sits in the lane (full dead on the tail line, fading to nothing at the
/// lane edge), so the full [`DRAFT_MAX_SPEED_BONUS`] is earned only by tucking in
/// close on a tidy line; when several cars qualify, the strongest wake wins.
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
/// heading. Strength is the product of two linear falloffs: how close the gap is
/// (full nose-to-tail, nothing at [`DRAFT_RADIUS`]) and how centred the follower
/// sits in the lane (full dead on the tail line, nothing at the lane edge). A car
/// must therefore both tuck in close *and* hold a tidy line to win the strongest
/// tow, and the wake fades smoothly to nothing at the lane edge rather than
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
    if leader_heading.dot(heading) < DRAFT_MIN_ALIGNMENT {
        return None;
    }
    let gap_falloff = (1.0 - distance / DRAFT_RADIUS).clamp(0.0, 1.0);
    let lane_centring = (1.0 - lateral / DRAFT_LANE_HALF_WIDTH).clamp(0.0, 1.0);
    Some(gap_falloff * lane_centring)
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
}
