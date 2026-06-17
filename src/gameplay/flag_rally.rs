//! Flag-recovery rally: the catch-up urge a team's cars find the moment an enemy
//! snatches their flag, to keep a steal from coasting into a capture.
//!
//! A classic capture-the-flag beat in the Death Rally mould: the instant your flag
//! is lifted, the whole side scrambles to run the thief down before it reaches its
//! base. Modelled here as a small, capped speed bonus a car carries while its own
//! flag is in enemy hands, the speed-feel mirror of the tactical lift the field
//! already gives the same situation, where a stolen flag jumps a sabotage to its
//! flag-chase value (see [`crate::gameplay::pickup::PickupKind::virtual_player_priority_for_context`])
//! so a defender breaks off to slow the runner. Where that prices a *pickup*, this
//! prices the *pace*. The bonus is read by both movement systems, the human's
//! `car_movement_system` and the field's `virtual_player_drive_system`, so the
//! human and the AI rally on the identical terms: whichever side's flag is out is
//! the side that finds the urge.
//!
//! A flag carrier never earns the rally: the bonus is for the cars chasing the
//! thief down, never for speeding a flag run home (a side caught in a double steal
//! still rallies its empty-handed cars, but not the one hauling the enemy flag).
//! That is flavour, but it is also what keeps the mechanic balance-safe. The rally
//! can only ever speed a *non-carrier*, so it can never let a team's own flag run
//! outpace the field; the tuned "even the slowest chaser outpaces the fastest
//! carrier" chase balance is left fully intact, exactly as the catch-up
//! ([`crate::gameplay::comeback`]) and the slipstream tow
//! ([`crate::gameplay::slipstream`]) leave it. Unlike the catch-up, the rally is
//! transient: it lasts only while the flag is genuinely out, clearing the instant
//! the flag is captured, dropped or returned home.
//!
//! How hard a driver presses the rally is its own personality: a keener driver (a
//! higher [`crate::gameplay::virtual_player::VirtualPlayer::corner_throttle`], the
//! commitment axis that already sets how hard it stays on the gas through a corner,
//! how deep it noses a kill home and how tightly it squeezes past a blocker) floors
//! it harder to run a thief down, while a disciplined one rallies more gently. The
//! scale is centred on the neutral [`MIN_THROTTLE`] baseline the all-rounder corners
//! on, so the baseline driver, and the human that mirrors it, keep the exact original
//! flat rally; only a roster of distinct AI personalities deviates from it. The
//! flag-steal mirror of the same commitment scaling the strategic catch-up
//! ([`crate::gameplay::comeback::comeback_speed_multiplier`]) already applies, held a
//! notch gentler so even the keenest scaled rally stays below the baseline catch-up
//! (compile-asserted), keeping the feel-bonus hierarchy flag-rally < comeback intact.

use crate::gameplay::comeback::COMEBACK_MAX_SPEED_BONUS;
use crate::gameplay::virtual_player::ai::MIN_THROTTLE;

/// Fraction the flag-recovery rally adds to a chasing car's speed while its team's
/// flag is in enemy hands.
///
/// A modest urge, not a power item: pitched below the strategic catch-up
/// ([`COMEBACK_MAX_SPEED_BONUS`]) because the rally is a flat, always-on push for
/// the duration of a single steal rather than a deficit-scaled handout, so the
/// smaller of the two feel bonuses keeps a transient situational nudge from
/// out-urging the season-long comeback. Well below a nitro burst, so chasing a
/// thief stays a manoeuvre the defenders earn by positioning rather than a free
/// sprint.
pub const FLAG_RALLY_SPEED_BONUS: f32 = 0.06;

/// The rally must be a real urge yet stay below the strategic catch-up, enforced
/// at compile time, so a flat steal-window push never out-urges the deficit-scaled
/// comeback and can never drift into a power item. Pins the feel-bonus hierarchy
/// flag-rally < comeback < slipstream the three modules build out.
const _: () =
    assert!(FLAG_RALLY_SPEED_BONUS > 0.0 && FLAG_RALLY_SPEED_BONUS < COMEBACK_MAX_SPEED_BONUS);

/// How far a driver's flag-recovery rally scales per unit of cornering commitment
/// away from the neutral [`MIN_THROTTLE`] baseline.
///
/// A keener driver (a higher
/// [`crate::gameplay::virtual_player::VirtualPlayer::corner_throttle`]) presses the
/// rally harder, a disciplined one more gently. The flag-steal mirror of the same
/// commitment axis that scales the strategic catch-up
/// ([`crate::gameplay::comeback::comeback_speed_multiplier`]); it reuses the
/// catch-up's gain so a driver reads as keen or disciplined identically on both the
/// captures and the steal lever, the same way a corner, a kill run-down and a
/// scoring-run juke all flex on the one commitment axis.
const FLAG_RALLY_COMMITMENT_SCALE_GAIN: f32 = 1.5;

