//! Flag escort: the urge a team's empty-handed cars find while one of their own is
//! hauling the enemy flag home, to clear the path and shepherd the capture in.
//!
//! The offensive mirror of the flag-recovery rally ([`crate::gameplay::flag_rally`]):
//! where that scrambles a *robbed* side's chasers to run a thief down, this rallies a
//! *raiding* side's empty-handed cars to escort their own carrier home. It is the
//! speed-feel twin of the offensive lift the pickup layer already grants the same
//! situation, where a side hauling the enemy flag prices a sabotage as getaway cover
//! and a shield as getaway armour (see
//! [`crate::gameplay::pickup::PickupKind::virtual_player_priority_for_context`]) so a
//! teammate breaks off to shepherd the run. Where that prices a *pickup*, this prices
//! the *pace*, and it reinforces the field's existing positional escort, where a
//! teammate already peels off to lead the carrier home. The bonus is read by both
//! movement systems, the human's `car_movement_system` and the field's
//! `virtual_player_drive_system`, so the human and the AI escort on the identical
//! terms: whichever side holds the enemy flag is the side that finds the urge.
//!
//! A flag carrier never earns the escort: the bonus is for the cars clearing the path,
//! never for speeding the flag run home itself (a side caught in a double steal still
//! escorts with its empty-handed cars, but not the one hauling the enemy flag). That
//! is flavour, but it is also what keeps the mechanic balance-safe. The escort can
//! only ever speed a *non-carrier*, so it can never let a team's own flag run outpace
//! the field; the tuned "even the slowest chaser outpaces the fastest carrier" chase
//! balance is left fully intact, exactly as the flag-recovery rally
//! ([`crate::gameplay::flag_rally`]), the catch-up ([`crate::gameplay::comeback`]) and
//! the slipstream tow ([`crate::gameplay::slipstream`]) leave it. Like the rally, the
//! escort is transient: it lasts only while one of the side's cars genuinely holds the
//! enemy flag, clearing the instant that flag is captured, dropped or returned.
//!
//! How hard a driver presses the escort is its own personality: a keener driver (a
//! higher [`crate::gameplay::virtual_player::VirtualPlayer::corner_throttle`], the
//! commitment axis that already sets how hard it stays on the gas through a corner, how
//! deep it noses a kill home and how hard it runs a thief down) floors it harder to
//! shepherd the capture in, while a disciplined one escorts more gently. The scale is
//! centred on the neutral [`MIN_THROTTLE`] baseline the all-rounder corners on, so the
//! baseline driver, and the human that mirrors it, keep the exact original flat escort;
//! only a roster of distinct AI personalities deviates from it. The flag-carry mirror
//! of the same commitment scaling the flag-recovery rally
//! ([`crate::gameplay::flag_rally::flag_rally_speed_multiplier`]) already applies, but
//! pitched gentler still: flag escort sits at the very bottom of the feel-bonus
//! hierarchy with the least headroom of any lever (escort resolve just below, chase
//! resolve just above), so its commitment band is held tighter than the rally's,
//! keeping even the keenest scaled escort below the chase resolve (compile-asserted)
//! and even the most disciplined above the escort resolve it complements.

use crate::gameplay::chase_resolve::CHASE_RESOLVE_MAX_SPEED_BONUS;
use crate::gameplay::virtual_player::ai::MIN_THROTTLE;

/// Fraction the escort adds to a clearing car's speed while its team holds the enemy
/// flag.
///
/// The gentlest of the flag-in-flight feel bonuses by design: shepherding a capture
/// already under way is the least urgent push of the lot, so it is pitched below even
/// the slow-building chase resolve ([`CHASE_RESOLVE_MAX_SPEED_BONUS`]), the same
/// precedence the pickup layer keeps when it rates getaway cover below a flag chase
/// (defending or recovering a steal outranks covering our own run). Extends the
/// feel-bonus hierarchy to flag-escort < chase-resolve < flag-rally < comeback <
/// slipstream. Well below a nitro burst, so escorting a carrier stays a manoeuvre the
/// teammates earn by positioning rather than a free sprint.
pub const FLAG_ESCORT_SPEED_BONUS: f32 = 0.04;

