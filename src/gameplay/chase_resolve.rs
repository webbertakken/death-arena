//! Chase resolve: the urge a robbed team's empty-handed cars find building in
//! them the longer an enemy clings to their stolen flag.
//!
//! The chaser-side *time* mirror of carrier fatigue
//! ([`crate::gameplay::carry_fatigue`]), and the piece that completes the
//! steal-pressure symmetry. The carrier already feels two drags on a stolen flag:
//! a flat carry tax ([`crate::gameplay::ctf::flag_carrier_speed_multiplier`]) the
//! instant it lifts the flag, and a fatigue that ramps in the longer it holds
//! ([`crate::gameplay::carry_fatigue`]). The robbed side, by contrast, had only the
//! flat flag-recovery rally ([`crate::gameplay::flag_rally`]) it earns the instant
//! its flag goes out, and nothing that built as the steal dragged on. So a carrier
//! that survived the opening rally could still settle into a war of attrition the
//! defenders could never tighten. Chase resolve closes that out: the chasing pack's
//! urge hardens with the very frames the carrier tires over, so a keep-away is
//! squeezed from *both* ends at once and the flag situation always resolves rather
//! than circling the arena forever.
//!
//! Modelled, like fatigue, as a small bonus that holds off through an opening grace
//! window then ramps in with the frames the flag has been carried, reaching its
//! full bite only on a long hold. It deliberately shares carrier fatigue's grace
//! and full-bite horizons (see [`chase_resolve_speed_multiplier`]) so the carrier's
//! drag and the pack's resolve build in lockstep over the identical window and can
//! never drift apart.
//!
//! Like every per-car feel modifier the bonus is read by both movement systems, the
//! human's `car_movement_system` and the field's `virtual_player_drive_system`, so
//! the human and the AI find their resolve on the identical terms: whichever side's
//! flag is out is the side whose chasers dig in.
//!
//! Chase resolve can only ever *speed* an empty-handed chaser, never a flag carrier
//! (a double-steal carrier hauling the enemy flag home earns none, exactly as it
//! earns no rally), so it can never let a flag run outpace the field: the tuned
//! "even the slowest chaser outpaces the fastest carrier" chase balance is left
//! fully intact, and the resolve only ever helps the pack close on the thief,
//! exactly as the flag-recovery rally ([`crate::gameplay::flag_rally`]) and the
//! slipstream tow ([`crate::gameplay::slipstream`]) do.

use crate::gameplay::carry_fatigue::{CARRY_FATIGUE_FULL_FRAMES, CARRY_FATIGUE_GRACE_FRAMES};
use crate::gameplay::flag_rally::FLAG_RALLY_SPEED_BONUS;

/// Largest fraction chase resolve adds to an empty-handed chaser's speed at the
/// full bite.
///
/// The gentlest of the chaser bonuses by design: the flat
/// [`FLAG_RALLY_SPEED_BONUS`] is the immediate push a steal earns, and this is the
/// slow-building top-up that only matters once a carrier refuses to commit, so it
/// is pitched a notch below the rally it complements. The bonus scales up with the
/// frames carried (see [`chase_resolve_speed_multiplier`]), so this top rate is
/// reached only after a long hold.
pub const CHASE_RESOLVE_MAX_SPEED_BONUS: f32 = 0.05;

/// Chase resolve must be a real urge yet stay below the flat steal-window rally,
/// enforced at compile time, so the slow-building top-up never out-urges the
/// immediate flag-recovery push and can never drift into a power item. Extends the
/// feel-bonus hierarchy to chase-resolve < flag-rally < comeback < slipstream.
const _: () = assert!(
    CHASE_RESOLVE_MAX_SPEED_BONUS > 0.0 && CHASE_RESOLVE_MAX_SPEED_BONUS < FLAG_RALLY_SPEED_BONUS
);

