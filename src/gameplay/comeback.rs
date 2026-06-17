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
//!
//! How hard a driver presses the catch-up is its own personality: a keener driver
//! (a higher [`crate::gameplay::virtual_player::VirtualPlayer::corner_throttle`],
//! the commitment axis that already sets how hard it stays on the gas through a
//! corner, how deep it noses a kill home and how tightly it squeezes past a
//! blocker) floors it harder to claw a deficit back, while a disciplined one urges
//! on more gently. The scale is centred on the neutral [`MIN_THROTTLE`] baseline
//! the all-rounder corners on, so the baseline driver, and the human that mirrors
//! it, keep the exact original catch-up; only a roster of distinct AI personalities
//! deviates from it. The off-the-objective-line mirror of the same commitment axis,
//! alongside the greed axis that scales a driver's draft cone and pickup detours.
//! Even the keenest scaled catch-up stays a handout below an earned slipstream tow
//! (compile-asserted), so the anti-snowball urge can never out-reward clean racing.

use crate::gameplay::ctf::CAPTURES_TO_WIN;
use crate::gameplay::slipstream::DRAFT_MAX_SPEED_BONUS;
use crate::gameplay::virtual_player::ai::MIN_THROTTLE;

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

/// How far a driver's catch-up urge scales per unit of cornering commitment away
/// from the neutral [`MIN_THROTTLE`] baseline.
///
/// A keener driver (a higher
/// [`crate::gameplay::virtual_player::VirtualPlayer::corner_throttle`]) presses the
/// catch-up harder, a disciplined one more gently. The catch-up mirror of the same
/// commitment axis that sets how deep a driver noses a kill home
/// (`pursuit_arrive_radius`) and how tightly it squeezes past a blocker on its
/// scoring run (`carrier_juke_offset`), so a keen driver commits to closing a
/// deficit as hard as it commits everywhere else.
const COMEBACK_COMMITMENT_SCALE_GAIN: f32 = 1.5;

/// Floor on the commitment-driven catch-up scale: a safety net so a degenerate or
/// extreme-disciplined `corner_throttle` can never collapse the catch-up to nothing
/// (or invert it). The asserted roster commitment band (`0.15..=0.5`, the range the
/// driver roster holds each
/// [`crate::gameplay::virtual_player::VirtualPlayer::corner_throttle`] to) maps
/// strictly inside this band, so the clamp only ever guards a garbage throttle,
/// never a real driver's personality.
const COMEBACK_COMMITMENT_SCALE_MIN: f32 = 0.7;

/// Ceiling on the commitment-driven catch-up scale: the keen counterpart to the
/// floor, so a degenerate or extreme-reckless `corner_throttle` tops out here rather
/// than scaling the catch-up without bound. Held low enough that even the keenest
/// scaled catch-up stays below an earned slipstream tow (asserted below).
const COMEBACK_COMMITMENT_SCALE_MAX: f32 = 1.3;

/// The catch-up scale is centred on the baseline driver (scale `1.0`, the original
/// uniform catch-up) and never inverts commitment: a keener driver always gets at
/// least as strong an urge as a more disciplined one. Enforced at compile time.
const _: () = assert!(COMEBACK_COMMITMENT_SCALE_MIN < 1.0 && COMEBACK_COMMITMENT_SCALE_MAX > 1.0);

/// Commitment must genuinely strengthen the catch-up, never weaken or flatten it,
/// enforced at compile time.
const _: () = assert!(COMEBACK_COMMITMENT_SCALE_GAIN > 0.0);

/// Even the keenest scaled catch-up must stay below an earned slipstream tow,
/// enforced at compile time, so the personality scaling can never lift the
/// anti-snowball handout into out-rewarding the pace a clean racing line earns nor
/// drift into a power item. The flat [`COMEBACK_MAX_SPEED_BONUS`] ordering asserts
/// (the rally and front-runner bands pin themselves to it) are untouched: this
/// guards only the extra headroom the commitment ceiling opens up.
const _: () =
    assert!(COMEBACK_MAX_SPEED_BONUS * COMEBACK_COMMITMENT_SCALE_MAX < DRAFT_MAX_SPEED_BONUS);

