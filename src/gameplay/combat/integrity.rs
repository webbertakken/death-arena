//! The per-team vehicle durability and wreck STATE: how worn each side's cars are
//! and which teams crossed into a wreck this frame.
//!
//! The combat model's per-team vehicle STATE, split from the ram, wreck and
//! economy MECHANICS in the parent `combat` module that drive it.
//! [`VehicleIntegrity`] is the per-team durability pool ram wear is subtracted
//! from and repairs add back to; [`WreckEvents`] reports which teams crossed to
//! zero this frame, [`WreckCarriers`] which of them was hauling a flag when the
//! wreck landed, and [`CarriedFlag`] / [`flags_dropped_by_wrecks`] resolve the
//! capture-the-flag turnover a wreck forces. [`FirstBloodClaimed`] latches the
//! round's opening wreck, and [`BaseRepair`] / [`base_repair`] compute the
//! home-base pit recovery a retreating team earns. Pure state and pure functions
//! with no ECS reach: driven by the parent's [`super::ram_damage_system`] and its
//! decay/reset systems, and read by the movement systems and the wreck economy.

use super::{
    TeamDamage, BASE_REPAIR_PER_FRAME, BASE_REPAIR_RADIUS, MAX_INTEGRITY,
    MIN_INTEGRITY_SPEED_MULTIPLIER, REPAIR_INTEGRITY,
};
use crate::gameplay::virtual_player::ai::AiTeam;
use bevy::prelude::*;

/// Per-team vehicle durability, mirroring [`crate::gameplay::pickup::NitroBoosts`].
///
/// Cars wear down by ramming opposing cars and are patched up by repair
/// pickups. A battered team drives slower, so trading paint with the flag
/// carrier becomes a real way to slow them down, the classic Death Rally way.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct VehicleIntegrity {
    /// Durability of the player team (the human and blue virtual players).
    pub player: f32,
    /// Durability of the opponent team (the red virtual players).
    pub opponent: f32,
}

impl Default for VehicleIntegrity {
    fn default() -> Self {
        Self {
            player: MAX_INTEGRITY,
            opponent: MAX_INTEGRITY,
        }
    }
}

impl VehicleIntegrity {
    /// Speed multiplier the player team's wear translates to.
    #[must_use]
    pub fn player_multiplier(self) -> f32 {
        integrity_speed_multiplier(self.player)
    }

    /// Speed multiplier the opponent team's wear translates to.
    #[must_use]
    pub fn opponent_multiplier(self) -> f32 {
        integrity_speed_multiplier(self.opponent)
    }

    /// Durability fraction (`0.0`..=`1.0`) of the given team's own wear.
    ///
    /// Virtual players price repair pickups against their *own* team's wear: a
    /// pristine team gains nothing from a patch-up (durability is capped at
    /// [`MAX_INTEGRITY`]), so it must not chase one however battered the enemy is.
    #[must_use]
    pub fn fraction_for_team(self, team: AiTeam) -> f32 {
        let integrity = match team {
            AiTeam::Blue => self.player,
            AiTeam::Red => self.opponent,
        };
        (integrity / MAX_INTEGRITY).clamp(0.0, 1.0)
    }

    /// Speed multiplier for the given team's current wear.
    #[must_use]
    pub fn multiplier_for_team(self, team: AiTeam) -> f32 {
        match team {
            AiTeam::Blue => self.player_multiplier(),
            AiTeam::Red => self.opponent_multiplier(),
        }
    }

    /// Patches up a team's durability, capped at [`MAX_INTEGRITY`].
    pub fn repair(&mut self, team: AiTeam) {
        match team {
            AiTeam::Blue => self.player = (self.player + REPAIR_INTEGRITY).min(MAX_INTEGRITY),
            AiTeam::Red => self.opponent = (self.opponent + REPAIR_INTEGRITY).min(MAX_INTEGRITY),
        }
    }

    /// Wears down a team's durability, floored at zero.
    pub fn apply_damage(&mut self, damage: TeamDamage) {
        self.player = (self.player - damage.player).max(0.0);
        self.opponent = (self.opponent - damage.opponent).max(0.0);
    }