/// Floor on the commitment-driven rally scale: a safety net so a degenerate or
/// extreme-disciplined `corner_throttle` can never collapse the rally to nothing
/// (or invert it). The asserted roster commitment band (`0.15..=0.5`, the range the
/// driver roster holds each
/// [`crate::gameplay::virtual_player::VirtualPlayer::corner_throttle`] to) maps
/// strictly inside this band, so the clamp only ever guards a garbage throttle,
/// never a real driver's personality.
const FLAG_RALLY_COMMITMENT_SCALE_MIN: f32 = 0.7;

/// Ceiling on the commitment-driven rally scale: the keen counterpart to the floor,
/// so a degenerate or extreme-reckless `corner_throttle` tops out here rather than
/// scaling the rally without bound. Held low enough that even the keenest scaled
/// rally stays below the baseline strategic catch-up (asserted below).
const FLAG_RALLY_COMMITMENT_SCALE_MAX: f32 = 1.3;

/// The rally scale is centred on the baseline driver (scale `1.0`, the original
/// flat rally) and never inverts commitment: a keener driver always gets at least
/// as strong an urge as a more disciplined one. Enforced at compile time.
const _: () =
    assert!(FLAG_RALLY_COMMITMENT_SCALE_MIN < 1.0 && FLAG_RALLY_COMMITMENT_SCALE_MAX > 1.0);

/// Commitment must genuinely strengthen the rally, never weaken or flatten it,
/// enforced at compile time.
const _: () = assert!(FLAG_RALLY_COMMITMENT_SCALE_GAIN > 0.0);

/// Even the keenest scaled rally must stay below the baseline strategic catch-up,
/// enforced at compile time, so the personality scaling can never lift a transient
/// steal-window push past the deficit-scaled comeback nor drift into a power item.
/// The flat [`FLAG_RALLY_SPEED_BONUS`] `<` [`COMEBACK_MAX_SPEED_BONUS`] ordering
/// above is untouched: this guards only the extra headroom the commitment ceiling
/// opens up.
const _: () =
    assert!(FLAG_RALLY_SPEED_BONUS * FLAG_RALLY_COMMITMENT_SCALE_MAX < COMEBACK_MAX_SPEED_BONUS);

/// Scales a driver's flag-recovery rally by its cornering commitment.
///
/// A driver cornering on the neutral [`MIN_THROTTLE`] floor scales by exactly `1.0`,
/// so the all-rounder baseline and the human's mirror keep the original flat rally
/// untouched. A keener driver (a higher `corner_throttle`) scales up toward
/// [`FLAG_RALLY_COMMITMENT_SCALE_MAX`]; a disciplined one down toward
/// [`FLAG_RALLY_COMMITMENT_SCALE_MIN`]. The affine map is clamped to the
/// [[`FLAG_RALLY_COMMITMENT_SCALE_MIN`], [`FLAG_RALLY_COMMITMENT_SCALE_MAX`]] band as
/// a safety net for a degenerate throttle.
#[must_use]
fn flag_rally_commitment_scale(corner_throttle: f32) -> f32 {
    let keen = (corner_throttle - MIN_THROTTLE) * FLAG_RALLY_COMMITMENT_SCALE_GAIN;
    (1.0 + keen).clamp(
        FLAG_RALLY_COMMITMENT_SCALE_MIN,
        FLAG_RALLY_COMMITMENT_SCALE_MAX,
    )
}

