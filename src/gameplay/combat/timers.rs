//! The per-team wreck timers: the short-lived speed and payback windows a wreck
//! opens for each side.
//!
//! The combat model's per-team frame-timer STATE, split from the ram, wreck and
//! stun MECHANICS in the parent `combat` module that drive them. Each timer here
//! is a pair of per-team frame counters wound down a frame at a time, sharing one
//! shape: [`WreckStuns`] is the wrecked side's spin-out, [`WreckSurges`] the
//! wrecker's speed burst, [`PaybackWindows`] the riposte window. Pure state with
//! no ECS reach, triggered by the parent's [`super::ram_damage_system`] on the
//! frame a wreck lands, wound down by the parent's per-timer decay systems, and
//! read by the movement systems (and the wreck economy's
//! [`super::payback_wreck_bonus`]).

use super::{
    WreckEvents, PAYBACK_WINDOW_FRAMES, WRECK_STUN_FRAMES, WRECK_STUN_SPEED_MULTIPLIER,
    WRECK_SURGE_FRAMES, WRECK_SURGE_SPEED_MULTIPLIER,
};
use crate::gameplay::virtual_player::ai::AiTeam;
use bevy::prelude::*;

/// Brief spin-out each team suffers the instant its cars are wrecked.
///
/// Mirrors [`crate::gameplay::pickup::NitroBoosts`]: a per-team frame timer that
/// translates into a speed multiplier while it burns down. Triggered by
/// [`super::ram_damage_system`] on the frame a team is newly wrecked, wound down each
/// frame by [`super::wreck_stun_decay_system`], and read by the movement systems to
/// stagger a freshly wrecked team's cars.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WreckStuns {
    /// Frames the player team's cars keep spinning out.
    pub player_frames: u32,
    /// Frames the opponent team's cars keep spinning out.
    pub opponent_frames: u32,
}

impl WreckStuns {
    /// Speed multiplier the player team suffers while spinning out.
    #[must_use]
    pub const fn player_multiplier(self) -> f32 {
        if self.player_frames > 0 {
            WRECK_STUN_SPEED_MULTIPLIER
        } else {
            1.0
        }
    }

    /// Speed multiplier the opponent team suffers while spinning out.
    #[must_use]
    pub const fn opponent_multiplier(self) -> f32 {
        if self.opponent_frames > 0 {
            WRECK_STUN_SPEED_MULTIPLIER
        } else {
            1.0
        }
    }

    /// Speed multiplier for the given team's current spin-out.
    #[must_use]
    pub const fn multiplier_for_team(self, team: AiTeam) -> f32 {
        match team {
            AiTeam::Blue => self.player_multiplier(),
            AiTeam::Red => self.opponent_multiplier(),
        }
    }

    /// Spins out the player team for a fresh [`WRECK_STUN_FRAMES`] window.
    pub const fn trigger_player(&mut self) {
        self.player_frames = WRECK_STUN_FRAMES;
    }

    /// Spins out the opponent team for a fresh [`WRECK_STUN_FRAMES`] window.
    pub const fn trigger_opponent(&mut self) {
        self.opponent_frames = WRECK_STUN_FRAMES;
    }

    /// Spins out whichever teams crossed into a full wreck this frame.
    pub const fn apply_wrecks(&mut self, wrecks: WreckEvents) {
        if wrecks.player {
            self.trigger_player();
        }
        if wrecks.opponent {
            self.trigger_opponent();
        }
    }

    /// Winds every team's spin-out down by one frame.
    pub const fn tick(&mut self) {
        self.player_frames = self.player_frames.saturating_sub(1);
        self.opponent_frames = self.opponent_frames.saturating_sub(1);
    }
}

/// Brief speed surge each team enjoys the instant it wrecks an enemy.
///
/// The reward mirror of [`WreckStuns`]: where the wrecked team spins out, the
/// team that dealt the wreck surges. Same per-team frame-timer shape, triggered
/// by [`super::ram_damage_system`] on the frame an enemy is newly wrecked, wound down
/// each frame by [`super::wreck_surge_decay_system`], and read by the movement systems
/// to give a fresh wrecker a burst of speed it can capitalise on.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WreckSurges {
    /// Frames the player team's cars keep surging.
    pub player_frames: u32,
    /// Frames the opponent team's cars keep surging.
    pub opponent_frames: u32,
}

impl WreckSurges {
    /// Speed multiplier the player team enjoys while surging.
    #[must_use]
    pub const fn player_multiplier(self) -> f32 {
        if self.player_frames > 0 {
            WRECK_SURGE_SPEED_MULTIPLIER
        } else {
            1.0
        }
    }

    /// Speed multiplier the opponent team enjoys while surging.
    #[must_use]
    pub const fn opponent_multiplier(self) -> f32 {
        if self.opponent_frames > 0 {
            WRECK_SURGE_SPEED_MULTIPLIER
        } else {
            1.0
        }
    }

    /// Speed multiplier for the given team's current surge.
    #[must_use]
    pub const fn multiplier_for_team(self, team: AiTeam) -> f32 {
        match team {
            AiTeam::Blue => self.player_multiplier(),
            AiTeam::Red => self.opponent_multiplier(),
        }
    }

