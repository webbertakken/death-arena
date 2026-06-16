//! Flag-carrier fatigue: the speed a flag runner sheds the longer it clings to a
//! stolen flag, on top of the flat carry tax.
//!
//! A classic capture-the-flag pressure valve in the Death Rally mould: the flat
//! carry tax ([`crate::gameplay::ctf::flag_carrier_speed_multiplier`]) already
//! makes the run home a gauntlet, but on its own it lets a carrier with a clean
//! break circle the arena indefinitely, never committing to its base, stalling the
//! round into a chase that never resolves. Fatigue closes that out: a carry costs
//! more pace the longer it drags on, so a runner must commit to a quick break for
//! home rather than dawdle, and a chasing pack is handed a steadily widening
//! window to run the carrier down. Modelled here as a small, capped speed penalty
//! that holds off through an opening grace window, then ramps in with the frames a
//! flag has been carried, reaching its full bite only on a long hold.
//!
//! Like every per-car feel modifier the penalty is read by both movement systems,
//! the human's `car_movement_system` and the field's `virtual_player_drive_system`,
//! so the human and the AI tire on the identical terms: whichever side is hauling
//! a flag is the side that feels the drag.
//!
//! Fatigue can only ever *slow* a carrier, never speed one, so it can never let a
//! flag run outpace the field: it leaves the tuned "even the slowest chaser
//! outpaces the fastest carrier" chase balance fully intact and only ever presses
//! it harder, exactly as the carry tax, the spoiled slipstream
//! ([`crate::gameplay::slipstream`]) and the forfeited catch-up
//! ([`crate::gameplay::comeback`]) all do for a carrier.

/// Frames a flag may be carried before fatigue begins to bite.
///
/// A grace window so a clean, quick break for home is never punished: a runner
/// that grabs the flag and commits straight to its base outruns the drag entirely.
/// At the game's 60 FPS convention this is three seconds, long enough to reward a
/// daring grab-and-go yet short enough that a carrier playing keep-away soon tires.
pub const CARRY_FATIGUE_GRACE_FRAMES: u32 = 180;

/// Frames of carry at which fatigue reaches its full bite.
///
/// Past the grace window the penalty ramps in to its cap over this horizon, ten
/// seconds at 60 FPS, matching the [`crate::gameplay::ctf::FLAG_RESET_FRAMES`]
/// window an abandoned flag takes to auto-return: a carry that has dragged on as
/// long as a loose flag would take to reset is as tired as it ever gets.
pub const CARRY_FATIGUE_FULL_FRAMES: u32 = 600;

/// The fatigue ramp must span a real stretch of carry, enforced at compile time,
/// so the penalty builds with the hold rather than snapping straight to full the
/// instant the grace window lapses.
const _: () = assert!(CARRY_FATIGUE_GRACE_FRAMES < CARRY_FATIGUE_FULL_FRAMES);

/// A grace window must genuinely shield a quick break, enforced at compile time, so
/// a clean grab-and-go for home never tires.
const _: () = assert!(CARRY_FATIGUE_GRACE_FRAMES > 0);

/// Largest fraction of its speed a carrier sheds to fatigue at the full bite.
///
/// A meaningful drag, on the order of a perfect slipstream tow
/// ([`crate::gameplay::slipstream::DRAFT_MAX_SPEED_BONUS`]), so a long hold visibly
/// costs pace, yet never a stop: even fully spent the carrier still rolls home. The
/// penalty scales up with the frames carried (see [`carry_fatigue_speed_multiplier`]),
/// so this top rate is reached only after a long hold.
pub const CARRY_FATIGUE_MAX_SPEED_PENALTY: f32 = 0.12;

/// Fatigue must be a real bleed yet never a stop, enforced at compile time, so the
/// penalty can never drift into pinning a carrier motionless.
const _: () =
    assert!(CARRY_FATIGUE_MAX_SPEED_PENALTY > 0.0 && CARRY_FATIGUE_MAX_SPEED_PENALTY < 0.5);

/// A fully-spent but otherwise sound carrier must stay quicker than a near-wrecked
/// engine limping on minimum integrity, enforced at compile time. Fatigue layers on
/// top of the flat carry tax ([`crate::gameplay::ctf::FLAG_CARRIER_SPEED_MULTIPLIER`]),
/// and even at full bite the two together must out-pace
/// [`crate::gameplay::combat::MIN_INTEGRITY_SPEED_MULTIPLIER`], so a tired carry
/// never costs more pace than a battered engine and the speed-penalty ordering
/// stays coherent.
const _: () = assert!(
    crate::gameplay::ctf::FLAG_CARRIER_SPEED_MULTIPLIER * (1.0 - CARRY_FATIGUE_MAX_SPEED_PENALTY)
        > crate::gameplay::combat::MIN_INTEGRITY_SPEED_MULTIPLIER
);