/// The escort must be a real urge yet stay below every defensive recovery urge,
/// enforced at compile time, so covering our own run never out-urges running a thief
/// down and can never drift into a power item. Pins the bottom of the feel-bonus
/// hierarchy: flag-escort < chase-resolve < flag-rally < comeback < slipstream.
const _: () = assert!(
    FLAG_ESCORT_SPEED_BONUS > 0.0 && FLAG_ESCORT_SPEED_BONUS < CHASE_RESOLVE_MAX_SPEED_BONUS
);

/// How far a driver's flag escort scales per unit of cornering commitment away from
/// the neutral [`MIN_THROTTLE`] baseline.
///
/// A keener driver (a higher
/// [`crate::gameplay::virtual_player::VirtualPlayer::corner_throttle`]) presses the
/// escort harder, a disciplined one more gently. The flag-carry mirror of the same
/// commitment axis that scales the flag-recovery rally
/// ([`crate::gameplay::flag_rally::flag_rally_speed_multiplier`]), but pitched gentler
/// (the rally uses `1.5`): flag escort has the least headroom of any feel bonus, so a
/// hotter gain would push the keenest scaled escort past the chase resolve just above
/// it. At this gain the asserted roster commitment band maps exactly onto the
/// [[`FLAG_ESCORT_COMMITMENT_SCALE_MIN`], [`FLAG_ESCORT_COMMITMENT_SCALE_MAX`]] band,
/// so the clamp only ever guards a garbage throttle.
const FLAG_ESCORT_COMMITMENT_SCALE_GAIN: f32 = 1.0;

/// Floor on the commitment-driven escort scale: a safety net so a degenerate or
/// extreme-disciplined `corner_throttle` can never collapse the escort to nothing (or
/// invert it), and held high enough that even the most disciplined scaled escort stays
/// above the escort resolve it complements. The asserted roster commitment band
/// (`0.15..=0.5`, the range the driver roster holds each
/// [`crate::gameplay::virtual_player::VirtualPlayer::corner_throttle`] to) maps
/// strictly inside this band, so the clamp only ever guards a garbage throttle, never a
/// real driver's personality.
const FLAG_ESCORT_COMMITMENT_SCALE_MIN: f32 = 0.8;

/// Ceiling on the commitment-driven escort scale: the keen counterpart to the floor,
/// so a degenerate or extreme-reckless `corner_throttle` tops out here rather than
/// scaling the escort without bound. Held low enough that even the keenest scaled
/// escort stays below the chase resolve just above it (asserted below).
const FLAG_ESCORT_COMMITMENT_SCALE_MAX: f32 = 1.2;

/// The escort scale is centred on the baseline driver (scale `1.0`, the original flat
/// escort) and never inverts commitment: a keener driver always gets at least as strong
/// an urge as a more disciplined one. Enforced at compile time.
const _: () =
    assert!(FLAG_ESCORT_COMMITMENT_SCALE_MIN < 1.0 && FLAG_ESCORT_COMMITMENT_SCALE_MAX > 1.0);

/// Commitment must genuinely strengthen the escort, never weaken or flatten it,
/// enforced at compile time.
const _: () = assert!(FLAG_ESCORT_COMMITMENT_SCALE_GAIN > 0.0);

/// Even the keenest scaled escort must stay below the slow-building chase resolve,
/// enforced at compile time, so the personality scaling can never lift covering our own
/// run past a defensive recovery urge nor drift into a power item. The flat
/// [`FLAG_ESCORT_SPEED_BONUS`] `<` [`CHASE_RESOLVE_MAX_SPEED_BONUS`] ordering above is
/// untouched: this guards only the extra headroom the commitment ceiling opens up.
const _: () = assert!(
    FLAG_ESCORT_SPEED_BONUS * FLAG_ESCORT_COMMITMENT_SCALE_MAX < CHASE_RESOLVE_MAX_SPEED_BONUS
);

