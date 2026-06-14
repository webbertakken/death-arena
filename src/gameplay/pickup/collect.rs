/// Tactical value a wrecked team places on a repair pickup.
///
/// A repair is near worthless at full durability but becomes the most valuable
/// thing on the track for a wrecked team, outranking even nitro so a battered
/// team breaks off to patch up and win its lost speed back.
pub const REPAIR_MAX_VIRTUAL_PLAYER_PRIORITY: u32 = 175;

/// Tactical value a wrecked team places on a shield pickup.
///
/// A shield prevents wear rather than healing it, so it is most precious to a
/// team that is being ground down: it buys the breather to limp to a repair
/// without being wrecked anew. Pitched just below a wrecked team's repair value
/// so the heal still wins a straight choice, yet above nitro so a battered team
/// reaches for the breather before raw speed.
pub const SHIELD_MAX_VIRTUAL_PLAYER_PRIORITY: u32 = 160;

/// A wrecked team must still rate a heal over mere damage prevention, enforced
/// at compile time, so a repair always wins a straight repair-vs-shield choice.
const _: () = assert!(SHIELD_MAX_VIRTUAL_PLAYER_PRIORITY < REPAIR_MAX_VIRTUAL_PLAYER_PRIORITY);

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
    /// A sabotage charge that briefly slows the *enemy* team's engines: the
    /// classic Death Rally disruption item and the missing enemy-denial axis
    /// alongside self-speed (nitro), self-defence (shield) and heal (repair).
    /// Slowing a fleeing flag carrier makes it a real CTF chase tool. Pays the
    /// same small bounty as the other utility grabs.
    Sabotage,
}

impl PickupKind {
    /// Cash awarded for collecting this pickup.
    #[must_use]
    pub const fn bounty(self) -> u32 {
        match self {
            Self::Cash => 100,
            Self::Repair => 25,
            // A nitro canister, a shield and a sabotage charge are all modest
            // utility grabs.
            Self::Nitro | Self::Shield | Self::Sabotage => 50,
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
            // A touch above the shield: sabotage is proactive offence (deny the
            // enemy their speed, slow a fleeing carrier) rather than a defensive
            // edge, so a team reaches for it a little more readily, yet still
            // below nitro's raw race pressure so it never eclipses a flag run.
            Self::Sabotage => 130,
        }
    }

