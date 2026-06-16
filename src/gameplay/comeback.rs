//! Comeback boost (catch-up): the classic arcade edge a team earns while it is
//! behind on captures, to keep a match alive to the flag.
//!
//! A staple of the Death Rally feel: fall a flag or two behind and your cars find
//! a little extra urge, so a runaway lead never quite shakes the chase and the
//! decider stays in reach. Modelled here as a small, capped speed bonus a car
//! carries while its team trails on captures, scaling with the size of the
//! deficit: nothing level or ahead, a part bonus one capture down, the full bonus
//! at the largest deficit a live match can hold. The bonus is read by both
//! movement systems, the human's `car_movement_system` and the field's
//! `virtual_player_drive_system`, so the human and the AI catch up on the
//! identical terms: whichever side trails is the side that gets the urge.
//!
//! A flag carrier never earns the catch-up: the bonus is for the cars chasing,
//! harassing and racing for pickups to claw the deficit back, never for speeding a
//! flag run home. That is flavour, but it is also what keeps the mechanic
//! balance-safe. Catch-up can only ever speed a *non-carrier*, so it can never let
//! a trailing team's flag run outpace the field; the tuned "even the slowest
//! chaser outpaces the fastest carrier" chase balance is left fully intact,
//! exactly as the slipstream tow ([`crate::gameplay::slipstream`]) leaves it.

use crate::gameplay::ctf::CAPTURES_TO_WIN;
use crate::gameplay::slipstream::DRAFT_MAX_SPEED_BONUS;

/// Largest fraction a fully-behind team's catch-up adds to a car's speed.
///
/// A modest urge, not a power item: pitched below a perfect slipstream tow
/// ([`DRAFT_MAX_SPEED_BONUS`]) so a handout for trailing can never out-reward the
/// pace a clean racing line earns, and far below a nitro burst. The bonus scales
/// down with the deficit (see [`comeback_speed_multiplier`]), so this top rate is
/// reached only by a team at the largest deficit a live match can hold.
pub const COMEBACK_MAX_SPEED_BONUS: f32 = 0.08;

/// Catch-up must be a real urge yet stay below an earned slipstream tow, enforced
/// at compile time so a handout for trailing never out-rewards clean positioning
/// and can never drift into a power item.
const _: () =
    assert!(COMEBACK_MAX_SPEED_BONUS > 0.0 && COMEBACK_MAX_SPEED_BONUS < DRAFT_MAX_SPEED_BONUS);

/// There must be a real deficit band to scale the catch-up across, enforced at
/// compile time, so the largest live deficit ([`CAPTURES_TO_WIN`] `- 1`) is a
/// positive divisor and the bonus ramps rather than snapping straight to full.
const _: () = assert!(CAPTURES_TO_WIN > 1);

/// Speed multiplier a non-carrier earns from its team trailing on captures.
///
/// Returns `1.0` (no urge) when the team is level or ahead, or when the car is a
/// flag carrier (the catch-up is for the chase, never for a flag run home). When
/// the team trails, the bonus scales linearly with the deficit: a part of
/// [`COMEBACK_MAX_SPEED_BONUS`] one capture down, building to the full bonus at the
/// largest deficit a live match can hold ([`CAPTURES_TO_WIN`] `- 1`, since a team
/// reaching [`CAPTURES_TO_WIN`] ends the match). A deficit beyond that band is
/// clamped, so the result is always in `1.0..=1.0 + COMEBACK_MAX_SPEED_BONUS`.
#[must_use]
pub fn comeback_speed_multiplier(
    own_captures: u32,
    enemy_captures: u32,
    carrying_flag: bool,
) -> f32 {
    if carrying_flag {
        return 1.0;
    }
    let steps = enemy_captures
        .saturating_sub(own_captures)
        .min(CAPTURES_TO_WIN - 1);
    if steps == 0 {
        return 1.0;
    }
    let fraction = captures_to_f32(steps) / captures_to_f32(CAPTURES_TO_WIN - 1);
    COMEBACK_MAX_SPEED_BONUS.mul_add(fraction, 1.0)
}

/// Losslessly widens a tiny capture count to `f32` for the deficit ramp.
///
/// Capture counts are bounded by [`CAPTURES_TO_WIN`], so they always fit a `u16`
/// and convert to `f32` exactly; the saturating fallback is unreachable in
/// practice and merely keeps the conversion total without an `as` cast or a panic.
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
    fn a_level_team_earns_no_catch_up() {
        assert_near(comeback_speed_multiplier(1, 1, false), 1.0);
    }

    #[test]
    fn a_leading_team_earns_no_catch_up() {
        assert_near(comeback_speed_multiplier(2, 0, false), 1.0);
    }

    #[test]
    fn a_trailing_team_earns_a_catch_up() {
        let multiplier = comeback_speed_multiplier(0, 1, false);
        assert!(
            multiplier > 1.0 && multiplier <= 1.0 + COMEBACK_MAX_SPEED_BONUS,
            "expected a capped catch-up, got {multiplier}"
        );
    }

    #[test]
    fn the_catch_up_strengthens_with_the_deficit() {
        let one_down = comeback_speed_multiplier(0, 1, false);
        let two_down = comeback_speed_multiplier(0, 2, false);
        assert!(
            two_down > one_down,
            "a larger deficit should urge harder: two_down={two_down}, one_down={one_down}"
        );
    }

    #[test]
    fn the_largest_live_deficit_earns_the_full_cap() {
        let full = comeback_speed_multiplier(0, CAPTURES_TO_WIN - 1, false);
        assert_near(full, 1.0 + COMEBACK_MAX_SPEED_BONUS);
    }

    #[test]
    fn a_deficit_beyond_the_band_is_capped() {
        let capped = comeback_speed_multiplier(0, CAPTURES_TO_WIN + 5, false);
        assert_near(capped, 1.0 + COMEBACK_MAX_SPEED_BONUS);
    }

    #[test]
    fn a_flag_carrier_never_earns_the_catch_up() {
        // Even at the largest deficit, a carrier gets no urge: the catch-up can
        // never speed a flag run home, leaving the chase balance fully intact.
        assert_near(comeback_speed_multiplier(0, CAPTURES_TO_WIN - 1, true), 1.0);
    }

    // The invariant "catch-up never exceeds an earned slipstream tow"
    // (`COMEBACK_MAX_SPEED_BONUS < DRAFT_MAX_SPEED_BONUS`) is enforced at compile
    // time by the `const _: () = assert!(..)` block above, so a runtime test would
    // only assert a constant clippy already proves.

    #[test]
    fn the_catch_up_ramps_rather_than_snapping_to_full() {
        // One capture down must be a genuine part bonus, strictly between nothing
        // and the full cap, so the urge builds with the deficit rather than
        // snapping straight to its top rate the instant a team falls behind.
        let one_down = comeback_speed_multiplier(0, 1, false);
        assert!(
            one_down > 1.0 && one_down < 1.0 + COMEBACK_MAX_SPEED_BONUS,
            "one capture down should be a part bonus, got {one_down}"
        );
    }
}
