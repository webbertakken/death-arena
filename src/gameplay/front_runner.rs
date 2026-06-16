//! Front-runner's burden: the speed a leading team's flag carrier sheds while it
//! is ahead on captures, the carrier-side anti-snowball lever.
//!
//! The exact mirror of the catch-up boost ([`crate::gameplay::comeback`]): where
//! that urges a *trailing* team's chasers on so a runaway lead never quite shakes
//! the pack, this drags a *leading* team's flag runner back so a side already in
//! front cannot also sprint its decider home unopposed. Together the two close the
//! anti-snowball loop on the speed axis, alongside the economy's matching levers
//! (the comeback capture bonus and the combat most-wanted bounty): a team behind
//! gets the urge, a team ahead carries the weight, and the decider stays in reach.
//! Modelled here as a small, capped speed penalty a carrier suffers while its team
//! leads on captures, scaling with the size of the lead: nothing level or behind,
//! a part penalty one capture up, the full bite at the largest lead a live match
//! can hold. The penalty is read by both movement systems, the human's
//! `car_movement_system` and the field's `virtual_player_drive_system`, so the
//! human and the AI carry the burden on the identical terms: whichever side leads
//! is the side whose flag run is weighed down.
//!
//! The burden falls only on a flag carrier, the mirror image of the catch-up that
//! spares one: a non-carrier on a leading team drives unhindered. That is flavour,
//! but it is also what keeps the mechanic balance-safe. The drag can only ever
//! *slow* a carrier, never speed one, so it can never let a flag run outpace the
//! field: it leaves the tuned "even the slowest chaser outpaces the fastest
//! carrier" chase balance fully intact and only ever presses it harder, exactly as
//! the flat carry tax, carrier fatigue and the spoiled slipstream all do for a
//! carrier.

use crate::gameplay::ctf::CAPTURES_TO_WIN;

/// Largest fraction of its speed a fully-ahead team's carrier sheds to the burden.
///
/// A modest weight, not a shackle: pitched no harsher than a fully-behind team's
/// catch-up urge ([`crate::gameplay::comeback::COMEBACK_MAX_SPEED_BONUS`]) so the
/// two anti-snowball speed levers stay balanced, neither side's rubber band
/// overpowering the other. The penalty scales down with the lead (see
/// [`front_runner_speed_multiplier`]), so this top rate is reached only by a team
/// at the largest lead a live match can hold.
pub const FRONT_RUNNER_MAX_SPEED_PENALTY: f32 = 0.06;

/// The burden must be a real weight yet never a stop, enforced at compile time so
/// the penalty can never drift into pinning a leading carrier motionless.
const _: () = assert!(FRONT_RUNNER_MAX_SPEED_PENALTY > 0.0 && FRONT_RUNNER_MAX_SPEED_PENALTY < 0.5);

/// The carrier-side burden must stay no harsher than the chaser-side catch-up it
/// mirrors, enforced at compile time, so the two anti-snowball speed levers are
/// balanced and a leading carrier is never dragged back harder than a trailing
/// chaser is urged on.
const _: () =
    assert!(FRONT_RUNNER_MAX_SPEED_PENALTY <= crate::gameplay::comeback::COMEBACK_MAX_SPEED_BONUS);

/// There must be a real lead band to scale the burden across, enforced at compile
/// time, so the largest live lead ([`CAPTURES_TO_WIN`] `- 1`) is a positive divisor
/// and the penalty ramps rather than snapping straight to full.
const _: () = assert!(CAPTURES_TO_WIN > 1);

/// A fully-burdened carrier on an otherwise sound engine must still out-roll a
/// near-wrecked one limping on minimum integrity, enforced at compile time. The
/// burden layers on top of the flat carry tax
/// ([`crate::gameplay::ctf::FLAG_CARRIER_SPEED_MULTIPLIER`]) and full carrier
/// fatigue ([`crate::gameplay::carry_fatigue::CARRY_FATIGUE_MAX_SPEED_PENALTY`]),
/// and even with all three stacked the carrier must out-pace
/// [`crate::gameplay::combat::MIN_INTEGRITY_SPEED_MULTIPLIER`], so the worst a
/// leading, tired carry can cost still never tips below a battered engine and the
/// speed-penalty ordering stays coherent.
const _: () = assert!(
    crate::gameplay::ctf::FLAG_CARRIER_SPEED_MULTIPLIER
        * (1.0 - crate::gameplay::carry_fatigue::CARRY_FATIGUE_MAX_SPEED_PENALTY)
        * (1.0 - FRONT_RUNNER_MAX_SPEED_PENALTY)
        > crate::gameplay::combat::MIN_INTEGRITY_SPEED_MULTIPLIER
);

