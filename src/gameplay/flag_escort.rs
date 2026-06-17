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

use crate::gameplay::chase_resolve::CHASE_RESOLVE_MAX_SPEED_BONUS;

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

/// Speed multiplier a non-carrier earns while one of its own team's cars is hauling
/// the enemy flag home.
///
/// Returns `1.0` (no urge) when no car on the team holds the enemy flag, or when the
/// car is itself the flag carrier (the escort is for clearing the path, never for a
/// flag run home, so the carrier being shepherded earns nothing while its empty-handed
/// teammates clear the way). While the team holds the enemy flag and the car is
/// empty-handed it carries the flat [`FLAG_ESCORT_SPEED_BONUS`], so the result is
/// always in `1.0..=1.0 + FLAG_ESCORT_SPEED_BONUS`.
#[must_use]
pub fn flag_escort_speed_multiplier(we_hold_enemy_flag: bool, carrying_flag: bool) -> f32 {
    if carrying_flag || !we_hold_enemy_flag {
        return 1.0;
    }
    1.0 + FLAG_ESCORT_SPEED_BONUS
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
    fn a_team_without_the_enemy_flag_earns_no_escort() {
        assert_near(flag_escort_speed_multiplier(false, false), 1.0);
    }

    #[test]
    fn a_team_holding_the_enemy_flag_earns_an_escort() {
        let multiplier = flag_escort_speed_multiplier(true, false);
        assert!(
            multiplier > 1.0 && multiplier <= 1.0 + FLAG_ESCORT_SPEED_BONUS,
            "expected a capped escort, got {multiplier}"
        );
    }

    #[test]
    fn the_escort_is_the_full_flat_bonus_while_we_hold_the_flag() {
        // Unlike the deficit-scaled catch-up, the escort is a flat push: any team with
        // the enemy flag in hand earns the whole bonus, so it never ramps.
        assert_near(
            flag_escort_speed_multiplier(true, false),
            1.0 + FLAG_ESCORT_SPEED_BONUS,
        );
    }

    #[test]
    fn the_shepherded_carrier_never_earns_the_escort() {
        // The very car hauling the enemy flag home earns no escort, so the bonus can
        // never speed a flag run home, leaving the chase balance fully intact while its
        // empty-handed teammates clear the path.
        assert_near(flag_escort_speed_multiplier(true, true), 1.0);
    }

    #[test]
    fn the_carry_exclusion_holds_however_the_flag_situation_reads() {
        // The carry exclusion never depends on the team's flag state, so a carrier is
        // never urged on even should the situation flags read inconsistently.
        assert_near(flag_escort_speed_multiplier(false, true), 1.0);
    }

    // The invariant "escort never exceeds the slow-building chase resolve"
    // (`FLAG_ESCORT_SPEED_BONUS < CHASE_RESOLVE_MAX_SPEED_BONUS`) is enforced at
    // compile time by the `const _: () = assert!(..)` block above, so a runtime test
    // would only assert a constant clippy already proves.
}
