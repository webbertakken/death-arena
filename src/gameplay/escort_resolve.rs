//! Escort resolve: the urge a raiding team's empty-handed cars find building in
//! them the longer one of their own clings to the enemy flag on its run home.
//!
//! The offensive *time* mirror of chase resolve ([`crate::gameplay::chase_resolve`]),
//! and the piece that completes the carry-pressure symmetry. The two sides of a
//! flag in flight had grown lopsided. A *robbed* side already feels its urge build
//! two ways: a flat flag-recovery rally ([`crate::gameplay::flag_rally`]) the instant
//! its flag goes out, then a chase resolve that ramps in the longer the thief holds.
//! A *raiding* side, by contrast, had only the flat flag escort
//! ([`crate::gameplay::flag_escort`]) it earns the instant its carrier lifts the
//! enemy flag, and nothing that built as the run home dragged on. So while the
//! carrier itself was squeezed harder and harder (a flat carry tax, a ramping
//! fatigue, a leading side's front-runner burden), the teammates clearing its path
//! pushed no harder than they had at the off. Escort resolve closes that out: the
//! escorting pack's urge hardens with the very frames the carrier tires over, so a
//! contested run is a genuine tug-of-war rather than a one-sided decay, and the flag
//! situation resolves into a capture or a turnover rather than circling the arena
//! forever.
//!
//! Modelled, like fatigue and chase resolve, as a small bonus that holds off through
//! an opening grace window then ramps in with the frames the enemy flag has been
//! carried, reaching its full bite only on a long run. It deliberately shares carrier
//! fatigue's grace and full-bite horizons (see [`escort_resolve_speed_multiplier`]) so
//! the carrier's drag and the escorts' resolve build in lockstep over the identical
//! window and can never drift apart, exactly as chase resolve ties the robbed pack to
//! the thief's fatigue.
//!
//! Like every per-car feel modifier the bonus is read by both movement systems, the
//! human's `car_movement_system` and the field's `virtual_player_drive_system`, so the
//! human and the AI escort on the identical terms: whichever side is hauling the enemy
//! flag home is the side whose escorts dig in.
//!
//! Escort resolve can only ever *speed* an empty-handed escort, never the flag carrier
//! it shepherds (the carrier hauling the enemy flag home earns none, exactly as it
//! earns no flat escort), so it can never let a flag run outpace the field: the tuned
//! "even the slowest chaser outpaces the fastest carrier" chase balance is left fully
//! intact, and the resolve only ever helps the pack clear the carrier's path, exactly
//! as the flag escort ([`crate::gameplay::flag_escort`]) and the chase resolve
//! ([`crate::gameplay::chase_resolve`]) it mirrors do.

use crate::gameplay::carry_fatigue::{CARRY_FATIGUE_FULL_FRAMES, CARRY_FATIGUE_GRACE_FRAMES};
use crate::gameplay::flag_escort::FLAG_ESCORT_SPEED_BONUS;

/// Largest fraction escort resolve adds to an empty-handed escort's speed at the
/// full bite.
///
/// The gentlest feel bonus of the lot by design: the flat [`FLAG_ESCORT_SPEED_BONUS`]
/// is the immediate push a raiding side earns when its carrier lifts the enemy flag,
/// and this is the slow-building top-up that only matters once a run drags on, so it
/// is pitched a notch below the flat escort it complements, the same way chase resolve
/// sits below its flat rally. The bonus scales up with the frames carried (see
/// [`escort_resolve_speed_multiplier`]), so this top rate is reached only after a long
/// run.
pub const ESCORT_RESOLVE_MAX_SPEED_BONUS: f32 = 0.03;

/// Escort resolve must be a real urge yet stay below the flat escort, enforced at
/// compile time, so the slow-building top-up never out-urges the immediate escort push
/// and can never drift into a power item. Settles the new floor of the feel-bonus
/// hierarchy: escort-resolve < flag-escort < chase-resolve < flag-rally < comeback <
/// slipstream.
const _: () = assert!(
    ESCORT_RESOLVE_MAX_SPEED_BONUS > 0.0
        && ESCORT_RESOLVE_MAX_SPEED_BONUS < FLAG_ESCORT_SPEED_BONUS
);