/// Scales a driver's catch-up urge by its cornering commitment.
///
/// A driver cornering on the neutral [`MIN_THROTTLE`] floor scales by exactly `1.0`,
/// so the all-rounder baseline and the human's mirror keep the original uniform
/// catch-up untouched. A keener driver (a higher `corner_throttle`) scales up toward
/// [`COMEBACK_COMMITMENT_SCALE_MAX`]; a disciplined one down toward
/// [`COMEBACK_COMMITMENT_SCALE_MIN`]. The affine map is clamped to the
/// [[`COMEBACK_COMMITMENT_SCALE_MIN`], [`COMEBACK_COMMITMENT_SCALE_MAX`]] band as a
/// safety net for a degenerate throttle.
#[must_use]
fn comeback_commitment_scale(corner_throttle: f32) -> f32 {
    let keen = (corner_throttle - MIN_THROTTLE) * COMEBACK_COMMITMENT_SCALE_GAIN;
    (1.0 + keen).clamp(COMEBACK_COMMITMENT_SCALE_MIN, COMEBACK_COMMITMENT_SCALE_MAX)
}

/// Speed multiplier a non-carrier earns from its team trailing on captures, scaled
/// by the driver's cornering commitment `corner_throttle`.
///
/// Returns `1.0` (no urge) when the team is level or ahead, or when the car is a
/// flag carrier (the catch-up is for the chase, never for a flag run home). When
/// the team trails, the bonus scales linearly with the deficit: a part of the
/// commitment-scaled cap one capture down, building to the full scaled bonus at the
/// largest deficit a live match can hold ([`CAPTURES_TO_WIN`] `- 1`, since a team
/// reaching [`CAPTURES_TO_WIN`] ends the match). A deficit beyond that band is
/// clamped.
///
/// The cap itself is scaled by the driver's commitment (see
/// [`comeback_commitment_scale`]): a driver on the neutral [`MIN_THROTTLE`] floor
/// earns exactly [`COMEBACK_MAX_SPEED_BONUS`], so the all-rounder baseline and the
/// human's mirror (which pass `MIN_THROTTLE`) keep the original uniform catch-up; a
/// keener driver claws back harder, a disciplined one more gently. The scaled cap
/// stays strictly below an earned slipstream tow (compile-asserted), so the result
/// is always in `1.0..=1.0 + COMEBACK_MAX_SPEED_BONUS * COMEBACK_COMMITMENT_SCALE_MAX`.
#[must_use]
pub fn comeback_speed_multiplier(
    own_captures: u32,
    enemy_captures: u32,
    carrying_flag: bool,
    corner_throttle: f32,
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
    let cap = COMEBACK_MAX_SPEED_BONUS * comeback_commitment_scale(corner_throttle);
    cap.mul_add(fraction, 1.0)
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

    /// A keen, reckless driver and a disciplined one, both well inside the asserted
    /// roster commitment band (`0.15..=0.5`), so the tests read the real scaling
    /// without coupling to the private roster profiles. The baseline is
    /// [`MIN_THROTTLE`], the neutral throttle the all-rounder and the human mirror.
    const KEEN_THROTTLE: f32 = 0.45;
    const DISCIPLINED_THROTTLE: f32 = 0.2;

    #[test]
    fn a_level_team_earns_no_catch_up() {
        assert_near(comeback_speed_multiplier(1, 1, false, MIN_THROTTLE), 1.0);
    }

    #[test]
    fn a_leading_team_earns_no_catch_up() {
        assert_near(comeback_speed_multiplier(2, 0, false, MIN_THROTTLE), 1.0);
    }

    #[test]
    fn a_trailing_team_earns_a_catch_up() {
        let multiplier = comeback_speed_multiplier(0, 1, false, MIN_THROTTLE);
        assert!(
            multiplier > 1.0 && multiplier <= 1.0 + COMEBACK_MAX_SPEED_BONUS,
            "expected a capped catch-up, got {multiplier}"
        );
    }

    #[test]
    fn the_catch_up_strengthens_with_the_deficit() {
        let one_down = comeback_speed_multiplier(0, 1, false, MIN_THROTTLE);
        let two_down = comeback_speed_multiplier(0, 2, false, MIN_THROTTLE);
        assert!(
            two_down > one_down,
            "a larger deficit should urge harder: two_down={two_down}, one_down={one_down}"
        );
    }

    #[test]
    fn the_baseline_driver_keeps_the_original_uniform_catch_up() {
        // The all-rounder and the human corner on the neutral MIN_THROTTLE floor, so
        // they keep the exact pre-personality catch-up: the full deficit earns the
        // unscaled cap, never a notch more or less.
        let full = comeback_speed_multiplier(0, CAPTURES_TO_WIN - 1, false, MIN_THROTTLE);
        assert_near(full, 1.0 + COMEBACK_MAX_SPEED_BONUS);
    }

    #[test]
    fn a_deficit_beyond_the_band_is_capped() {
        let capped = comeback_speed_multiplier(0, CAPTURES_TO_WIN + 5, false, MIN_THROTTLE);
        assert_near(capped, 1.0 + COMEBACK_MAX_SPEED_BONUS);
    }

    #[test]
    fn a_flag_carrier_never_earns_the_catch_up() {
        // Even at the largest deficit, and even a reckless carrier, gets no urge: the
        // catch-up can never speed a flag run home, leaving the chase balance intact.
        assert_near(
            comeback_speed_multiplier(0, CAPTURES_TO_WIN - 1, true, KEEN_THROTTLE),
            1.0,
        );
    }

    // The invariant "the keenest scaled catch-up never exceeds an earned slipstream
    // tow" (`COMEBACK_MAX_SPEED_BONUS * COMEBACK_COMMITMENT_SCALE_MAX <
    // DRAFT_MAX_SPEED_BONUS`) is enforced at compile time by the `const _: () =
    // assert!(..)` block above, so a runtime test would only assert a constant
    // clippy already proves.

    #[test]
    fn the_catch_up_ramps_rather_than_snapping_to_full() {
        // One capture down must be a genuine part bonus, strictly between nothing
        // and the full cap, so the urge builds with the deficit rather than
        // snapping straight to its top rate the instant a team falls behind.
        let one_down = comeback_speed_multiplier(0, 1, false, MIN_THROTTLE);
        assert!(
            one_down > 1.0 && one_down < 1.0 + COMEBACK_MAX_SPEED_BONUS,
            "one capture down should be a part bonus, got {one_down}"
        );
    }

    #[test]
    fn the_commitment_scale_is_neutral_at_the_baseline_throttle() {
        // The all-rounder (and the human that mirrors it) corner on MIN_THROTTLE, so
        // the scale is exactly 1.0 there and the baseline catch-up is untouched.
        assert_near(comeback_commitment_scale(MIN_THROTTLE), 1.0);
    }

    #[test]
    fn a_keener_driver_claws_back_harder_than_the_baseline() {
        // The personality lever: at the same deficit a keener, gas-committed driver
        // earns a stronger catch-up than the neutral baseline, so it presses the
        // equaliser harder.
        let baseline = comeback_speed_multiplier(0, CAPTURES_TO_WIN - 1, false, MIN_THROTTLE);
        let keen = comeback_speed_multiplier(0, CAPTURES_TO_WIN - 1, false, KEEN_THROTTLE);
        assert!(
            keen > baseline,
            "a keener driver should claw back harder: keen={keen}, baseline={baseline}"
        );
    }

    #[test]
    fn a_disciplined_driver_claws_back_more_gently_than_the_baseline() {
        // The mirror of the keen case: a disciplined driver still earns a real
        // catch-up (above 1.0) but a gentler one than the neutral baseline.
        let baseline = comeback_speed_multiplier(0, CAPTURES_TO_WIN - 1, false, MIN_THROTTLE);
        let disciplined =
            comeback_speed_multiplier(0, CAPTURES_TO_WIN - 1, false, DISCIPLINED_THROTTLE);
        assert!(
            disciplined < baseline && disciplined > 1.0,
            "a disciplined driver should still claw back, but gentler: \
             disciplined={disciplined}, baseline={baseline}"
        );
    }

    #[test]
    fn the_keenest_roster_driver_stays_below_a_slipstream_tow() {
        // The roster caps a driver's cornering commitment at 0.5 (asserted in
        // spawn.rs). Even that keenest driver, fully behind, must earn a catch-up
        // strictly below a perfect slipstream tow, so the anti-snowball handout can
        // never out-reward the pace a clean racing line earns.
        let keenest = comeback_speed_multiplier(0, CAPTURES_TO_WIN - 1, false, 0.5);
        assert!(
            keenest < 1.0 + DRAFT_MAX_SPEED_BONUS,
            "the keenest scaled catch-up ({keenest}) must stay below a slipstream tow \
             ({})",
            1.0 + DRAFT_MAX_SPEED_BONUS
        );
        assert!(
            keenest > 1.0 + COMEBACK_MAX_SPEED_BONUS,
            "the keenest driver should still out-claw the baseline cap: {keenest}"
        );
    }

    #[test]
    fn the_commitment_scale_clamps_a_degenerate_throttle() {
        // A garbage throttle far outside the roster band can never collapse the
        // catch-up to nothing nor blow it out: the clamp pins it to the band.
        assert_near(
            comeback_commitment_scale(-100.0),
            COMEBACK_COMMITMENT_SCALE_MIN,
        );
        assert_near(
            comeback_commitment_scale(100.0),
            COMEBACK_COMMITMENT_SCALE_MAX,
        );
    }
}