/// Scales a driver's flag escort by its cornering commitment.
///
/// A driver cornering on the neutral [`MIN_THROTTLE`] floor scales by exactly `1.0`, so
/// the all-rounder baseline and the human's mirror keep the original flat escort
/// untouched. A keener driver (a higher `corner_throttle`) scales up toward
/// [`FLAG_ESCORT_COMMITMENT_SCALE_MAX`]; a disciplined one down toward
/// [`FLAG_ESCORT_COMMITMENT_SCALE_MIN`]. The affine map is clamped to the
/// [[`FLAG_ESCORT_COMMITMENT_SCALE_MIN`], [`FLAG_ESCORT_COMMITMENT_SCALE_MAX`]] band as
/// a safety net for a degenerate throttle.
#[must_use]
fn flag_escort_commitment_scale(corner_throttle: f32) -> f32 {
    let keen = (corner_throttle - MIN_THROTTLE) * FLAG_ESCORT_COMMITMENT_SCALE_GAIN;
    (1.0 + keen).clamp(
        FLAG_ESCORT_COMMITMENT_SCALE_MIN,
        FLAG_ESCORT_COMMITMENT_SCALE_MAX,
    )
}

/// Speed multiplier a non-carrier earns while one of its own team's cars is hauling
/// the enemy flag home, scaled by the driver's cornering commitment `corner_throttle`.
///
/// Returns `1.0` (no urge) when no car on the team holds the enemy flag, or when the
/// car is itself the flag carrier (the escort is for clearing the path, never for a
/// flag run home, so the carrier being shepherded earns nothing while its empty-handed
/// teammates clear the way).
///
/// While the team holds the enemy flag and the car is empty-handed it carries the
/// [`FLAG_ESCORT_SPEED_BONUS`], scaled by the driver's commitment (see
/// [`flag_escort_commitment_scale`]): a driver on the neutral [`MIN_THROTTLE`] floor
/// earns exactly the flat bonus, so the all-rounder baseline and the human's mirror
/// (which pass `MIN_THROTTLE`) keep the original flat escort; a keener driver shepherds
/// the carrier home harder, a disciplined one more gently. The scaled bonus stays
/// strictly below the slow-building chase resolve (compile-asserted), so the result is
/// always in `1.0..=1.0 + FLAG_ESCORT_SPEED_BONUS * FLAG_ESCORT_COMMITMENT_SCALE_MAX`.
#[must_use]
pub fn flag_escort_speed_multiplier(
    we_hold_enemy_flag: bool,
    carrying_flag: bool,
    corner_throttle: f32,
) -> f32 {
    if carrying_flag || !we_hold_enemy_flag {
        return 1.0;
    }
    FLAG_ESCORT_SPEED_BONUS.mul_add(flag_escort_commitment_scale(corner_throttle), 1.0)
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
    fn a_team_without_the_enemy_flag_earns_no_escort() {
        assert_near(
            flag_escort_speed_multiplier(false, false, MIN_THROTTLE),
            1.0,
        );
    }

    #[test]
    fn a_team_holding_the_enemy_flag_earns_an_escort() {
        let multiplier = flag_escort_speed_multiplier(true, false, MIN_THROTTLE);
        assert!(
            multiplier > 1.0 && multiplier <= 1.0 + FLAG_ESCORT_SPEED_BONUS,
            "expected a capped escort, got {multiplier}"
        );
    }

    #[test]
    fn the_baseline_driver_keeps_the_original_flat_escort() {
        // The all-rounder and the human corner on the neutral MIN_THROTTLE floor, so
        // they keep the exact pre-personality escort: the full flat bonus, never a notch
        // more or less, while the team holds the enemy flag.
        assert_near(
            flag_escort_speed_multiplier(true, false, MIN_THROTTLE),
            1.0 + FLAG_ESCORT_SPEED_BONUS,
        );
    }

    #[test]
    fn the_shepherded_carrier_never_earns_the_escort() {
        // The very car hauling the enemy flag home earns no escort, so the bonus can
        // never speed a flag run home, leaving the chase balance fully intact while its
        // empty-handed teammates clear the path. Even a reckless carrier earns nothing.
        assert_near(flag_escort_speed_multiplier(true, true, KEEN_THROTTLE), 1.0);
    }

    #[test]
    fn the_carry_exclusion_holds_however_the_flag_situation_reads() {
        // The carry exclusion never depends on the team's flag state, so a carrier is
        // never urged on even should the situation flags read inconsistently.
        assert_near(flag_escort_speed_multiplier(false, true, MIN_THROTTLE), 1.0);
    }

    #[test]
    fn the_commitment_scale_is_neutral_at_the_baseline_throttle() {
        // The all-rounder (and the human that mirrors it) corner on MIN_THROTTLE, so the
        // scale is exactly 1.0 there and the baseline escort is untouched.
        assert_near(flag_escort_commitment_scale(MIN_THROTTLE), 1.0);
    }

    #[test]
    fn a_keener_driver_escorts_harder_than_the_baseline() {
        // The personality lever: with the same capture in flight a keener, gas-committed
        // driver earns a stronger escort than the neutral baseline, so it shepherds the
        // carrier home harder.
        let baseline = flag_escort_speed_multiplier(true, false, MIN_THROTTLE);
        let keen = flag_escort_speed_multiplier(true, false, KEEN_THROTTLE);
        assert!(
            keen > baseline,
            "a keener driver should escort harder: keen={keen}, baseline={baseline}"
        );
    }

    #[test]
    fn a_disciplined_driver_escorts_more_gently_than_the_baseline() {
        // The mirror of the keen case: a disciplined driver still earns a real escort
        // (above 1.0) but a gentler one than the neutral baseline.
        let baseline = flag_escort_speed_multiplier(true, false, MIN_THROTTLE);
        let disciplined = flag_escort_speed_multiplier(true, false, DISCIPLINED_THROTTLE);
        assert!(
            disciplined < baseline && disciplined > 1.0,
            "a disciplined driver should still escort, but gentler: \
             disciplined={disciplined}, baseline={baseline}"
        );
    }

    #[test]
    fn the_keenest_roster_driver_stays_below_the_chase_resolve() {
        // The roster caps a driver's cornering commitment at 0.5 (asserted in spawn.rs).
        // Even that keenest driver must earn an escort strictly below the slow-building
        // chase resolve just above it, so the personality scaling never lifts covering
        // our own run past a defensive recovery urge.
        let keenest = flag_escort_speed_multiplier(true, false, 0.5);
        assert!(
            keenest < 1.0 + CHASE_RESOLVE_MAX_SPEED_BONUS,
            "the keenest scaled escort ({keenest}) must stay below the chase resolve ({})",
            1.0 + CHASE_RESOLVE_MAX_SPEED_BONUS
        );
        assert!(
            keenest > 1.0 + FLAG_ESCORT_SPEED_BONUS,
            "the keenest driver should still out-escort the flat baseline bonus: {keenest}"
        );
    }

    #[test]
    fn the_commitment_scale_clamps_a_degenerate_throttle() {
        // A garbage throttle far outside the roster band can never collapse the escort
        // to nothing nor blow it out: the clamp pins it to the band.
        assert_near(
            flag_escort_commitment_scale(-100.0),
            FLAG_ESCORT_COMMITMENT_SCALE_MIN,
        );
        assert_near(
            flag_escort_commitment_scale(100.0),
            FLAG_ESCORT_COMMITMENT_SCALE_MAX,
        );
    }

    // The invariants "escort never exceeds the slow-building chase resolve" (both the
    // flat `FLAG_ESCORT_SPEED_BONUS < CHASE_RESOLVE_MAX_SPEED_BONUS` and the scaled
    // `FLAG_ESCORT_SPEED_BONUS * FLAG_ESCORT_COMMITMENT_SCALE_MAX <
    // CHASE_RESOLVE_MAX_SPEED_BONUS`) and "commitment never inverts the escort" are all
    // enforced at compile time by the `const _: () = assert!(..)` blocks above, so a
    // runtime test would only assert a constant clippy already proves.
}