    /// Surges the player team for a fresh [`WRECK_SURGE_FRAMES`] window.
    pub const fn trigger_player(&mut self) {
        self.player_frames = WRECK_SURGE_FRAMES;
    }

    /// Surges the opponent team for a fresh [`WRECK_SURGE_FRAMES`] window.
    pub const fn trigger_opponent(&mut self) {
        self.opponent_frames = WRECK_SURGE_FRAMES;
    }

    /// Surges whichever team *dealt* a wreck this frame: the enemy of each
    /// wrecked team.
    ///
    /// A wrecked opponent means the player team landed the kill and surges, and
    /// vice versa. A double wreck surges both teams at once, mirroring how both
    /// also spin out.
    pub const fn reward_wreckers(&mut self, wrecks: WreckEvents) {
        if wrecks.opponent {
            self.trigger_player();
        }
        if wrecks.player {
            self.trigger_opponent();
        }
    }

    /// Winds every team's surge down by one frame.
    pub const fn tick(&mut self) {
        self.player_frames = self.player_frames.saturating_sub(1);
        self.opponent_frames = self.opponent_frames.saturating_sub(1);
    }
}

/// Tracks how long each team is still owed a payback after being wrecked.
///
/// A per-team frame timer shaped exactly like [`WreckStuns`], but read as a flag
/// rather than a speed multiplier: it opens for [`PAYBACK_WINDOW_FRAMES`] the
/// instant a team is wrecked, winds down each frame via
/// [`super::payback_window_decay_system`], and lets [`super::ram_damage_system`] pay the
/// [`super::payback_wreck_bonus`] when a team wrecks an enemy back while its window is
/// still live. Reset when a fresh match begins.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaybackWindows {
    /// Frames the player team is still owed a payback.
    pub player_frames: u32,
    /// Frames the opponent team is still owed a payback.
    pub opponent_frames: u32,
}

impl PaybackWindows {
    /// Whether the player team is still owed a payback this frame.
    #[must_use]
    pub const fn is_player_live(self) -> bool {
        self.player_frames > 0
    }

    /// Whether the opponent team is still owed a payback this frame.
    #[must_use]
    pub const fn is_opponent_live(self) -> bool {
        self.opponent_frames > 0
    }

    /// Owes the player team a payback for a fresh [`PAYBACK_WINDOW_FRAMES`] window.
    pub const fn trigger_player(&mut self) {
        self.player_frames = PAYBACK_WINDOW_FRAMES;
    }

    /// Owes the opponent team a payback for a fresh [`PAYBACK_WINDOW_FRAMES`] window.
    pub const fn trigger_opponent(&mut self) {
        self.opponent_frames = PAYBACK_WINDOW_FRAMES;
    }

    /// Opens a payback window for whichever teams were wrecked this frame, so a
    /// freshly wrecked side is owed a riposte from the next frame on.
    pub const fn apply_wrecks(&mut self, wrecks: WreckEvents) {
        if wrecks.player {
            self.trigger_player();
        }
        if wrecks.opponent {
            self.trigger_opponent();
        }
    }