/// Speed multiplier a carrier earns from fatigue, given the `carry_frames` the flag
/// it is hauling has been continuously carried.
///
/// Returns `1.0` (no drag) through the opening [`CARRY_FATIGUE_GRACE_FRAMES`] grace
/// window, so a quick break for home is never punished. Past the grace window the
/// penalty ramps in linearly with the frames carried: nothing at the edge of the
/// grace window, building to the full [`CARRY_FATIGUE_MAX_SPEED_PENALTY`] by
/// [`CARRY_FATIGUE_FULL_FRAMES`] and held there for any longer hold. The result is
/// always in `1.0 - CARRY_FATIGUE_MAX_SPEED_PENALTY ..= 1.0`.
///
/// The caller passes the carried flag's continuous-carry frame count, so the same
/// reading drives the human and the field alike.
#[must_use]
pub fn carry_fatigue_speed_multiplier(carry_frames: u32) -> f32 {
    if carry_frames <= CARRY_FATIGUE_GRACE_FRAMES {
        return 1.0;
    }
    let span = CARRY_FATIGUE_FULL_FRAMES - CARRY_FATIGUE_GRACE_FRAMES;
    let into_fatigue = (carry_frames - CARRY_FATIGUE_GRACE_FRAMES).min(span);
    let fraction = frames_to_f32(into_fatigue) / frames_to_f32(span);
    CARRY_FATIGUE_MAX_SPEED_PENALTY.mul_add(-fraction, 1.0)
}

/// Losslessly widens a small frame count to `f32` for the fatigue ramp.
///
/// The frame counts fed to the ramp are clamped to the [`CARRY_FATIGUE_FULL_FRAMES`]
/// span before conversion, so they always fit a `u16` and convert to `f32` exactly;
/// the saturating fallback is unreachable in practice and merely keeps the
/// conversion total without an `as` cast or a panic.
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
    fn a_fresh_grab_keeps_full_carry_speed() {
        assert_near(carry_fatigue_speed_multiplier(0), 1.0);
    }

    #[test]
    fn a_carry_within_the_grace_window_keeps_full_speed() {
        assert_near(
            carry_fatigue_speed_multiplier(CARRY_FATIGUE_GRACE_FRAMES),
            1.0,
        );
    }

    #[test]
    fn a_carry_just_past_the_grace_window_begins_to_tire() {
        let multiplier = carry_fatigue_speed_multiplier(CARRY_FATIGUE_GRACE_FRAMES + 1);
        assert!(
            multiplier < 1.0 && multiplier > 1.0 - CARRY_FATIGUE_MAX_SPEED_PENALTY,
            "a carry past the grace window should bleed a little, got {multiplier}"
        );
    }

    #[test]
    fn a_long_hold_reaches_the_full_penalty() {
        assert_near(
            carry_fatigue_speed_multiplier(CARRY_FATIGUE_FULL_FRAMES),
            1.0 - CARRY_FATIGUE_MAX_SPEED_PENALTY,
        );
    }

    #[test]
    fn fatigue_is_capped_beyond_the_full_horizon() {
        // A flag clung to far past the full-fatigue horizon tires no further than
        // the cap, so the multiplier can never run below its floor.
        assert_near(
            carry_fatigue_speed_multiplier(CARRY_FATIGUE_FULL_FRAMES + 100_000),
            1.0 - CARRY_FATIGUE_MAX_SPEED_PENALTY,
        );
    }

    #[test]
    fn fatigue_strengthens_the_longer_the_flag_is_held() {
        let early = carry_fatigue_speed_multiplier(CARRY_FATIGUE_GRACE_FRAMES + 60);
        let late = carry_fatigue_speed_multiplier(CARRY_FATIGUE_FULL_FRAMES - 60);
        assert!(
            late < early && early < 1.0,
            "a longer hold should tire harder: early={early}, late={late}"
        );
    }

    #[test]
    fn fatigue_ramps_rather_than_snapping_to_full() {
        // Halfway through the ramp must be a genuine part penalty, strictly between
        // nothing and the full cap, so the drag builds with the hold rather than
        // snapping straight to its top rate the instant the grace window lapses.
        let span = CARRY_FATIGUE_FULL_FRAMES - CARRY_FATIGUE_GRACE_FRAMES;
        let midpoint = carry_fatigue_speed_multiplier(CARRY_FATIGUE_GRACE_FRAMES + span / 2);
        assert!(
            midpoint < 1.0 && midpoint > 1.0 - CARRY_FATIGUE_MAX_SPEED_PENALTY,
            "the midpoint should be a part penalty, got {midpoint}"
        );
    }

    // The invariants "fatigue is a real bleed yet never a stop" and "a tired carry
    // never out-costs a battered engine" are enforced at compile time by the
    // `const _: () = assert!(..)` blocks above, so a runtime test would only assert
    // a constant clippy already proves.
}