    /// Tactical value virtual players place on this pickup given their team's
    /// current durability `integrity_fraction` (`0.0` wrecked, `1.0` pristine).
    ///
    /// The two durability-driven pickups vary with wear: a repair (heal) and a
    /// shield (prevent further wear) both grow more valuable the more battered a
    /// team is. Every other pickup keeps its flat [`Self::virtual_player_priority`].
    #[must_use]
    pub fn virtual_player_priority_for_integrity(self, integrity_fraction: f32) -> u32 {
        match self {
            Self::Repair => repair_priority_for_integrity(integrity_fraction),
            Self::Shield => shield_priority_for_integrity(integrity_fraction),
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

/// Maps a team's durability fraction onto how hard its cars chase a shield.
///
/// The defensive mirror of [`repair_priority_for_integrity`]: a pristine team is
/// not under pressure and barely detours (kept below cash so it never trades a
/// cash grab for armour it does not need), while a battered team values the
/// breather more and more, crossing the narrow detour threshold when worn and
/// the wide one ([`crate::gameplay::virtual_player::ai::CTF_WIDE_DETOUR_MIN_PRIORITY`])
/// once it is genuinely battered. Capped below a wrecked team's repair value so
/// a heal still wins the straight choice.
#[must_use]
fn shield_priority_for_integrity(integrity_fraction: f32) -> u32 {
    if integrity_fraction <= 0.15 {
        SHIELD_MAX_VIRTUAL_PLAYER_PRIORITY
    } else if integrity_fraction <= 0.35 {
        150
    } else if integrity_fraction <= 0.55 {
        140
    } else if integrity_fraction <= 0.75 {
        120
    } else {
        90
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
    fn sabotage_pays_the_same_modest_bounty_as_nitro() {
        assert_eq!(PickupKind::Sabotage.bounty(), PickupKind::Nitro.bounty());
        assert!(PickupKind::Sabotage.bounty() < PickupKind::Cash.bounty());
    }

    #[test]
    fn virtual_players_rate_sabotage_between_cash_and_nitro() {
        let sabotage = PickupKind::Sabotage.virtual_player_priority();
        assert!(
            sabotage > PickupKind::Cash.virtual_player_priority(),
            "a sabotage should outrank cash so a team detours to deny the enemy: {sabotage}"
        );
        assert!(
            sabotage < PickupKind::Nitro.virtual_player_priority(),
            "a sabotage should not eclipse nitro's race pressure: {sabotage}"
        );
        assert!(
            sabotage > PickupKind::Shield.virtual_player_priority(),
            "proactive offence should edge out a purely defensive shield: {sabotage}"
        );
    }

    #[test]
    fn team_will_take_a_narrow_detour_for_a_sabotage() {
        use crate::gameplay::virtual_player::ai::{
            CTF_PICKUP_DETOUR_MIN_PRIORITY, CTF_WIDE_DETOUR_MIN_PRIORITY,
        };
        let sabotage = PickupKind::Sabotage.virtual_player_priority();
        assert!(
            sabotage >= CTF_PICKUP_DETOUR_MIN_PRIORITY,
            "a team must be willing to take at least a narrow detour for a sabotage"
        );
        assert!(
            sabotage < CTF_WIDE_DETOUR_MIN_PRIORITY,
            "a sabotage must not pull a car off a committed flag run the way nitro can"
        );
    }

    #[test]
    fn integrity_does_not_change_sabotage_priority() {
        for fraction in [1.0, 0.5, 0.0] {
            assert_eq!(
                PickupKind::Sabotage.virtual_player_priority_for_integrity(fraction),
                PickupKind::Sabotage.virtual_player_priority(),
                "sabotage value is about denying the enemy, not the team's own wear"
            );
        }
    }

    #[test]
    fn shield_priority_rises_as_durability_drops() {
        let fractions = [1.0, 0.7, 0.5, 0.3, 0.1, 0.0];
        let priorities: Vec<u32> = fractions
            .iter()
            .map(|&fraction| PickupKind::Shield.virtual_player_priority_for_integrity(fraction))
            .collect();

        for pair in priorities.windows(2) {
            assert!(
                pair[1] >= pair[0],
                "shield priority must not fall as a team wears down: {priorities:?}"
            );
        }
        assert!(
            priorities.last() > priorities.first(),
            "a battered team must value a shield more than a healthy one: {priorities:?}"
        );
    }

    #[test]
    fn pristine_team_values_a_shield_below_cash() {
        assert!(
            PickupKind::Shield.virtual_player_priority_for_integrity(1.0)
                < PickupKind::Cash.virtual_player_priority(),
            "a healthy, unpressured team must not trade a cash grab for armour it does not need"
        );
    }

    #[test]
    fn worn_team_will_detour_for_a_shield() {
        use crate::gameplay::virtual_player::ai::CTF_PICKUP_DETOUR_MIN_PRIORITY;
        assert!(
            PickupKind::Shield.virtual_player_priority_for_integrity(0.6)
                >= CTF_PICKUP_DETOUR_MIN_PRIORITY,
            "a worn team must be willing to take at least a narrow detour for a shield"
        );
    }

    #[test]
    fn battered_team_takes_a_wide_detour_for_a_shield() {
        use crate::gameplay::virtual_player::ai::{
            CTF_PICKUP_DETOUR_MIN_PRIORITY, CTF_WIDE_DETOUR_MIN_PRIORITY,
        };
        let battered = PickupKind::Shield.virtual_player_priority_for_integrity(0.2);
        assert!(
            battered >= CTF_WIDE_DETOUR_MIN_PRIORITY,
            "a battered team must commit a wide detour to grab a breather: {battered}"
        );
        assert!(battered > PickupKind::Cash.virtual_player_priority());
        assert!(battered > CTF_PICKUP_DETOUR_MIN_PRIORITY);
    }

    #[test]
    fn wrecked_team_still_prefers_a_repair_over_a_shield() {
        let shield = PickupKind::Shield.virtual_player_priority_for_integrity(0.0);
        let repair = PickupKind::Repair.virtual_player_priority_for_integrity(0.0);
        assert_eq!(shield, SHIELD_MAX_VIRTUAL_PLAYER_PRIORITY);
        assert!(
            shield < repair,
            "a heal (repair={repair}) must still beat mere prevention (shield={shield})"
        );
        assert!(
            shield > PickupKind::Nitro.virtual_player_priority(),
            "a wrecked team must reach for the breather before raw speed: shield={shield}"
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
    fn integrity_does_not_change_cash_or_nitro_priority() {
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
