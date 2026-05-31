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

    /// Tactical value virtual players use when choosing which pickup to chase.
    #[must_use]
    pub const fn virtual_player_priority(self) -> u32 {
        match self {
            Self::Cash => 100,
            Self::Repair => 25,
            Self::Nitro => 150,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounty_rewards_cash_highest() {
        assert!(PickupKind::Cash.bounty() > PickupKind::Nitro.bounty());
        assert!(PickupKind::Nitro.bounty() > PickupKind::Repair.bounty());
    }

    #[test]
    fn virtual_players_value_nitro_highest() {
        assert!(
            PickupKind::Nitro.virtual_player_priority()
                > PickupKind::Cash.virtual_player_priority()
        );
        assert!(
            PickupKind::Cash.virtual_player_priority()
                > PickupKind::Repair.virtual_player_priority()
        );
    }
}