/// Speed multiplier a non-carrier earns while its own team's flag is in enemy
/// hands, scaled by the driver's cornering commitment `corner_throttle`.
///
/// Returns `1.0` (no urge) when the team's flag is safe (home or already
/// recovered), or when the car is itself a flag carrier (the rally is for the
/// chase, never for a flag run home, so a double-steal carrier earns nothing while
/// its empty-handed teammates still rally).
///
/// While the flag is out and the car is empty-handed it carries the
/// [`FLAG_RALLY_SPEED_BONUS`], scaled by the driver's commitment (see
/// [`flag_rally_commitment_scale`]): a driver on the neutral [`MIN_THROTTLE`] floor
/// earns exactly the flat bonus, so the all-rounder baseline and the human's mirror
/// (which pass `MIN_THROTTLE`) keep the original flat rally; a keener driver runs the
/// thief down harder, a disciplined one more gently. The scaled bonus stays strictly
/// below the baseline strategic catch-up (compile-asserted), so the result is always
/// in `1.0..=1.0 + FLAG_RALLY_SPEED_BONUS * FLAG_RALLY_COMMITMENT_SCALE_MAX`.
#[must_use]
pub fn flag_rally_speed_multiplier(
    own_flag_stolen: bool,
    carrying_flag: bool,
    corner_throttle: f32,
) -> f32 {
    if carrying_flag || !own_flag_stolen {
        return 1.0;
    }
    FLAG_RALLY_SPEED_BONUS.mul_add(flag_rally_commitment_scale(corner_throttle), 1.0)
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
    fn a_team_with_its_flag_safe_earns_no_rally() {
        assert_near(flag_rally_speed_multiplier(false, false, MIN_THROTTLE), 1.0);
    }

    #[test]
    fn a_team_whose_flag_is_stolen_earns_a_rally() {
        let multiplier = flag_rally_speed_multiplier(true, false, MIN_THROTTLE);
        assert!(
            multiplier > 1.0 && multiplier <= 1.0 + FLAG_RALLY_SPEED_BONUS,
            "expected a capped rally, got {multiplier}"
        );
    }

    #[test]
    fn the_baseline_driver_keeps_the_original_flat_rally() {
        // The all-rounder and the human corner on the neutral MIN_THROTTLE floor, so
        // they keep the exact pre-personality rally: the full flat bonus, never a
        // notch more or less, whether or not a steal is in progress.
        assert_near(
            flag_rally_speed_multiplier(true, false, MIN_THROTTLE),
            1.0 + FLAG_RALLY_SPEED_BONUS,
        );
    }

    #[test]
    fn a_flag_carrier_never_earns_the_rally() {
        // A double steal: this car hauls the enemy flag home while its own flag is
        // also out. Even a reckless carrier earns no rally, so the bonus can never
        // speed a flag run home, leaving the chase balance fully intact.
        assert_near(flag_rally_speed_multiplier(true, true, KEEN_THROTTLE), 1.0);
    }

    #[test]
    fn a_carrier_whose_flag_is_safe_also_earns_no_rally() {
        // The carry exclusion holds whether or not the team's own flag is out, so a
        // carrier is never urged on under any flag situation.
        assert_near(flag_rally_speed_multiplier(false, true, MIN_THROTTLE), 1.0);
    }

    #[test]
    fn the_commitment_scale_is_neutral_at_the_baseline_throttle() {
        // The all-rounder (and the human that mirrors it) corner on MIN_THROTTLE, so
        // the scale is exactly 1.0 there and the baseline rally is untouched.
        assert_near(flag_rally_commitment_scale(MIN_THROTTLE), 1.0);
    }

    #[test]
    fn a_keener_driver_rallies_harder_than_the_baseline() {
        // The personality lever: with the same steal in progress a keener,
        // gas-committed driver earns a stronger rally than the neutral baseline, so it
        // presses the thief harder.
        let baseline = flag_rally_speed_multiplier(true, false, MIN_THROTTLE);
        let keen = flag_rally_speed_multiplier(true, false, KEEN_THROTTLE);
        assert!(
            keen > baseline,
            "a keener driver should rally harder: keen={keen}, baseline={baseline}"
        );
    }

    #[test]
    fn a_disciplined_driver_rallies_more_gently_than_the_baseline() {
        // The mirror of the keen case: a disciplined driver still earns a real rally
        // (above 1.0) but a gentler one than the neutral baseline.
        let baseline = flag_rally_speed_multiplier(true, false, MIN_THROTTLE);
        let disciplined = flag_rally_speed_multiplier(true, false, DISCIPLINED_THROTTLE);
        assert!(
            disciplined < baseline && disciplined > 1.0,
            "a disciplined driver should still rally, but gentler: \
             disciplined={disciplined}, baseline={baseline}"
        );
    }

    #[test]
    fn the_keenest_roster_driver_stays_below_the_baseline_catch_up() {
        // The roster caps a driver's cornering commitment at 0.5 (asserted in
        // spawn.rs). Even that keenest driver, mid-steal, must earn a rally strictly
        // below the baseline strategic catch-up, so a transient steal-window push can
        // never out-urge the deficit-scaled comeback.
        let keenest = flag_rally_speed_multiplier(true, false, 0.5);
        assert!(
            keenest < 1.0 + COMEBACK_MAX_SPEED_BONUS,
            "the keenest scaled rally ({keenest}) must stay below the baseline catch-up ({})",
            1.0 + COMEBACK_MAX_SPEED_BONUS
        );
        assert!(
            keenest > 1.0 + FLAG_RALLY_SPEED_BONUS,
            "the keenest driver should still out-rally the flat baseline bonus: {keenest}"
        );
    }

    #[test]
    fn the_commitment_scale_clamps_a_degenerate_throttle() {
        // A garbage throttle far outside the roster band can never collapse the rally
        // to nothing nor blow it out: the clamp pins it to the band.
        assert_near(
            flag_rally_commitment_scale(-100.0),
            FLAG_RALLY_COMMITMENT_SCALE_MIN,
        );
        assert_near(
            flag_rally_commitment_scale(100.0),
            FLAG_RALLY_COMMITMENT_SCALE_MAX,
        );
    }

    // The invariant "the keenest scaled rally never exceeds the baseline catch-up"
    // (`FLAG_RALLY_SPEED_BONUS * FLAG_RALLY_COMMITMENT_SCALE_MAX <
    // COMEBACK_MAX_SPEED_BONUS`) and the flat "rally never exceeds the strategic
    // catch-up" ordering are both enforced at compile time by the `const _: () =
    // assert!(..)` blocks above, so a runtime test would only assert a constant
    // clippy already proves.
}
