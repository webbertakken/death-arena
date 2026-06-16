//! The per-team flag timers: how long each side's flag has lain loose and how
//! long it has been continuously carried.
//!
//! The CTF model's per-team frame-timer STATE, split from the flag mechanics, the
//! match clock and the cash economy in the parent `ctf` module that drive them.
//! Each timer here is a pair of per-team frame counters wound a frame at a time:
//! [`LooseFlagTimers`] counts a flag abandoned loose toward its auto-return,
//! [`FlagCarryTimers`] counts a flag held toward carrier fatigue, alongside the
//! pure [`advance_loose_flag`] classifier that decides what a loose timer dictates
//! each frame. Mirrors the per-team-timer split in [`crate::gameplay::combat`].
//! Pure state with no ECS reach: the parent's [`super::capture_the_flag_system`]
//! reads these through its flag-slice rules
//! ([`super::auto_return_loose_flags`], [`super::advance_flag_carry_timers`]), and
//! [`FlagCarryTimers`] is read by the movement systems through
//! [`crate::gameplay::carry_fatigue::carry_fatigue_speed_multiplier`].

use super::{FlagTeam, FLAG_RESET_FRAMES};
use bevy::prelude::*;

/// Per-team countdown tracking how long each side's flag has lain loose.
///
/// Mirrors [`crate::gameplay::combat::WreckStuns`]: a per-team frame counter,
/// here advanced each frame by [`super::capture_the_flag_system`] and read to
/// auto-return a flag abandoned past [`FLAG_RESET_FRAMES`]. Cleared the moment a
/// flag is held or sitting home, so only a genuinely loose flag ever counts up.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LooseFlagTimers {
    /// Frames the blue flag has lain loose.
    pub blue_frames: u32,
    /// Frames the red flag has lain loose.
    pub red_frames: u32,
}

impl LooseFlagTimers {
    /// Frames the given team's flag has lain loose.
    #[must_use]
    pub const fn frames_for(self, team: FlagTeam) -> u32 {
        match team {
            FlagTeam::Blue => self.blue_frames,
            FlagTeam::Red => self.red_frames,
        }
    }

    /// Sets the loose-frame count for the given team's flag.
    pub const fn set_for(&mut self, team: FlagTeam, frames: u32) {
        match team {
            FlagTeam::Blue => self.blue_frames = frames,
            FlagTeam::Red => self.red_frames = frames,
        }
    }
}

/// Per-team count of consecutive frames each side's flag has been carried.
///
/// The carry-side mirror of [`LooseFlagTimers`]: where that counts how long a
/// flag has lain loose toward an auto-return, this counts how long a flag has been
/// held toward carrier fatigue ([`crate::gameplay::carry_fatigue`]). Advanced each
/// frame by [`super::capture_the_flag_system`]: a flag in a holder's hands counts
/// up, one sitting loose or home clears to zero, so a flag knocked free and grabbed
/// afresh starts its carrier on a clean clock.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagCarryTimers {
    /// Frames the blue flag has been continuously carried.
    pub blue_frames: u32,
    /// Frames the red flag has been continuously carried.
    pub red_frames: u32,
}

impl FlagCarryTimers {
    /// Frames the given team's flag has been continuously carried.
    #[must_use]
    pub const fn frames_for(self, team: FlagTeam) -> u32 {
        match team {
            FlagTeam::Blue => self.blue_frames,
            FlagTeam::Red => self.red_frames,
        }
    }

    /// Sets the carry-frame count for the given team's flag.
    pub const fn set_for(&mut self, team: FlagTeam, frames: u32) {
        match team {
            FlagTeam::Blue => self.blue_frames = frames,
            FlagTeam::Red => self.red_frames = frames,
        }
    }
}

/// What a flag's loose timer dictates for the current frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LooseFlagOutcome {
    /// Held or already home: clear the timer.
    Settled,
    /// Loose and still inside the grace window: keep counting at this value.
    Counting(u32),
    /// Loose past [`FLAG_RESET_FRAMES`]: return the flag to its home base.
    ResetHome,
}

/// Advances a flag's loose timer by one fixed-update frame.
///
/// A held or home flag clears the timer ([`LooseFlagOutcome::Settled`]); a loose
/// flag counts up until it crosses [`FLAG_RESET_FRAMES`], when it is sent home.
#[must_use]
pub const fn advance_loose_flag(is_held: bool, is_at_home: bool, frames: u32) -> LooseFlagOutcome {
    if is_held || is_at_home {
        return LooseFlagOutcome::Settled;
    }
    let next = frames + 1;
    if next >= FLAG_RESET_FRAMES {
        LooseFlagOutcome::ResetHome
    } else {
        LooseFlagOutcome::Counting(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_flag_clears_its_loose_timer() {
        assert_eq!(
            advance_loose_flag(true, false, 123),
            LooseFlagOutcome::Settled,
            "a carried flag is not loose, so its timer must reset"
        );
    }

    #[test]
    fn a_home_flag_clears_its_loose_timer() {
        assert_eq!(
            advance_loose_flag(false, true, 123),
            LooseFlagOutcome::Settled,
            "a flag sitting at base is not loose, so its timer must reset"
        );
    }

    #[test]
    fn a_loose_flag_counts_up_inside_the_grace_window() {
        assert_eq!(
            advance_loose_flag(false, false, 0),
            LooseFlagOutcome::Counting(1)
        );
        assert_eq!(
            advance_loose_flag(false, false, FLAG_RESET_FRAMES - 2),
            LooseFlagOutcome::Counting(FLAG_RESET_FRAMES - 1)
        );
    }

    #[test]
    fn a_loose_flag_returns_home_at_the_reset_limit() {
        assert_eq!(
            advance_loose_flag(false, false, FLAG_RESET_FRAMES - 1),
            LooseFlagOutcome::ResetHome,
            "a flag loose for the full grace window must auto-return"
        );
    }
}