    /// Patches each team up by its home-base pit recovery, capped at
    /// [`MAX_INTEGRITY`]. The recovery mirror of [`Self::apply_damage`]: where ram
    /// wear is subtracted, a team parked on its home turf adds durability back.
    pub fn apply_base_repair(&mut self, repair: BaseRepair) {
        self.player = (self.player + repair.player).min(MAX_INTEGRITY);
        self.opponent = (self.opponent + repair.opponent).min(MAX_INTEGRITY);
    }

    /// Teams this frame's wear drove from operational (`> 0`) to a full wreck
    /// (`0`), given the durability `before` the damage was applied.
    ///
    /// The crossing is the trigger: a team already flat-lined when `before` was
    /// taken does not re-fire, so each wreck is reported exactly once until a
    /// repair lifts the team back above zero and it can be wrecked anew. This is
    /// what makes [`super::WRECK_CASH_BOUNTY`] pay per wreck rather than per frame.
    #[must_use]
    pub fn newly_wrecked(self, before: Self) -> WreckEvents {
        WreckEvents {
            player: before.player > 0.0 && self.player <= 0.0,
            opponent: before.opponent > 0.0 && self.opponent <= 0.0,
        }
    }
}

/// Which teams crossed into a full wreck (zero integrity) this frame.
///
/// A wreck pays the wrecking team a [`super::WRECK_CASH_BOUNTY`]: a wrecked player team
/// banks the bounty for the opponents, a wrecked opponent team banks it for the
/// player team.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WreckEvents {
    /// The player team (human + blue virtual players) was wrecked this frame.
    pub player: bool,
    /// The opponent team (red virtual players) was wrecked this frame.
    pub opponent: bool,
}

impl WreckEvents {
    /// Whether either team was wrecked this frame.
    #[must_use]
    pub const fn any(self) -> bool {
        self.player || self.opponent
    }

    /// Whether the given team was among those wrecked this frame.
    #[must_use]
    pub const fn includes(self, team: AiTeam) -> bool {
        match team {
            AiTeam::Blue => self.player,
            AiTeam::Red => self.opponent,
        }
    }
}

/// Which teams had a car hauling the enemy flag the frame a wreck landed.
///
/// Read before a wreck knocks flags loose so the carrier-takedown bonus can tell
/// which side's wreck cut down a flag runner and denied a capture in flight. The
/// per-team companion to [`WreckEvents`], mirroring its `{ player, opponent }`
/// shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WreckCarriers {
    /// The player team had a car hauling the enemy flag this frame.
    pub player: bool,
    /// The opponent team had a car hauling the enemy flag this frame.
    pub opponent: bool,
}

/// A flag currently being hauled by a car, tagged with the carrying team.
///
/// Bridges the per-team [`WreckEvents`] to the per-flag CTF state so a wreck can
/// strip the flag from whichever car was carrying it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarriedFlag {
    /// The flag entity being hauled.
    pub flag: Entity,
    /// The team of the car hauling it.
    pub carrier_team: AiTeam,
}

/// Flags a freshly wrecked team must drop this frame.
///
/// The classic capture-the-flag turnover: when a team's integrity is ground to
/// zero its cars spin out, and a spun-out wreck cannot keep its grip on a stolen
/// flag. Every flag carried by a member of a newly wrecked team is dropped where
/// it lies, handing the wrecking team a real shot at recovering it and closing
/// the loop the ramming systems open (steal -> slowed + fragile -> rammed ->
/// wrecked -> drop the flag). Symmetric: both teams' carriers are equally fragile.
#[must_use]
pub fn flags_dropped_by_wrecks(carried: &[CarriedFlag], wrecks: WreckEvents) -> Vec<Entity> {
    carried
        .iter()
        .filter(|held| wrecks.includes(held.carrier_team))
        .map(|held| held.flag)
        .collect()
}

/// Tracks whether first blood has been drawn in the current round.
///
/// First blood is the round's opening wreck, so its [`super::FIRST_BLOOD_CASH_BONUS`] is
/// paid exactly once. This latch flips the frame any wreck lands while it is still
/// unclaimed and resets when a fresh match begins, mirroring
/// [`crate::gameplay::ctf::MatchPursePaid`]: a one-shot per-round flag read by its
/// own system rather than diffed at each award site.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirstBloodClaimed(pub bool);

