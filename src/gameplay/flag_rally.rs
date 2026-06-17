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

use crate::gameplay::comeback::COMEBACK_MAX_SPEED_BONUS;

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

/// Speed multiplier a non-carrier earns while its own team's flag is in enemy
/// hands.
///
/// Returns `1.0` (no urge) when the team's flag is safe (home or already
/// recovered), or when the car is itself a flag carrier (the rally is for the
/// chase, never for a flag run home, so a double-steal carrier earns nothing while
/// its empty-handed teammates still rally). While the flag is out and the car is
/// empty-handed it carries the flat [`FLAG_RALLY_SPEED_BONUS`], so the result is
/// always in `1.0..=1.0 + FLAG_RALLY_SPEED_BONUS`.
#[must_use]
pub fn flag_rally_speed_multiplier(own_flag_stolen: bool, carrying_flag: bool) -> f32 {
    if carrying_flag || !own_flag_stolen {
        return 1.0;
    }
    1.0 + FLAG_RALLY_SPEED_BONUS
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
    fn a_team_with_its_flag_safe_earns_no_rally() {
        assert_near(flag_rally_speed_multiplier(false, false), 1.0);
    }

    #[test]
    fn a_team_whose_flag_is_stolen_earns_a_rally() {
        let multiplier = flag_rally_speed_multiplier(true, false);
        assert!(
            multiplier > 1.0 && multiplier <= 1.0 + FLAG_RALLY_SPEED_BONUS,
            "expected a capped rally, got {multiplier}"
        );
    }

    #[test]
    fn the_rally_is_the_full_flat_bonus_while_the_flag_is_out() {
        // Unlike the deficit-scaled catch-up, the rally is a flat push: any team
        // with its flag out earns the whole bonus, so it never ramps.
        assert_near(
            flag_rally_speed_multiplier(true, false),
            1.0 + FLAG_RALLY_SPEED_BONUS,
        );
    }

    #[test]
    fn a_flag_carrier_never_earns_the_rally() {
        // A double steal: this car hauls the enemy flag home while its own flag is
        // also out. The carrier earns no rally, so the bonus can never speed a flag
        // run home, leaving the chase balance fully intact.
        assert_near(flag_rally_speed_multiplier(true, true), 1.0);
    }

    #[test]
    fn a_carrier_whose_flag_is_safe_also_earns_no_rally() {
        // The carry exclusion holds whether or not the team's own flag is out, so a
        // carrier is never urged on under any flag situation.
        assert_near(flag_rally_speed_multiplier(false, true), 1.0);
    }

    // The invariant "rally never exceeds the strategic catch-up"
    // (`FLAG_RALLY_SPEED_BONUS < COMEBACK_MAX_SPEED_BONUS`) is enforced at compile
    // time by the `const _: () = assert!(..)` block above, so a runtime test would
    // only assert a constant clippy already proves.
}