    /// Winds every team's payback window down by one frame.
    pub const fn tick(&mut self) {
        self.player_frames = self.player_frames.saturating_sub(1);
        self.opponent_frames = self.opponent_frames.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 1e-4,
            "actual={actual}, expected={expected}"
        );
    }

    #[test]
    fn wreck_stuns_default_to_inactive_for_both_teams() {
        let stuns = WreckStuns::default();
        assert_eq!(stuns.player_frames, 0);
        assert_eq!(stuns.opponent_frames, 0);
        assert_near(stuns.player_multiplier(), 1.0);
        assert_near(stuns.opponent_multiplier(), 1.0);
    }

    #[test]
    fn triggering_a_stun_spins_out_only_that_team() {
        let mut stuns = WreckStuns::default();
        stuns.trigger_opponent();
        assert_eq!(stuns.opponent_frames, WRECK_STUN_FRAMES);
        assert_near(stuns.opponent_multiplier(), WRECK_STUN_SPEED_MULTIPLIER);
        // The wrecking team keeps full speed; only the wreck spins out.
        assert_eq!(stuns.player_frames, 0);
        assert_near(stuns.player_multiplier(), 1.0);
    }

    #[test]
    fn a_spin_out_expires_after_its_window() {
        let mut stuns = WreckStuns::default();
        stuns.trigger_player();
        for _ in 0..WRECK_STUN_FRAMES {
            assert_near(stuns.player_multiplier(), WRECK_STUN_SPEED_MULTIPLIER);
            stuns.tick();
        }
        assert_near(stuns.player_multiplier(), 1.0);
        // Ticking a spent timer never underflows.
        stuns.tick();
        assert_eq!(stuns.player_frames, 0);
    }

    #[test]
    fn stun_multiplier_for_team_routes_to_the_right_pool() {
        let mut stuns = WreckStuns::default();
        stuns.trigger_player();
        assert_near(
            stuns.multiplier_for_team(AiTeam::Blue),
            WRECK_STUN_SPEED_MULTIPLIER,
        );
        assert_near(stuns.multiplier_for_team(AiTeam::Red), 1.0);
    }

    #[test]
    fn apply_wrecks_spins_out_each_wrecked_team() {
        let mut player_only = WreckStuns::default();
        player_only.apply_wrecks(WreckEvents {
            player: true,
            opponent: false,
        });
        assert_eq!(player_only.player_frames, WRECK_STUN_FRAMES);
        assert_eq!(player_only.opponent_frames, 0);

        let mut both = WreckStuns::default();
        both.apply_wrecks(WreckEvents {
            player: true,
            opponent: true,
        });
        assert_eq!(both.player_frames, WRECK_STUN_FRAMES);
        assert_eq!(both.opponent_frames, WRECK_STUN_FRAMES);

        let mut quiet = WreckStuns::default();
        quiet.apply_wrecks(WreckEvents::default());
        assert_eq!(quiet, WreckStuns::default());
    }

    #[test]
    fn wreck_surges_default_to_inactive_for_both_teams() {
        let surges = WreckSurges::default();
        assert_eq!(surges.player_frames, 0);
        assert_eq!(surges.opponent_frames, 0);
        assert_near(surges.player_multiplier(), 1.0);
        assert_near(surges.opponent_multiplier(), 1.0);
    }

    #[test]
    fn a_surge_speeds_only_the_team_that_landed_the_kill() {
        let mut surges = WreckSurges::default();
        surges.trigger_player();
        assert_eq!(surges.player_frames, WRECK_SURGE_FRAMES);
        assert_near(surges.player_multiplier(), WRECK_SURGE_SPEED_MULTIPLIER);
        // The wrecked team gets no surge; only the wrecker speeds up.
        assert_eq!(surges.opponent_frames, 0);
        assert_near(surges.opponent_multiplier(), 1.0);
    }

    #[test]
    fn a_surge_expires_after_its_window() {
        let mut surges = WreckSurges::default();
        surges.trigger_opponent();
        for _ in 0..WRECK_SURGE_FRAMES {
            assert_near(surges.opponent_multiplier(), WRECK_SURGE_SPEED_MULTIPLIER);
            surges.tick();
        }
        assert_near(surges.opponent_multiplier(), 1.0);
        // Ticking a spent timer never underflows.
        surges.tick();
        assert_eq!(surges.opponent_frames, 0);
    }

    #[test]
    fn surge_multiplier_for_team_routes_to_the_right_pool() {
        let mut surges = WreckSurges::default();
        surges.trigger_opponent();
        assert_near(
            surges.multiplier_for_team(AiTeam::Red),
            WRECK_SURGE_SPEED_MULTIPLIER,
        );
        assert_near(surges.multiplier_for_team(AiTeam::Blue), 1.0);
    }

    #[test]
    fn reward_wreckers_surges_the_enemy_of_each_wrecked_team() {
        // A wrecked opponent means the player team dealt the kill and surges.
        let mut player_dealt = WreckSurges::default();
        player_dealt.reward_wreckers(WreckEvents {
            player: false,
            opponent: true,
        });
        assert_eq!(player_dealt.player_frames, WRECK_SURGE_FRAMES);
        assert_eq!(player_dealt.opponent_frames, 0);

        // A wrecked player team means the opponents dealt the kill and surge.
        let mut opponent_dealt = WreckSurges::default();
        opponent_dealt.reward_wreckers(WreckEvents {
            player: true,
            opponent: false,
        });
        assert_eq!(opponent_dealt.opponent_frames, WRECK_SURGE_FRAMES);
        assert_eq!(opponent_dealt.player_frames, 0);

        // A double wreck surges both teams at once.
        let mut both = WreckSurges::default();
        both.reward_wreckers(WreckEvents {
            player: true,
            opponent: true,
        });
        assert_eq!(both.player_frames, WRECK_SURGE_FRAMES);
        assert_eq!(both.opponent_frames, WRECK_SURGE_FRAMES);

        // A quiet frame surges nobody.
        let mut quiet = WreckSurges::default();
        quiet.reward_wreckers(WreckEvents::default());
        assert_eq!(quiet, WreckSurges::default());
    }

    #[test]
    fn payback_window_opens_for_a_wrecked_team_and_winds_down() {
        let mut windows = PaybackWindows::default();
        assert!(
            !windows.is_player_live(),
            "no one is owed a payback at kick-off"
        );

        windows.apply_wrecks(WreckEvents {
            player: true,
            opponent: false,
        });
        assert_eq!(
            windows.player_frames, PAYBACK_WINDOW_FRAMES,
            "being wrecked owes the player team a full payback window"
        );
        assert!(
            !windows.is_opponent_live(),
            "the side that landed the wreck is owed nothing"
        );

        windows.tick();
        assert_eq!(windows.player_frames, PAYBACK_WINDOW_FRAMES - 1);
        assert!(windows.is_player_live(), "the riposte window is still open");
    }
}