/// Maps a durability value onto the linear speed penalty it imposes.
fn integrity_speed_multiplier(integrity: f32) -> f32 {
    let fraction = (integrity / MAX_INTEGRITY).clamp(0.0, 1.0);
    (1.0 - MIN_INTEGRITY_SPEED_MULTIPLIER).mul_add(fraction, MIN_INTEGRITY_SPEED_MULTIPLIER)
}

/// Durability each team regains from home-base pit recovery in a single frame.
///
/// The recovery mirror of [`TeamDamage`]: where ram wear is subtracted from a
/// team's pool, this is added back for a team that has retreated to its own base.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BaseRepair {
    pub player: f32,
    pub opponent: f32,
}

/// Computes the home-base pit recovery each team earns from the current car
/// positions.
///
/// A team with at least one car within [`BASE_REPAIR_RADIUS`] of its own home
/// base regains [`BASE_REPAIR_PER_FRAME`] durability; a team with no car home
/// regains nothing. Presence is binary, one car home patches the whole team pool
/// (matching the per-team integrity model), so massing cars at base grants no
/// extra heal. A car loitering in the *enemy* base earns its own team nothing,
/// so the recovery only ever rewards retreating to home turf.
#[must_use]
pub fn base_repair(cars: &[(AiTeam, Vec2)], blue_home: Vec2, red_home: Vec2) -> BaseRepair {
    let radius_sq = BASE_REPAIR_RADIUS * BASE_REPAIR_RADIUS;
    let home_for = |team: AiTeam| match team {
        AiTeam::Blue => blue_home,
        AiTeam::Red => red_home,
    };
    let team_at_home = |team: AiTeam| {
        cars.iter().any(|&(car_team, position)| {
            car_team == team && position.distance_squared(home_for(team)) <= radius_sq
        })
    };

    BaseRepair {
        player: if team_at_home(AiTeam::Blue) {
            BASE_REPAIR_PER_FRAME
        } else {
            0.0
        },
        opponent: if team_at_home(AiTeam::Red) {
            BASE_REPAIR_PER_FRAME
        } else {
            0.0
        },
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

    const BLUE_HOME: Vec2 = Vec2::new(-500.0, 0.0);
    const RED_HOME: Vec2 = Vec2::new(500.0, 0.0);

    #[test]
    fn integrity_defaults_to_full_for_both_teams() {
        let integrity = VehicleIntegrity::default();
        assert_near(integrity.player, MAX_INTEGRITY);
        assert_near(integrity.opponent, MAX_INTEGRITY);
    }

    #[test]
    fn full_integrity_imposes_no_speed_penalty() {
        let integrity = VehicleIntegrity::default();
        assert_near(integrity.player_multiplier(), 1.0);
        assert_near(integrity.opponent_multiplier(), 1.0);
    }

    #[test]
    fn zero_integrity_imposes_the_minimum_speed_multiplier() {
        let integrity = VehicleIntegrity {
            player: 0.0,
            opponent: 0.0,
        };
        assert_near(
            integrity.player_multiplier(),
            MIN_INTEGRITY_SPEED_MULTIPLIER,
        );
        assert_near(
            integrity.opponent_multiplier(),
            MIN_INTEGRITY_SPEED_MULTIPLIER,
        );
    }

    #[test]
    fn half_integrity_imposes_a_proportional_penalty() {
        let integrity = VehicleIntegrity {
            player: MAX_INTEGRITY / 2.0,
            opponent: MAX_INTEGRITY,
        };
        let expected =
            MIN_INTEGRITY_SPEED_MULTIPLIER + (1.0 - MIN_INTEGRITY_SPEED_MULTIPLIER) / 2.0;
        assert_near(integrity.player_multiplier(), expected);
    }

    #[test]
    fn fraction_for_team_reports_each_team_against_its_own_wear() {
        let integrity = VehicleIntegrity {
            player: MAX_INTEGRITY,
            opponent: MAX_INTEGRITY / 4.0,
        };
        assert_near(integrity.fraction_for_team(AiTeam::Blue), 1.0);
        assert_near(integrity.fraction_for_team(AiTeam::Red), 0.25);
    }

    #[test]
    fn fraction_for_team_clamps_into_the_unit_range() {
        let extremes = VehicleIntegrity {
            player: -MAX_INTEGRITY,
            opponent: MAX_INTEGRITY * 2.0,
        };
        assert_near(extremes.fraction_for_team(AiTeam::Blue), 0.0);
        assert_near(extremes.fraction_for_team(AiTeam::Red), 1.0);
    }

    #[test]
    fn multiplier_for_team_routes_to_the_right_pool() {
        let integrity = VehicleIntegrity {
            player: 0.0,
            opponent: MAX_INTEGRITY,
        };
        assert_near(
            integrity.multiplier_for_team(AiTeam::Blue),
            MIN_INTEGRITY_SPEED_MULTIPLIER,
        );
        assert_near(integrity.multiplier_for_team(AiTeam::Red), 1.0);
    }

    #[test]
    fn repair_restores_the_collecting_team_up_to_the_cap() {
        let mut integrity = VehicleIntegrity {
            player: 50.0,
            opponent: 50.0,
        };
        integrity.repair(AiTeam::Blue);
        assert_near(integrity.player, 50.0 + REPAIR_INTEGRITY);
        assert_near(integrity.opponent, 50.0);

        integrity.player = MAX_INTEGRITY - 5.0;
        integrity.repair(AiTeam::Blue);
        assert_near(integrity.player, MAX_INTEGRITY);
    }

    #[test]
    fn apply_damage_floors_each_team_at_zero() {
        let mut integrity = VehicleIntegrity {
            player: 10.0,
            opponent: 1.0,
        };
        integrity.apply_damage(TeamDamage {
            player: 4.0,
            opponent: 100.0,
        });
        assert_near(integrity.player, 6.0);
        assert_near(integrity.opponent, 0.0);
    }

    #[test]
    fn newly_wrecked_flags_each_team_that_crosses_to_zero() {
        let before = VehicleIntegrity {
            player: 5.0,
            opponent: 5.0,
        };

        let player_only = VehicleIntegrity {
            player: 0.0,
            opponent: 5.0,
        }
        .newly_wrecked(before);
        assert_eq!(
            player_only,
            WreckEvents {
                player: true,
                opponent: false,
            }
        );

        let both = VehicleIntegrity {
            player: 0.0,
            opponent: 0.0,
        }
        .newly_wrecked(before);
        assert_eq!(
            both,
            WreckEvents {
                player: true,
                opponent: true,
            }
        );
    }

    #[test]
    fn newly_wrecked_ignores_a_team_already_flatlined() {
        // The opponent was already wrecked when `before` was taken, so holding
        // at zero this frame must not re-fire the bounty.
        let before = VehicleIntegrity {
            player: 5.0,
            opponent: 0.0,
        };
        let after = VehicleIntegrity {
            player: 5.0,
            opponent: 0.0,
        };
        assert_eq!(after.newly_wrecked(before), WreckEvents::default());
    }

    #[test]
    fn newly_wrecked_ignores_a_team_still_operational() {
        let before = VehicleIntegrity {
            player: 5.0,
            opponent: 5.0,
        };
        let after = VehicleIntegrity {
            player: 4.0,
            opponent: 0.5,
        };
        assert_eq!(after.newly_wrecked(before), WreckEvents::default());
    }

    #[test]
    fn wreck_events_report_whether_any_team_fell() {
        assert!(!WreckEvents::default().any());
        assert!(WreckEvents {
            player: false,
            opponent: true,
        }
        .any());
    }

    #[test]
    fn wreck_events_report_per_team_membership() {
        let player_only = WreckEvents {
            player: true,
            opponent: false,
        };
        assert!(player_only.includes(AiTeam::Blue));
        assert!(!player_only.includes(AiTeam::Red));

        let opponent_only = WreckEvents {
            player: false,
            opponent: true,
        };
        assert!(!opponent_only.includes(AiTeam::Blue));
        assert!(opponent_only.includes(AiTeam::Red));
    }

    #[test]
    fn a_quiet_frame_drops_no_carried_flags() {
        let carried = [CarriedFlag {
            flag: Entity::from_raw(5),
            carrier_team: AiTeam::Blue,
        }];
        assert!(flags_dropped_by_wrecks(&carried, WreckEvents::default()).is_empty());
    }

    #[test]
    fn a_wrecked_team_drops_only_the_flag_it_was_hauling() {
        let blue_flag = Entity::from_raw(5);
        let red_flag = Entity::from_raw(6);
        let carried = [
            CarriedFlag {
                flag: blue_flag,
                carrier_team: AiTeam::Blue,
            },
            CarriedFlag {
                flag: red_flag,
                carrier_team: AiTeam::Red,
            },
        ];

        let dropped = flags_dropped_by_wrecks(
            &carried,
            WreckEvents {
                player: true,
                opponent: false,
            },
        );

        assert_eq!(dropped, vec![blue_flag]);
    }

    #[test]
    fn both_wrecked_teams_drop_their_flags() {
        let blue_flag = Entity::from_raw(5);
        let red_flag = Entity::from_raw(6);
        let carried = [
            CarriedFlag {
                flag: blue_flag,
                carrier_team: AiTeam::Blue,
            },
            CarriedFlag {
                flag: red_flag,
                carrier_team: AiTeam::Red,
            },
        ];

        let dropped = flags_dropped_by_wrecks(
            &carried,
            WreckEvents {
                player: true,
                opponent: true,
            },
        );

        assert_eq!(dropped, vec![blue_flag, red_flag]);
    }

    #[test]
    fn base_repair_heals_a_team_parked_in_its_own_base() {
        let cars = [(AiTeam::Blue, BLUE_HOME)];
        let repair = base_repair(&cars, BLUE_HOME, RED_HOME);
        assert_near(repair.player, BASE_REPAIR_PER_FRAME);
        assert_near(repair.opponent, 0.0);
    }

    #[test]
    fn base_repair_ignores_a_car_just_outside_its_base_radius() {
        let cars = [(
            AiTeam::Blue,
            BLUE_HOME + Vec2::new(BASE_REPAIR_RADIUS + 1.0, 0.0),
        )];
        let repair = base_repair(&cars, BLUE_HOME, RED_HOME);
        assert_near(repair.player, 0.0);
        assert_near(repair.opponent, 0.0);
    }

    #[test]
    fn base_repair_ignores_a_car_loitering_in_the_enemy_base() {
        // A blue car sitting on the red base earns neither team a heal.
        let cars = [(AiTeam::Blue, RED_HOME)];
        let repair = base_repair(&cars, BLUE_HOME, RED_HOME);
        assert_near(repair.player, 0.0);
        assert_near(repair.opponent, 0.0);
    }

    #[test]
    fn base_repair_heals_each_team_independently() {
        let cars = [(AiTeam::Blue, BLUE_HOME), (AiTeam::Red, RED_HOME)];
        let repair = base_repair(&cars, BLUE_HOME, RED_HOME);
        assert_near(repair.player, BASE_REPAIR_PER_FRAME);
        assert_near(repair.opponent, BASE_REPAIR_PER_FRAME);
    }

    #[test]
    fn base_repair_pools_per_team_so_a_second_home_car_adds_nothing() {
        let cars = [
            (AiTeam::Blue, BLUE_HOME),
            (AiTeam::Blue, BLUE_HOME + Vec2::new(10.0, 0.0)),
        ];
        let repair = base_repair(&cars, BLUE_HOME, RED_HOME);
        assert_near(repair.player, BASE_REPAIR_PER_FRAME);
    }

    #[test]
    fn apply_base_repair_caps_each_team_at_full_integrity() {
        let mut integrity = VehicleIntegrity::default();
        integrity.apply_base_repair(BaseRepair {
            player: BASE_REPAIR_PER_FRAME,
            opponent: BASE_REPAIR_PER_FRAME,
        });
        assert_near(integrity.player, MAX_INTEGRITY);
        assert_near(integrity.opponent, MAX_INTEGRITY);
    }

    #[test]
    fn apply_base_repair_lifts_only_the_recovering_team() {
        let mut integrity = VehicleIntegrity {
            player: 10.0,
            opponent: 50.0,
        };
        integrity.apply_base_repair(BaseRepair {
            player: BASE_REPAIR_PER_FRAME,
            opponent: 0.0,
        });
        assert_near(integrity.player, 10.0 + BASE_REPAIR_PER_FRAME);
        assert_near(integrity.opponent, 50.0);
    }
}
