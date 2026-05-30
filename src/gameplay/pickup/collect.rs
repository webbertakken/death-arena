use bevy::prelude::*;

/// The classic Death Rally trackside collectibles a car can drive over.
///
/// Each kind awards a cash bounty when picked up; richer per-kind effects
/// (repair, nitro) build on top of this collection layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickupKind {
    /// A bag of cash: the core Death Rally economy reward.
    Cash,
    /// A repair pickup that also pays out a small bounty.
    Repair,
    /// A nitro canister that pays out a small bounty.
    Nitro,
}

impl PickupKind {
    /// Cash awarded for collecting this pickup.
    #[must_use]
    pub const fn bounty(self) -> u32 {
        match self {
            Self::Cash => 100,
            Self::Repair => 25,
            Self::Nitro => 50,
        }
    }
}

/// Index of the nearest pickup a collector at `collector` is touching.
///
/// A pickup counts as collected when the collector is within (or exactly on)
/// `radius`. When several pickups are in range the closest one wins; ties
/// resolve to the lowest index so the result is deterministic. Returns `None`
/// when nothing is in range.
#[must_use]
pub fn nearest_collectible(collector: Vec2, pickups: &[Vec2], radius: f32) -> Option<usize> {
    let radius_sq = radius * radius;
    pickups
        .iter()
        .enumerate()
        .filter_map(|(index, &pos)| {
            let distance_sq = collector.distance_squared(pos);
            (distance_sq <= radius_sq).then_some((index, distance_sq))
        })
        .min_by(|(a_index, a_dist), (b_index, b_dist)| {
            a_dist
                .partial_cmp(b_dist)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a_index.cmp(b_index))
        })
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RADIUS: f32 = 50.0;

    #[test]
    fn no_pickups_collects_nothing() {
        assert_eq!(nearest_collectible(Vec2::ZERO, &[], RADIUS), None);
    }

    #[test]
    fn ignores_pickups_outside_radius() {
        let pickups = [Vec2::new(200.0, 0.0)];
        assert_eq!(nearest_collectible(Vec2::ZERO, &pickups, RADIUS), None);
    }

    #[test]
    fn collects_pickup_within_radius() {
        let pickups = [Vec2::new(10.0, 0.0)];
        assert_eq!(nearest_collectible(Vec2::ZERO, &pickups, RADIUS), Some(0));
    }

    #[test]
    fn collects_pickup_exactly_on_radius_boundary() {
        let pickups = [Vec2::new(RADIUS, 0.0)];
        assert_eq!(nearest_collectible(Vec2::ZERO, &pickups, RADIUS), Some(0));
    }

    #[test]
    fn picks_the_nearest_of_several_in_range() {
        let pickups = [
            Vec2::new(40.0, 0.0),
            Vec2::new(5.0, 0.0),
            Vec2::new(20.0, 0.0),
        ];
        assert_eq!(nearest_collectible(Vec2::ZERO, &pickups, RADIUS), Some(1));
    }

    #[test]
    fn ties_resolve_to_lowest_index() {
        let pickups = [Vec2::new(0.0, 10.0), Vec2::new(10.0, 0.0)];
        assert_eq!(nearest_collectible(Vec2::ZERO, &pickups, RADIUS), Some(0));
    }

    #[test]
    fn zero_radius_only_collects_exact_overlap() {
        let pickups = [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0)];
        assert_eq!(nearest_collectible(Vec2::ZERO, &pickups, 0.0), Some(0));
    }

    #[test]
    fn bounty_rewards_cash_highest() {
        assert!(PickupKind::Cash.bounty() > PickupKind::Nitro.bounty());
        assert!(PickupKind::Nitro.bounty() > PickupKind::Repair.bounty());
    }
}