/// Speed multiplier an empty-handed chaser earns from chase resolve, given the
/// `carry_frames` its own team's stolen flag has been continuously carried.
///
/// Returns `1.0` (no urge) when the team's flag is safe (`own_flag_stolen` is
/// `false`), or when the car is itself a flag carrier (the resolve is for the
/// chase, never for a flag run home, so a double-steal carrier earns nothing while
/// its empty-handed teammates dig in). While the flag is out and the car is
/// empty-handed the bonus follows carrier fatigue's ramp exactly, only flipped to a
/// speed-up: nothing through the opening [`CARRY_FATIGUE_GRACE_FRAMES`] grace window
/// (a quick steal-and-score gives the pack no time to build resolve), then ramping
/// in linearly with the frames carried, building to the full
/// [`CHASE_RESOLVE_MAX_SPEED_BONUS`] by [`CARRY_FATIGUE_FULL_FRAMES`] and held there
/// for any longer hold. Sharing those horizons ties the pack's resolve to the
/// carrier's fatigue over the identical window. The result is always in
/// `1.0 ..= 1.0 + CHASE_RESOLVE_MAX_SPEED_BONUS`.
///
/// The caller passes the robbed team's own flag's continuous-carry frame count, so
/// the same reading drives the human and the field alike.
#[must_use]
pub fn chase_resolve_speed_multiplier(
    own_flag_stolen: bool,
    carrying_flag: bool,
    carry_frames: u32,
) -> f32 {
    if carrying_flag || !own_flag_stolen || carry_frames <= CARRY_FATIGUE_GRACE_FRAMES {
        return 1.0;
    }
    let span = CARRY_FATIGUE_FULL_FRAMES - CARRY_FATIGUE_GRACE_FRAMES;
    let into_resolve = (carry_frames - CARRY_FATIGUE_GRACE_FRAMES).min(span);
    let fraction = frames_to_f32(into_resolve) / frames_to_f32(span);
    CHASE_RESOLVE_MAX_SPEED_BONUS.mul_add(fraction, 1.0)
}

