use crate::{App, AppState, Plugin};
use bevy::prelude::*;

pub mod collect;
mod spawn;
mod system;

pub use collect::PickupKind;

/// World-space distance at which a car collects a pickup it drives over.
pub const PICKUP_RADIUS: f32 = 120.0;
/// Number of fixed update frames before a collected pickup returns.
pub const PICKUP_RESPAWN_FRAMES: u32 = 600;
/// Number of fixed update frames a nitro pickup boosts a car.
pub const NITRO_BOOST_FRAMES: u32 = 180;
/// Speed multiplier applied while a nitro boost is active.
pub const NITRO_SPEED_MULTIPLIER: f32 = 1.5;

/// A trackside collectible the player drives over to bank a bounty.
#[derive(Component, Debug)]
pub struct Pickup {
    pub kind: PickupKind,
}

/// A pickup waiting to return to the arena after collection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PendingPickupRespawn {
    pub kind: PickupKind,
    pub position: Vec2,
    pub frames_remaining: u32,
}

/// Queue of pickups that will respawn after a short cooldown.
#[derive(Resource, Default, Debug, PartialEq)]
pub struct PickupRespawns {
    pub pending: Vec<PendingPickupRespawn>,
}

impl PickupRespawns {
    pub fn queue(&mut self, kind: PickupKind, position: Vec2) {
        self.pending.push(PendingPickupRespawn {
            kind,
            position,
            frames_remaining: PICKUP_RESPAWN_FRAMES,
        });
    }
}

/// Timed nitro boosts currently active for the player and opponent team.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct NitroBoosts {
    pub player_frames: u32,
    pub opponent_frames: u32,
}

impl NitroBoosts {
    pub const fn player_multiplier(&self) -> f32 {
        if self.player_frames > 0 {
            NITRO_SPEED_MULTIPLIER
        } else {
            1.0
        }
    }

    pub const fn opponent_multiplier(&self) -> f32 {
        if self.opponent_frames > 0 {
            NITRO_SPEED_MULTIPLIER
        } else {
            1.0
        }
    }

    pub const fn trigger_player(&mut self) {
        self.player_frames = NITRO_BOOST_FRAMES;
    }

    pub const fn trigger_opponent(&mut self) {
        self.opponent_frames = NITRO_BOOST_FRAMES;
    }

    pub const fn tick(&mut self) {
        self.player_frames = self.player_frames.saturating_sub(1);
        self.opponent_frames = self.opponent_frames.saturating_sub(1);
    }
}

/// Running tally of what the player has collected this session.
///
/// Mirrors the Death Rally loop where banked cash drives upgrades.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct Score {
    /// Total cash banked from every collected pickup.
    pub cash: u32,
    /// Number of pickups collected.
    pub collected: u32,
}

impl Score {
    /// Apply a collected pickup's reward to the tally.
    pub const fn collect(&mut self, kind: PickupKind) {
        self.cash += kind.bounty();
        self.collected += 1;
    }
}

/// Running tally of pickups stolen by virtual opponents.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct OpponentScore {
    /// Total cash banked by virtual players.
    pub cash: u32,
    /// Number of pickups collected by virtual players.
    pub collected: u32,
}

impl OpponentScore {
    /// Apply a collected pickup's reward to the opponent tally.
    pub const fn collect(&mut self, kind: PickupKind) {
        self.cash += kind.bounty();
        self.collected += 1;
    }
}

#[derive(Default)]
pub struct PickupPlugin;

impl Plugin for PickupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Score>()
            .init_resource::<OpponentScore>()
            .init_resource::<NitroBoosts>()
            .init_resource::<PickupRespawns>()
            .add_system_set(SystemSet::on_enter(AppState::InGame).with_system(spawn::setup));
        app.add_system(system::nitro_boost_decay_system.before(system::pickup_collection_system))
            .add_system(system::pickup_collection_system)
            .add_system(system::pickup_respawn_system.after(system::pickup_collection_system));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collecting_accumulates_cash_and_count() {
        let mut score = Score::default();
        score.collect(PickupKind::Cash);
        score.collect(PickupKind::Repair);
        assert_eq!(
            score,
            Score {
                cash: PickupKind::Cash.bounty() + PickupKind::Repair.bounty(),
                collected: 2,
            }
        );
    }

    #[test]
    fn opponent_collecting_accumulates_cash_and_count() {
        let mut score = OpponentScore::default();
        score.collect(PickupKind::Nitro);
        score.collect(PickupKind::Cash);
        assert_eq!(
            score,
            OpponentScore {
                cash: PickupKind::Nitro.bounty() + PickupKind::Cash.bounty(),
                collected: 2,
            }
        );
    }

    fn assert_multiplier_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= f32::EPSILON,
            "actual={actual}, expected={expected}"
        );
    }

    #[test]
    fn nitro_boost_starts_active_and_expires() {
        let mut boosts = NitroBoosts::default();

        boosts.trigger_player();
        boosts.trigger_opponent();

        assert_multiplier_eq(boosts.player_multiplier(), NITRO_SPEED_MULTIPLIER);
        assert_multiplier_eq(boosts.opponent_multiplier(), NITRO_SPEED_MULTIPLIER);
        assert_eq!(boosts.player_frames, NITRO_BOOST_FRAMES);
        assert_eq!(boosts.opponent_frames, NITRO_BOOST_FRAMES);

        for _ in 0..NITRO_BOOST_FRAMES {
            boosts.tick();
        }

        assert_multiplier_eq(boosts.player_multiplier(), 1.0);
        assert_multiplier_eq(boosts.opponent_multiplier(), 1.0);
        assert_eq!(boosts.player_frames, 0);
        assert_eq!(boosts.opponent_frames, 0);
    }
}
