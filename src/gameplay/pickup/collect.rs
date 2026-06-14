/// Tactical value a wrecked team places on a repair pickup.
///
/// A repair is near worthless at full durability but becomes the most valuable
/// thing on the track for a wrecked team, outranking even nitro so a battered
/// team breaks off to patch up and win its lost speed back.
pub const REPAIR_MAX_VIRTUAL_PLAYER_PRIORITY: u32 = 175;

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
    /// A shield that briefly blunts the ram damage a team takes, the defensive
    /// counter to an otherwise all-offence combat loop. Also pays a small bounty.
    Shield,
}

impl PickupKind {
    /// Cash awarded for collecting this pickup.
    #[must_use]
    pub const fn bounty(self) -> u32 {
        match self {
            Self::Cash => 100,
            Self::Repair => 25,
            // A nitro canister and a shield are both modest utility grabs.
            Self::Nitro | Self::Shield => 50,
        }
    }

    /// Tactical value virtual players use when choosing which pickup to chase.
    #[must_use]
    pub const fn virtual_player_priority(self) -> u32 {
        match self {
            Self::Cash => 100,
            Self::Repair => 25,
            Self::Nitro => 150,
            // Worth a narrow detour (> cash, crossing the CTF detour threshold)
            // but not a wide one: grabbing a defensive edge should not pull a
            // car off a committed flag run the way nitro can.
            Self::Shield => 120,
        }
    }

    /// Tactical value virtual players place on this pickup given their team's
    /// current durability `integrity_fraction` (`0.0` wrecked, `1.0` pristine).
    ///
    /// Only repairs vary: the more battered the team, the harder its cars chase
    /// a patch-up. Every other pickup keeps its flat
    /// [`Self::virtual_player_priority`].
    #[must_use]
    pub fn virtual_player_priority_for_integrity(self, integrity_fraction: f32) -> u32 {
        match self {
            Self::Repair => repair_priority_for_integrity(integrity_fraction),
            other => other.virtual_player_priority(),
        }
    }
}

/// Maps a team's durability fraction onto how hard its cars chase a repair.
///
/// Stepped tiers keep the mapping legible and free of float-to-int casts:
/// healthy teams ignore repairs, lightly worn teams begin detouring for them
/// (crossing [`crate::gameplay::virtual_player::ai::CTF_PICKUP_DETOUR_MIN_PRIORITY`]),
/// and a wrecked team rates them above every other pickup.
#[must_use]
fn repair_priority_for_integrity(integrity_fraction: f32) -> u32 {
    if integrity_fraction <= 0.15 {
        REPAIR_MAX_VIRTUAL_PLAYER_PRIORITY
    } else if integrity_fraction <= 0.35 {
        150
    } else if integrity_fraction <= 0.55 {
        110
    } else if integrity_fraction <= 0.75 {
        60
    } else {
        PickupKind::Repair.virtual_player_priority()
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

    #[test]
    fn shield_pays_the_same_modest_bounty_as_nitro() {
        assert_eq!(PickupKind::Shield.bounty(), PickupKind::Nitro.bounty());
        assert!(PickupKind::Shield.bounty() < PickupKind::Cash.bounty());
    }

    #[test]
    fn virtual_players_rate_shield_between_cash_and_nitro() {
        let shield = PickupKind::Shield.virtual_player_priority();
        assert!(
            shield > PickupKind::Cash.virtual_player_priority(),
            "a shield should outrank cash so a worn team detours for it: {shield}"
        );
        assert!(
            shield < PickupKind::Nitro.virtual_player_priority(),
            "a shield should not eclipse nitro's race pressure: {shield}"
        );
    }

    #[test]
    fn shield_keeps_its_flat_priority_regardless_of_integrity() {
        for fraction in [1.0, 0.5, 0.0] {
            assert_eq!(
                PickupKind::Shield.virtual_player_priority_for_integrity(fraction),
                PickupKind::Shield.virtual_player_priority(),
                "only repairs should scale with durability"
            );
        }
    }

    #[test]
    fn worn_team_will_detour_for_a_shield() {
        use crate::gameplay::virtual_player::ai::CTF_PICKUP_DETOUR_MIN_PRIORITY;
        assert!(
            PickupKind::Shield.virtual_player_priority() >= CTF_PICKUP_DETOUR_MIN_PRIORITY,
            "a shield must be worth at least a narrow CTF detour"
        );
    }

    #[test]
    fn healthy_team_keeps_repair_at_its_flat_priority() {
        assert_eq!(
            PickupKind::Repair.virtual_player_priority_for_integrity(1.0),
            PickupKind::Repair.virtual_player_priority(),
            "a pristine team should ignore repairs just like before"
        );
    }

    #[test]
    fn wrecked_team_rates_repair_above_every_other_pickup() {
        let wrecked = PickupKind::Repair.virtual_player_priority_for_integrity(0.0);
        assert_eq!(wrecked, REPAIR_MAX_VIRTUAL_PLAYER_PRIORITY);
        assert!(
            wrecked > PickupKind::Nitro.virtual_player_priority(),
            "a wrecked team should chase a patch-up over nitro, repair={wrecked}"
        );
        assert!(wrecked > PickupKind::Cash.virtual_player_priority());
    }

    #[test]
    fn repair_priority_rises_as_durability_drops() {
        let fractions = [1.0, 0.7, 0.5, 0.3, 0.1, 0.0];
        let priorities: Vec<u32> = fractions
            .iter()
            .map(|&fraction| PickupKind::Repair.virtual_player_priority_for_integrity(fraction))
            .collect();

        for pair in priorities.windows(2) {
            assert!(
                pair[1] >= pair[0],
                "repair priority must not fall as a team wears down: {priorities:?}"
            );
        }
        assert!(
            priorities.last() > priorities.first(),
            "a wrecked team must value repairs more than a healthy one: {priorities:?}"
        );
    }

    #[test]
    fn worn_team_crosses_the_ctf_detour_threshold_for_repairs() {
        use crate::gameplay::virtual_player::ai::CTF_PICKUP_DETOUR_MIN_PRIORITY;
        assert!(
            PickupKind::Repair.virtual_player_priority_for_integrity(1.0)
                < CTF_PICKUP_DETOUR_MIN_PRIORITY,
            "a healthy team must not interrupt a CTF objective for a repair"
        );
        assert!(
            PickupKind::Repair.virtual_player_priority_for_integrity(0.6)
                >= CTF_PICKUP_DETOUR_MIN_PRIORITY,
            "a worn team must be willing to detour for a repair"
        );
    }

    #[test]
    fn integrity_only_changes_the_repair_priority() {
        for fraction in [1.0, 0.5, 0.0] {
            assert_eq!(
                PickupKind::Cash.virtual_player_priority_for_integrity(fraction),
                PickupKind::Cash.virtual_player_priority()
            );
            assert_eq!(
                PickupKind::Nitro.virtual_player_priority_for_integrity(fraction),
                PickupKind::Nitro.virtual_player_priority()
            );
        }
    }
}
