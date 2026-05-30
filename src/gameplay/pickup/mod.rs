use crate::{App, AppState, Plugin};
use bevy::prelude::*;

pub mod collect;
mod spawn;
mod system;

pub use collect::PickupKind;

/// World-space distance at which a car collects a pickup it drives over.
pub const PICKUP_RADIUS: f32 = 120.0;

/// A trackside collectible the player drives over to bank a bounty.
#[derive(Component, Debug)]
pub struct Pickup {
    pub kind: PickupKind,
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

#[derive(Default)]
pub struct PickupPlugin;

impl Plugin for PickupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Score>()
            .add_system_set(SystemSet::on_enter(AppState::InGame).with_system(spawn::setup));
        app.add_system(system::pickup_collection_system);
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
}