/// Speed multiplier an empty-handed escort earns from escort resolve, given the
/// `carry_frames` its own team's carrier has continuously held the enemy flag.
///
/// Returns `1.0` (no urge) when no car on the team holds the enemy flag
/// (`we_hold_enemy_flag` is `false`), or when the car is itself the flag carrier (the
/// resolve is for clearing the path, never for the flag run home, so the shepherded
/// carrier earns nothing while its empty-handed teammates dig in). While the team
/// holds the enemy flag and the car is empty-handed the bonus follows carrier
/// fatigue's ramp exactly, only flipped to a speed-up: nothing through the opening
/// [`CARRY_FATIGUE_GRACE_FRAMES`] grace window (a quick lift-and-score gives the
/// escorts no time to build resolve), then ramping in linearly with the frames
/// carried, building to the full [`ESCORT_RESOLVE_MAX_SPEED_BONUS`] by
/// [`CARRY_FATIGUE_FULL_FRAMES`] and held there for any longer run. Sharing those
/// horizons ties the escorts' resolve to the carrier's fatigue over the identical
/// window. The result is always in `1.0 ..= 1.0 + ESCORT_RESOLVE_MAX_SPEED_BONUS`.
///
/// The caller passes the raiding team's enemy-flag continuous-carry frame count (the
/// same count the carrier's own fatigue reads), so the same reading drives the human
/// and the field alike.
#[must_use]
pub fn escort_resolve_speed_multiplier(
    we_hold_enemy_flag: bool,
    carrying_flag: bool,
    carry_frames: u32,
) -> f32 {
    if carrying_flag || !we_hold_enemy_flag || carry_frames <= CARRY_FATIGUE_GRACE_FRAMES {
        return 1.0;
    }
    let span = CARRY_FATIGUE_FULL_FRAMES - CARRY_FATIGUE_GRACE_FRAMES;
    let into_resolve = (carry_frames - CARRY_FATIGUE_GRACE_FRAMES).min(span);
    let fraction = frames_to_f32(into_resolve) / frames_to_f32(span);
    ESCORT_RESOLVE_MAX_SPEED_BONUS.mul_add(fraction, 1.0)
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
    fn a_team_without_the_enemy_flag_finds_no_resolve() {
        // With no carrier of its own on the board there is no run to shepherd, so the
        // pack drives at its normal pace however the frame count reads.
        for frames in [0, CARRY_FATIGUE_GRACE_FRAMES + 1, CARRY_FATIGUE_FULL_FRAMES] {
            assert_near(escort_resolve_speed_multiplier(false, false, frames), 1.0);
        }
    }

    #[test]
    fn a_fresh_run_within_the_grace_window_grants_no_resolve() {
        // A quick lift-and-score gives the escorts no time to dig in, so the resolve
        // holds off entirely through the opening grace window.
        assert_near(escort_resolve_speed_multiplier(true, false, 0), 1.0);
        assert_near(
            escort_resolve_speed_multiplier(true, false, CARRY_FATIGUE_GRACE_FRAMES),
            1.0,
        );
    }

    #[test]
    fn a_run_just_past_the_grace_window_begins_to_urge() {
        let multiplier =
            escort_resolve_speed_multiplier(true, false, CARRY_FATIGUE_GRACE_FRAMES + 1);
        assert!(
            multiplier > 1.0 && multiplier < 1.0 + ESCORT_RESOLVE_MAX_SPEED_BONUS,
            "a run past the grace window should urge a little, got {multiplier}"
        );
    }

    #[test]
    fn a_long_run_reaches_the_full_bonus() {
        assert_near(
            escort_resolve_speed_multiplier(true, false, CARRY_FATIGUE_FULL_FRAMES),
            1.0 + ESCORT_RESOLVE_MAX_SPEED_BONUS,
        );
    }

    #[test]
    fn resolve_is_capped_beyond_the_full_horizon() {
        // A flag hauled far past the full horizon urges the escorts no harder than the
        // cap, so the multiplier can never run above its ceiling.
        assert_near(
            escort_resolve_speed_multiplier(true, false, CARRY_FATIGUE_FULL_FRAMES + 100_000),
            1.0 + ESCORT_RESOLVE_MAX_SPEED_BONUS,
        );
    }

    #[test]
    fn resolve_hardens_the_longer_the_run_drags_on() {
        let early = escort_resolve_speed_multiplier(true, false, CARRY_FATIGUE_GRACE_FRAMES + 60);
        let late = escort_resolve_speed_multiplier(true, false, CARRY_FATIGUE_FULL_FRAMES - 60);
        assert!(
            late > early && early > 1.0,
            "a longer run should urge the escorts harder: early={early}, late={late}"
        );
    }

    #[test]
    fn resolve_ramps_rather_than_snapping_to_full() {
        // Halfway through the ramp must be a genuine part bonus, strictly between
        // nothing and the full cap, so the urge builds with the run rather than
        // snapping straight to its top rate the instant the grace window lapses.
        let span = CARRY_FATIGUE_FULL_FRAMES - CARRY_FATIGUE_GRACE_FRAMES;
        let midpoint =
            escort_resolve_speed_multiplier(true, false, CARRY_FATIGUE_GRACE_FRAMES + span / 2);
        assert!(
            midpoint > 1.0 && midpoint < 1.0 + ESCORT_RESOLVE_MAX_SPEED_BONUS,
            "the midpoint should be a part bonus, got {midpoint}"
        );
    }

    #[test]
    fn the_shepherded_carrier_never_finds_the_resolve() {
        // This car is the one hauling the enemy flag home on a long run. The carrier
        // finds no resolve, so the bonus can never speed a flag run home, leaving the
        // chase balance fully intact while its empty-handed teammates clear the path.
        assert_near(
            escort_resolve_speed_multiplier(true, true, CARRY_FATIGUE_FULL_FRAMES),
            1.0,
        );
    }

    #[test]
    fn the_resolve_mirrors_carrier_fatigue_over_the_same_window() {
        // The escorts' resolve and the carrier's fatigue are flipped images over the
        // identical ramp: a longer run that scrubs a larger slice off the carrier adds
        // a proportionally larger slice onto the escorts, so the run is squeezed and
        // shepherded in lockstep.
        use crate::gameplay::carry_fatigue::carry_fatigue_speed_multiplier;
        for frames in [
            CARRY_FATIGUE_GRACE_FRAMES + 120,
            CARRY_FATIGUE_FULL_FRAMES - 120,
            CARRY_FATIGUE_FULL_FRAMES,
        ] {
            let fatigue_fraction = 1.0 - carry_fatigue_speed_multiplier(frames);
            let resolve_fraction = escort_resolve_speed_multiplier(true, false, frames) - 1.0;
            let fatigue_progress =
                fatigue_fraction / crate::gameplay::carry_fatigue::CARRY_FATIGUE_MAX_SPEED_PENALTY;
            let resolve_progress = resolve_fraction / ESCORT_RESOLVE_MAX_SPEED_BONUS;
            // Reconstructing each progress divides back out through a different-magnitude
            // cap (0.12 vs 0.03), so the shared ramp shape only survives to an f32
            // round-trip tolerance, not bit-equality; any genuine divergence would be
            // orders larger.
            assert!(
                (fatigue_progress - resolve_progress).abs() <= 1.0e-4,
                "fatigue and resolve must share the same ramp progress at {frames} frames: \
                 fatigue={fatigue_progress}, resolve={resolve_progress}"
            );
        }
    }

    // The invariant "resolve is a real urge yet stays below the flat escort"
    // (`ESCORT_RESOLVE_MAX_SPEED_BONUS < FLAG_ESCORT_SPEED_BONUS`) is enforced at
    // compile time by the `const _: () = assert!(..)` block above, so a runtime test
    // would only assert a constant clippy already proves.
}
