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
//!
//! How hard a driver digs into the resolve is its own personality, the time-ramped
//! mirror of the same commitment scaling its flat sibling the flag-recovery rally
//! ([`crate::gameplay::flag_rally::flag_rally_speed_multiplier`]) already applies: a
//! keener driver (a higher
//! [`crate::gameplay::virtual_player::VirtualPlayer::corner_throttle`], the commitment
//! axis that already sets how hard it stays on the gas through a corner, how deep it
//! noses a kill home and how hard it runs a thief down) digs in harder the longer a
//! steal drags on, while a disciplined one digs in more gently. The scale is centred
//! on the neutral [`MIN_THROTTLE`] baseline the all-rounder corners on, so the baseline
//! driver, and the human that mirrors it, keep the exact original ramped resolve; only
//! a roster of distinct AI personalities deviates from it. Scaling both the rally and
//! its slow-building top-up keeps a keen driver consistently quicker on the chase
//! across the *whole* steal, not just at the off. The commitment band is pitched
//! gentler than the rally's (it reuses flag escort's gain, not the rally's): chase
//! resolve sits low in the feel-bonus hierarchy with tight headroom to the flat rally
//! just above it, so even the keenest scaled resolve stays below that rally
//! (compile-asserted), keeping the hierarchy chase-resolve < flag-rally intact at every
//! commitment.

use crate::gameplay::carry_fatigue::{CARRY_FATIGUE_FULL_FRAMES, CARRY_FATIGUE_GRACE_FRAMES};
use crate::gameplay::flag_rally::FLAG_RALLY_SPEED_BONUS;
use crate::gameplay::virtual_player::ai::MIN_THROTTLE;

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

/// How far a driver's chase resolve scales per unit of cornering commitment away from
/// the neutral [`MIN_THROTTLE`] baseline.
///
/// A keener driver (a higher
/// [`crate::gameplay::virtual_player::VirtualPlayer::corner_throttle`]) digs into the
/// resolve harder, a disciplined one more gently. The time-ramped mirror of the same
/// commitment axis that scales the flat flag-recovery rally
/// ([`crate::gameplay::flag_rally::flag_rally_speed_multiplier`]), but pitched gentler
/// (the rally uses `1.5`): chase resolve sits low in the feel-bonus hierarchy with
/// tight headroom to the rally just above it, so a hotter gain would push the keenest
/// scaled resolve past that flat rally. It reuses flag escort's gentler gain instead,
/// the same gain every bottom-of-hierarchy lever takes.
const CHASE_RESOLVE_COMMITMENT_SCALE_GAIN: f32 = 1.0;

/// Floor on the commitment-driven resolve scale: a safety net so a degenerate or
/// extreme-disciplined `corner_throttle` can never collapse the resolve to nothing (or
/// invert it). The asserted roster commitment band (`0.15..=0.5`, the range the driver
/// roster holds each
/// [`crate::gameplay::virtual_player::VirtualPlayer::corner_throttle`] to) maps strictly
/// inside this band, so the clamp only ever guards a garbage throttle, never a real
/// driver's personality.
const CHASE_RESOLVE_COMMITMENT_SCALE_MIN: f32 = 0.85;

/// Ceiling on the commitment-driven resolve scale: the keen counterpart to the floor,
/// so a degenerate or extreme-reckless `corner_throttle` tops out here rather than
/// scaling the resolve without bound. Held low enough that even the keenest scaled
/// resolve stays below the flat flag-recovery rally just above it (asserted below):
/// chase resolve has tighter headroom to its rally than flag escort has to its chase
/// resolve, so its ceiling is held a notch lower than flag escort's `1.2`.
const CHASE_RESOLVE_COMMITMENT_SCALE_MAX: f32 = 1.15;

/// The resolve scale is centred on the baseline driver (scale `1.0`, the original
/// ramped resolve) and never inverts commitment: a keener driver always digs in at
/// least as hard as a more disciplined one. Enforced at compile time.
const _: () =
    assert!(CHASE_RESOLVE_COMMITMENT_SCALE_MIN < 1.0 && CHASE_RESOLVE_COMMITMENT_SCALE_MAX > 1.0);