/// Losslessly widens a small frame count to `f32` for the resolve ramp.
///
/// The frame counts fed to the ramp are clamped to the [`CARRY_FATIGUE_FULL_FRAMES`]
/// span before conversion, so they always fit a `u16` and convert exactly; the
/// saturating fallback is unreachable in practice and merely keeps the conversion
/// total without an `as` cast or a panic.
fn frames_to_f32(value: u32) -> f32 {
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
    fn a_team_with_its_flag_safe_finds_no_resolve() {
        // With its flag home there is no steal to chase, so the pack drives at its
        // normal pace however the frame count reads.
        for frames in [0, CARRY_FATIGUE_GRACE_FRAMES + 1, CARRY_FATIGUE_FULL_FRAMES] {
            assert_near(chase_resolve_speed_multiplier(false, false, frames), 1.0);
        }
    }

    #[test]
    fn a_fresh_steal_within_the_grace_window_grants_no_resolve() {
        // A quick steal-and-score gives the pack no time to dig in, so the resolve
        // holds off entirely through the opening grace window.
        assert_near(chase_resolve_speed_multiplier(true, false, 0), 1.0);
        assert_near(
            chase_resolve_speed_multiplier(true, false, CARRY_FATIGUE_GRACE_FRAMES),
            1.0,
        );
    }

    #[test]
    fn a_steal_just_past_the_grace_window_begins_to_urge() {
        let multiplier =
            chase_resolve_speed_multiplier(true, false, CARRY_FATIGUE_GRACE_FRAMES + 1);
        assert!(
            multiplier > 1.0 && multiplier < 1.0 + CHASE_RESOLVE_MAX_SPEED_BONUS,
            "a steal past the grace window should urge a little, got {multiplier}"
        );
    }

    #[test]
    fn a_long_hold_reaches_the_full_bonus() {
        assert_near(
            chase_resolve_speed_multiplier(true, false, CARRY_FATIGUE_FULL_FRAMES),
            1.0 + CHASE_RESOLVE_MAX_SPEED_BONUS,
        );
    }

    #[test]
    fn resolve_is_capped_beyond_the_full_horizon() {
        // A flag clung to far past the full horizon urges the pack no harder than the
        // cap, so the multiplier can never run above its ceiling.
        assert_near(
            chase_resolve_speed_multiplier(true, false, CARRY_FATIGUE_FULL_FRAMES + 100_000),
            1.0 + CHASE_RESOLVE_MAX_SPEED_BONUS,
        );
    }

    #[test]
    fn resolve_hardens_the_longer_the_flag_is_held() {
        let early = chase_resolve_speed_multiplier(true, false, CARRY_FATIGUE_GRACE_FRAMES + 60);
        let late = chase_resolve_speed_multiplier(true, false, CARRY_FATIGUE_FULL_FRAMES - 60);
        assert!(
            late > early && early > 1.0,
            "a longer hold should urge the pack harder: early={early}, late={late}"
        );
    }

    #[test]
    fn resolve_ramps_rather_than_snapping_to_full() {
        // Halfway through the ramp must be a genuine part bonus, strictly between
        // nothing and the full cap, so the urge builds with the hold rather than
        // snapping straight to its top rate the instant the grace window lapses.
        let span = CARRY_FATIGUE_FULL_FRAMES - CARRY_FATIGUE_GRACE_FRAMES;
        let midpoint =
            chase_resolve_speed_multiplier(true, false, CARRY_FATIGUE_GRACE_FRAMES + span / 2);
        assert!(
            midpoint > 1.0 && midpoint < 1.0 + CHASE_RESOLVE_MAX_SPEED_BONUS,
            "the midpoint should be a part bonus, got {midpoint}"
        );
    }

    #[test]
    fn a_flag_carrier_never_finds_the_resolve() {
        // A double steal: this car hauls the enemy flag home while its own flag is
        // also out and long held. The carrier finds no resolve, so the bonus can
        // never speed a flag run home, leaving the chase balance fully intact.
        assert_near(
            chase_resolve_speed_multiplier(true, true, CARRY_FATIGUE_FULL_FRAMES),
            1.0,
        );
    }

    #[test]
    fn the_resolve_mirrors_carrier_fatigue_over_the_same_window() {
        // The pack's resolve and the carrier's fatigue are flipped images over the
        // identical ramp: a deeper hold that scrubs a larger slice off the carrier
        // adds a proportionally larger slice onto the chasers, so the squeeze
        // tightens from both ends in lockstep.
        use crate::gameplay::carry_fatigue::carry_fatigue_speed_multiplier;
        for frames in [
            CARRY_FATIGUE_GRACE_FRAMES + 120,
            CARRY_FATIGUE_FULL_FRAMES - 120,
            CARRY_FATIGUE_FULL_FRAMES,
        ] {
            let fatigue_fraction = 1.0 - carry_fatigue_speed_multiplier(frames);
            let resolve_fraction = chase_resolve_speed_multiplier(true, false, frames) - 1.0;
            let fatigue_progress =
                fatigue_fraction / crate::gameplay::carry_fatigue::CARRY_FATIGUE_MAX_SPEED_PENALTY;
            let resolve_progress = resolve_fraction / CHASE_RESOLVE_MAX_SPEED_BONUS;
            // Reconstructing each progress divides back out through a different-magnitude
            // cap (0.12 vs 0.05), so the shared ramp shape only survives to an f32
            // round-trip tolerance, not bit-equality; any genuine divergence would be
            // orders larger.
            assert!(
                (fatigue_progress - resolve_progress).abs() <= 1.0e-4,
                "fatigue and resolve must share the same ramp progress at {frames} frames: \
                 fatigue={fatigue_progress}, resolve={resolve_progress}"
            );
        }
    }

    // The invariant "resolve is a real urge yet stays below the flat rally"
    // (`CHASE_RESOLVE_MAX_SPEED_BONUS < FLAG_RALLY_SPEED_BONUS`) is enforced at
    // compile time by the `const _: () = assert!(..)` block above, so a runtime test
    // would only assert a constant clippy already proves.
}