/// Speed multiplier a flag carrier suffers from its team leading on captures.
///
/// Returns `1.0` (no burden) when the team is level or behind, or when the car is
/// not a flag carrier (the burden is for the flag run, never for the chasers). When
/// a carrier's team leads, the penalty scales linearly with the lead: a part of
/// [`FRONT_RUNNER_MAX_SPEED_PENALTY`] one capture up, building to the full penalty
/// at the largest lead a live match can hold ([`CAPTURES_TO_WIN`] `- 1`, since a
/// team reaching [`CAPTURES_TO_WIN`] ends the match). A lead beyond that band is
/// clamped, so the result is always in `1.0 - FRONT_RUNNER_MAX_SPEED_PENALTY ..= 1.0`.
#[must_use]
pub fn front_runner_speed_multiplier(
    own_captures: u32,
    enemy_captures: u32,
    carrying_flag: bool,
) -> f32 {
    if !carrying_flag {
        return 1.0;
    }
    let steps = own_captures
        .saturating_sub(enemy_captures)
        .min(CAPTURES_TO_WIN - 1);
    if steps == 0 {
        return 1.0;
    }
    let fraction = captures_to_f32(steps) / captures_to_f32(CAPTURES_TO_WIN - 1);
    FRONT_RUNNER_MAX_SPEED_PENALTY.mul_add(-fraction, 1.0)
}

/// Losslessly widens a tiny capture count to `f32` for the lead ramp.
///
/// Capture counts are bounded by [`CAPTURES_TO_WIN`], so they always fit a `u16`
/// and convert to `f32` exactly; the saturating fallback is unreachable in practice
/// and merely keeps the conversion total without an `as` cast or a panic.
fn captures_to_f32(value: u32) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= f32::EPSILON,
            "actual={actual}, expected={expected}"
        );
    }

    #[test]
    fn a_level_team_carries_no_burden() {
        assert_near(front_runner_speed_multiplier(1, 1, true), 1.0);
    }

    #[test]
    fn a_trailing_team_carries_no_burden() {
        assert_near(front_runner_speed_multiplier(0, 2, true), 1.0);
    }

    #[test]
    fn a_leading_carrier_is_weighed_down() {
        let multiplier = front_runner_speed_multiplier(1, 0, true);
        assert!(
            (1.0 - FRONT_RUNNER_MAX_SPEED_PENALTY..1.0).contains(&multiplier),
            "expected a capped burden, got {multiplier}"
        );
    }

    #[test]
    fn the_burden_strengthens_with_the_lead() {
        let one_up = front_runner_speed_multiplier(1, 0, true);
        let two_up = front_runner_speed_multiplier(2, 0, true);
        assert!(
            two_up < one_up,
            "a larger lead should weigh heavier: two_up={two_up}, one_up={one_up}"
        );
    }

    #[test]
    fn the_largest_live_lead_reaches_the_full_penalty() {
        let full = front_runner_speed_multiplier(CAPTURES_TO_WIN - 1, 0, true);
        assert_near(full, 1.0 - FRONT_RUNNER_MAX_SPEED_PENALTY);
    }

    #[test]
    fn a_lead_beyond_the_band_is_capped() {
        let capped = front_runner_speed_multiplier(CAPTURES_TO_WIN + 5, 0, true);
        assert_near(capped, 1.0 - FRONT_RUNNER_MAX_SPEED_PENALTY);
    }

    #[test]
    fn a_non_carrier_never_carries_the_burden() {
        // Even at the largest lead, a chaser drives unhindered: the burden falls
        // only on the flag run, the mirror of the catch-up sparing a carrier.
        assert_near(
            front_runner_speed_multiplier(CAPTURES_TO_WIN - 1, 0, false),
            1.0,
        );
    }

    #[test]
    fn the_burden_ramps_rather_than_snapping_to_full() {
        // One capture up must be a genuine part penalty, strictly between nothing
        // and the full cap, so the weight builds with the lead rather than snapping
        // straight to its top rate the instant a team edges ahead.
        let one_up = front_runner_speed_multiplier(1, 0, true);
        assert!(
            one_up > 1.0 - FRONT_RUNNER_MAX_SPEED_PENALTY && one_up < 1.0,
            "one capture up should be a part penalty, got {one_up}"
        );
    }
}