/// Commitment must genuinely strengthen the resolve, never weaken or flatten it,
/// enforced at compile time.
const _: () = assert!(CHASE_RESOLVE_COMMITMENT_SCALE_GAIN > 0.0);

/// Even the keenest scaled resolve must stay below the flat flag-recovery rally just
/// above it in the hierarchy, enforced at compile time, so the personality scaling can
/// never lift the slow-building top-up past the immediate steal-window push nor drift
/// into a power item. The flat [`CHASE_RESOLVE_MAX_SPEED_BONUS`] `<`
/// [`FLAG_RALLY_SPEED_BONUS`] ordering above is untouched: this guards only the extra
/// headroom the commitment ceiling opens up.
const _: () = assert!(
    CHASE_RESOLVE_MAX_SPEED_BONUS * CHASE_RESOLVE_COMMITMENT_SCALE_MAX < FLAG_RALLY_SPEED_BONUS
);

/// Scales a driver's chase resolve by its cornering commitment.
///
/// A driver cornering on the neutral [`MIN_THROTTLE`] floor scales by exactly `1.0`, so
/// the all-rounder baseline and the human's mirror keep the original ramped resolve
/// untouched. A keener driver (a higher `corner_throttle`) scales up toward
/// [`CHASE_RESOLVE_COMMITMENT_SCALE_MAX`]; a disciplined one down toward
/// [`CHASE_RESOLVE_COMMITMENT_SCALE_MIN`]. The affine map is clamped to the
/// [[`CHASE_RESOLVE_COMMITMENT_SCALE_MIN`], [`CHASE_RESOLVE_COMMITMENT_SCALE_MAX`]] band
/// as a safety net for a degenerate throttle.
#[must_use]
fn chase_resolve_commitment_scale(corner_throttle: f32) -> f32 {
    let keen = (corner_throttle - MIN_THROTTLE) * CHASE_RESOLVE_COMMITMENT_SCALE_GAIN;
    (1.0 + keen).clamp(
        CHASE_RESOLVE_COMMITMENT_SCALE_MIN,
        CHASE_RESOLVE_COMMITMENT_SCALE_MAX,
    )
}

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
/// carrier's fatigue over the identical window.
///
/// The full bite is itself scaled by the driver's commitment (see
/// [`chase_resolve_commitment_scale`]): a driver on the neutral [`MIN_THROTTLE`] floor
/// earns exactly the original ramped resolve, so the all-rounder baseline and the
/// human's mirror (which pass `MIN_THROTTLE`) keep it untouched; a keener driver digs in
/// harder as the steal drags on, a disciplined one more gently. The scaled bite stays
/// strictly below the flat flag-recovery rally (compile-asserted), so the result is
/// always in `1.0 ..= 1.0 + CHASE_RESOLVE_MAX_SPEED_BONUS * CHASE_RESOLVE_COMMITMENT_SCALE_MAX`.
///
/// The caller passes the robbed team's own flag's continuous-carry frame count, so
/// the same reading drives the human and the field alike.
#[must_use]
pub fn chase_resolve_speed_multiplier(
    own_flag_stolen: bool,
    carrying_flag: bool,
    carry_frames: u32,
    corner_throttle: f32,
) -> f32 {
    if carrying_flag || !own_flag_stolen || carry_frames <= CARRY_FATIGUE_GRACE_FRAMES {
        return 1.0;
    }
    let span = CARRY_FATIGUE_FULL_FRAMES - CARRY_FATIGUE_GRACE_FRAMES;
    let into_resolve = (carry_frames - CARRY_FATIGUE_GRACE_FRAMES).min(span);
    let fraction = frames_to_f32(into_resolve) / frames_to_f32(span);
    let bonus = CHASE_RESOLVE_MAX_SPEED_BONUS * chase_resolve_commitment_scale(corner_throttle);
    bonus.mul_add(fraction, 1.0)
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

    /// A keen, reckless driver and a disciplined one, both well inside the asserted
    /// roster commitment band (`0.15..=0.5`), so the tests read the real scaling
    /// without coupling to the private roster profiles. The baseline is
    /// [`MIN_THROTTLE`], the neutral throttle the all-rounder and the human mirror.
    const KEEN_THROTTLE: f32 = 0.45;
    const DISCIPLINED_THROTTLE: f32 = 0.2;

    #[test]
    fn a_team_with_its_flag_safe_finds_no_resolve() {
        // With its flag home there is no steal to chase, so the pack drives at its
        // normal pace however the frame count reads.
        for frames in [0, CARRY_FATIGUE_GRACE_FRAMES + 1, CARRY_FATIGUE_FULL_FRAMES] {
            assert_near(
                chase_resolve_speed_multiplier(false, false, frames, MIN_THROTTLE),
                1.0,
            );
        }
    }

    #[test]
    fn a_fresh_steal_within_the_grace_window_grants_no_resolve() {
        // A quick steal-and-score gives the pack no time to dig in, so the resolve
        // holds off entirely through the opening grace window.
        assert_near(
            chase_resolve_speed_multiplier(true, false, 0, MIN_THROTTLE),
            1.0,
        );
        assert_near(
            chase_resolve_speed_multiplier(true, false, CARRY_FATIGUE_GRACE_FRAMES, MIN_THROTTLE),
            1.0,
        );
    }

    #[test]
    fn a_steal_just_past_the_grace_window_begins_to_urge() {
        let multiplier = chase_resolve_speed_multiplier(
            true,
            false,
            CARRY_FATIGUE_GRACE_FRAMES + 1,
            MIN_THROTTLE,
        );
        assert!(
            multiplier > 1.0 && multiplier < 1.0 + CHASE_RESOLVE_MAX_SPEED_BONUS,
            "a steal past the grace window should urge a little, got {multiplier}"
        );
    }

    #[test]
    fn a_long_hold_reaches_the_full_bonus() {
        assert_near(
            chase_resolve_speed_multiplier(true, false, CARRY_FATIGUE_FULL_FRAMES, MIN_THROTTLE),
            1.0 + CHASE_RESOLVE_MAX_SPEED_BONUS,
        );
    }

    #[test]
    fn resolve_is_capped_beyond_the_full_horizon() {
        // A flag clung to far past the full horizon urges the pack no harder than the
        // cap, so the multiplier can never run above its ceiling.
        assert_near(
            chase_resolve_speed_multiplier(
                true,
                false,
                CARRY_FATIGUE_FULL_FRAMES + 100_000,
                MIN_THROTTLE,
            ),
            1.0 + CHASE_RESOLVE_MAX_SPEED_BONUS,
        );
    }

    #[test]
    fn resolve_hardens_the_longer_the_flag_is_held() {
        let early = chase_resolve_speed_multiplier(
            true,
            false,
            CARRY_FATIGUE_GRACE_FRAMES + 60,
            MIN_THROTTLE,
        );
        let late = chase_resolve_speed_multiplier(
            true,
            false,
            CARRY_FATIGUE_FULL_FRAMES - 60,
            MIN_THROTTLE,
        );
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
        let midpoint = chase_resolve_speed_multiplier(
            true,
            false,
            CARRY_FATIGUE_GRACE_FRAMES + span / 2,
            MIN_THROTTLE,
        );
        assert!(
            midpoint > 1.0 && midpoint < 1.0 + CHASE_RESOLVE_MAX_SPEED_BONUS,
            "the midpoint should be a part bonus, got {midpoint}"
        );
    }

    #[test]
    fn a_flag_carrier_never_finds_the_resolve() {
        // A double steal: this car hauls the enemy flag home while its own flag is
        // also out and long held. The carrier finds no resolve, so the bonus can
        // never speed a flag run home, leaving the chase balance fully intact. Even a
        // reckless carrier earns nothing.
        assert_near(
            chase_resolve_speed_multiplier(true, true, CARRY_FATIGUE_FULL_FRAMES, KEEN_THROTTLE),
            1.0,
        );
    }

    #[test]
    fn the_baseline_driver_keeps_the_original_ramped_resolve() {
        // The all-rounder and the human corner on the neutral MIN_THROTTLE floor, so a
        // full hold earns the exact pre-personality resolve: the unscaled cap, never a
        // notch more or less.
        assert_near(
            chase_resolve_speed_multiplier(true, false, CARRY_FATIGUE_FULL_FRAMES, MIN_THROTTLE),
            1.0 + CHASE_RESOLVE_MAX_SPEED_BONUS,
        );
    }

    #[test]
    fn the_commitment_scale_is_neutral_at_the_baseline_throttle() {
        // The all-rounder (and the human that mirrors it) corner on MIN_THROTTLE, so the
        // scale is exactly 1.0 there and the baseline resolve is untouched.
        assert_near(chase_resolve_commitment_scale(MIN_THROTTLE), 1.0);
    }

    #[test]
    fn a_keener_driver_digs_in_harder_than_the_baseline() {
        // The personality lever: at the same long hold a keener, gas-committed driver
        // digs into the resolve harder than the neutral baseline, so it presses the
        // chase harder the longer the steal drags on.
        let baseline =
            chase_resolve_speed_multiplier(true, false, CARRY_FATIGUE_FULL_FRAMES, MIN_THROTTLE);
        let keen =
            chase_resolve_speed_multiplier(true, false, CARRY_FATIGUE_FULL_FRAMES, KEEN_THROTTLE);
        assert!(
            keen > baseline,
            "a keener driver should dig in harder: keen={keen}, baseline={baseline}"
        );
    }

    #[test]
    fn a_disciplined_driver_digs_in_more_gently_than_the_baseline() {
        // The mirror of the keen case: a disciplined driver still finds a real resolve
        // (above 1.0) but a gentler one than the neutral baseline.
        let baseline =
            chase_resolve_speed_multiplier(true, false, CARRY_FATIGUE_FULL_FRAMES, MIN_THROTTLE);
        let disciplined = chase_resolve_speed_multiplier(
            true,
            false,
            CARRY_FATIGUE_FULL_FRAMES,
            DISCIPLINED_THROTTLE,
        );
        assert!(
            disciplined < baseline && disciplined > 1.0,
            "a disciplined driver should still dig in, but gentler: \
             disciplined={disciplined}, baseline={baseline}"
        );
    }

    #[test]
    fn the_keenest_roster_driver_stays_below_the_flag_rally() {
        // The roster caps a driver's cornering commitment at 0.5 (asserted in spawn.rs).
        // Even that keenest driver, on a full hold, must dig in to a resolve strictly
        // below the flat flag-recovery rally just above it, so the slow-building top-up
        // never out-urges the immediate steal-window push.
        let keenest = chase_resolve_speed_multiplier(true, false, CARRY_FATIGUE_FULL_FRAMES, 0.5);
        assert!(
            keenest < 1.0 + FLAG_RALLY_SPEED_BONUS,
            "the keenest scaled resolve ({keenest}) must stay below the flat rally ({})",
            1.0 + FLAG_RALLY_SPEED_BONUS
        );
        assert!(
            keenest > 1.0 + CHASE_RESOLVE_MAX_SPEED_BONUS,
            "the keenest driver should still out-dig the unscaled cap: {keenest}"
        );
    }

    #[test]
    fn the_commitment_scale_clamps_a_degenerate_throttle() {
        // A garbage throttle far outside the roster band can never collapse the resolve
        // to nothing nor blow it out: the clamp pins it to the band.
        assert_near(
            chase_resolve_commitment_scale(-100.0),
            CHASE_RESOLVE_COMMITMENT_SCALE_MIN,
        );
        assert_near(
            chase_resolve_commitment_scale(100.0),
            CHASE_RESOLVE_COMMITMENT_SCALE_MAX,
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
            let resolve_fraction =
                chase_resolve_speed_multiplier(true, false, frames, MIN_THROTTLE) - 1.0;
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

    // The invariants "resolve is a real urge yet stays below the flat rally" (both the
    // flat `CHASE_RESOLVE_MAX_SPEED_BONUS < FLAG_RALLY_SPEED_BONUS` and the scaled
    // `CHASE_RESOLVE_MAX_SPEED_BONUS * CHASE_RESOLVE_COMMITMENT_SCALE_MAX <
    // FLAG_RALLY_SPEED_BONUS`) and "commitment never inverts the resolve" are all
    // enforced at compile time by the `const _: () = assert!(..)` blocks above, so a
    // runtime test would only assert a constant clippy already proves.
}
