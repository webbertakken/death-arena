use crate::gameplay::ctf::{CtfFlag, CtfMatchResult};
use crate::gameplay::pickup::NitroBoosts;
use crate::gameplay::player::Player;
use crate::gameplay::virtual_player::ai::AiTeam;
use crate::gameplay::virtual_player::VirtualPlayer;
use crate::{App, AppState, Plugin};
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;

/// Maximum durability a team's cars carry into a match.
pub const MAX_INTEGRITY: f32 = 100.0;
/// Durability restored when a car collects a repair pickup.
pub const REPAIR_INTEGRITY: f32 = 35.0;
/// World-space distance two cars must be within to count as ramming.
///
/// Cars use a `ball(350)` collider scaled to `0.2`, so two of them touch when
/// their centres are roughly 140 units apart.
pub const RAM_RADIUS: f32 = 140.0;
/// Durability a team loses per car caught ramming, each fixed frame.
pub const RAM_DAMAGE_PER_FRAME: f32 = 0.25;
/// Speed multiplier applied to a fully wrecked (zero integrity) team.
pub const MIN_INTEGRITY_SPEED_MULTIPLIER: f32 = 0.65;
/// Extra durability the enemy of a nitro-boosted car loses each frame the two
/// are trading paint.
///
/// A boosted car is charging, so slamming it into an opponent while nitro burns
/// wears the enemy down twice as fast as the base scrape, the classic Death
/// Rally "boost into them to wreck them" play. It also closes the combat loop:
/// nitro ram -> battered enemy -> enemy breaks off for a repair.
pub const NITRO_RAM_DAMAGE_PER_FRAME: f32 = 0.5;
/// Extra durability a flag-carrying car's team loses each frame it is trading
/// paint with an enemy.
///
/// A car hauling the enemy flag is not just slow, it is fragile: defenders who
/// ram the carrier wear its team down twice as fast as the base scrape. This
/// deepens the capture-the-flag gauntlet, the run home becomes a real risk, not
/// a victory lap, and pairs with the flag-carrier slowdown so a battered
/// carrier crawls back into reach of its pursuers.
pub const FLAG_CARRIER_RAM_DAMAGE_PER_FRAME: f32 = 0.5;

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

    /// Durability fraction (`0.0`..=`1.0`) of whichever team is more battered.
    ///
    /// Virtual players use this to decide how hotly to contest repair pickups,
    /// so a single worn-down team is enough to make a repair worth chasing.
    #[must_use]
    pub fn most_battered_fraction(self) -> f32 {
        (self.player.min(self.opponent) / MAX_INTEGRITY).clamp(0.0, 1.0)
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
}

/// Maps a durability value onto the linear speed penalty it imposes.
fn integrity_speed_multiplier(integrity: f32) -> f32 {
    let fraction = (integrity / MAX_INTEGRITY).clamp(0.0, 1.0);
    (1.0 - MIN_INTEGRITY_SPEED_MULTIPLIER).mul_add(fraction, MIN_INTEGRITY_SPEED_MULTIPLIER)
}

/// A car considered for ram damage this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RamCar {
    pub team: AiTeam,
    pub position: Vec2,
    /// Whether this car is currently hauling the enemy flag, making it a
    /// fragile target for [`carrier_ram_damage`].
    pub carrying_flag: bool,
}

/// Durability each team loses from ramming in a single frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamDamage {
    pub player: f32,
    pub opponent: f32,
}

impl TeamDamage {
    /// Sums two frames' worth of damage, e.g. the base scrape plus a nitro ram.
    #[must_use]
    pub fn combined(self, other: Self) -> Self {
        Self {
            player: self.player + other.player,
            opponent: self.opponent + other.opponent,
        }
    }
}

/// Which teams are burning nitro this frame, for offensive ram bonuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RamBoost {
    pub player: bool,
    pub opponent: bool,
}

impl RamBoost {
    /// Reads the live nitro timers into the teams that are currently boosting.
    #[must_use]
    pub const fn from_nitro(boosts: &NitroBoosts) -> Self {
        Self {
            player: boosts.is_player_active(),
            opponent: boosts.is_opponent_active(),
        }
    }

    const fn is_team_boosting(self, team: AiTeam) -> bool {
        match team {
            AiTeam::Blue => self.player,
            AiTeam::Red => self.opponent,
        }
    }
}

/// Computes the ram damage each team takes from the current car positions.
///
/// A car is "ramming" when an opposing car sits within [`RAM_RADIUS`]. Every
/// such car bleeds [`RAM_DAMAGE_PER_FRAME`] into its own team's pool, so being
/// outnumbered in a scrum wears a team down faster.
#[must_use]
pub fn ram_damage(cars: &[RamCar]) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, car) in cars.iter().enumerate() {
        let in_contact = cars.iter().enumerate().any(|(other_index, other)| {
            other_index != index
                && other.team != car.team
                && other.position.distance_squared(car.position) <= radius_sq
        });
        if in_contact {
            match car.team {
                AiTeam::Blue => damage.player += RAM_DAMAGE_PER_FRAME,
                AiTeam::Red => damage.opponent += RAM_DAMAGE_PER_FRAME,
            }
        }
    }

    damage
}

/// Computes the bonus ram damage nitro-boosted cars inflict on the enemy.
///
/// For every boosted car in contact with an opposing car, the *enemy* team
/// bleeds [`NITRO_RAM_DAMAGE_PER_FRAME`] on top of the base [`ram_damage`]
/// scrape. The hit lands on whoever the boosted car is charging, so the
/// aggressor's nitro window is what makes ramming bite.
#[must_use]
pub fn nitro_ram_damage(cars: &[RamCar], boost: RamBoost) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, car) in cars.iter().enumerate() {
        if !boost.is_team_boosting(car.team) {
            continue;
        }

        let in_contact = cars.iter().enumerate().any(|(other_index, other)| {
            other_index != index
                && other.team != car.team
                && other.position.distance_squared(car.position) <= radius_sq
        });
        if in_contact {
            // The enemy of the boosted car eats the charge.
            match car.team {
                AiTeam::Blue => damage.opponent += NITRO_RAM_DAMAGE_PER_FRAME,
                AiTeam::Red => damage.player += NITRO_RAM_DAMAGE_PER_FRAME,
            }
        }
    }

    damage
}

/// Computes the bonus ram damage flag carriers bleed while trading paint.
///
/// For every car carrying the enemy flag that is in contact with an opposing
/// car, the carrier's *own* team bleeds [`FLAG_CARRIER_RAM_DAMAGE_PER_FRAME`] on
/// top of the base [`ram_damage`] scrape. The hit lands on the carrier's team,
/// so hauling the flag through a scrum is what makes it bite.
#[must_use]
pub fn carrier_ram_damage(cars: &[RamCar]) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, car) in cars.iter().enumerate() {
        if !car.carrying_flag {
            continue;
        }

        let in_contact = cars.iter().enumerate().any(|(other_index, other)| {
            other_index != index
                && other.team != car.team
                && other.position.distance_squared(car.position) <= radius_sq
        });
        if in_contact {
            match car.team {
                AiTeam::Blue => damage.player += FLAG_CARRIER_RAM_DAMAGE_PER_FRAME,
                AiTeam::Red => damage.opponent += FLAG_CARRIER_RAM_DAMAGE_PER_FRAME,
            }
        }
    }

    damage
}

/// Wears down both teams whenever their cars are trading paint.
pub fn ram_damage_system(
    match_result: Option<Res<CtfMatchResult>>,
    nitro_boosts: Option<Res<NitroBoosts>>,
    mut integrity: ResMut<VehicleIntegrity>,
    player_query: Query<(Entity, &Transform), With<Player>>,
    virtual_player_query: Query<(Entity, &VirtualPlayer, &Transform), Without<Player>>,
    flag_query: Query<&CtfFlag>,
) {
    if match_result
        .as_ref()
        .is_some_and(|result| result.winner.is_some())
    {
        return;
    }

    let carriers: Vec<Entity> = flag_query.iter().filter_map(|flag| flag.holder).collect();
    let is_carrying = |entity: Entity| carriers.contains(&entity);

    let mut cars: Vec<RamCar> = Vec::new();
    if let Ok((entity, transform)) = player_query.get_single() {
        cars.push(RamCar {
            team: AiTeam::Blue,
            position: transform.translation.xy(),
            carrying_flag: is_carrying(entity),
        });
    }
    cars.extend(
        virtual_player_query
            .iter()
            .map(|(entity, virtual_player, transform)| RamCar {
                team: virtual_player.team,
                position: transform.translation.xy(),
                carrying_flag: is_carrying(entity),
            }),
    );

    let boost = nitro_boosts
        .as_deref()
        .map(RamBoost::from_nitro)
        .unwrap_or_default();
    let damage = ram_damage(&cars)
        .combined(nitro_ram_damage(&cars, boost))
        .combined(carrier_ram_damage(&cars));
    integrity.apply_damage(damage);
}

fn reset_vehicle_integrity(mut integrity: ResMut<VehicleIntegrity>) {
    *integrity = VehicleIntegrity::default();
}

#[derive(Default)]
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VehicleIntegrity>()
            .add_system_set(
                SystemSet::on_enter(AppState::InGame).with_system(reset_vehicle_integrity),
            )
            .add_system(ram_damage_system);
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

    fn blue(position: Vec2) -> RamCar {
        RamCar {
            team: AiTeam::Blue,
            position,
            carrying_flag: false,
        }
    }

    fn red(position: Vec2) -> RamCar {
        RamCar {
            team: AiTeam::Red,
            position,
            carrying_flag: false,
        }
    }

    fn blue_carrier(position: Vec2) -> RamCar {
        RamCar {
            carrying_flag: true,
            ..blue(position)
        }
    }

    fn red_carrier(position: Vec2) -> RamCar {
        RamCar {
            carrying_flag: true,
            ..red(position)
        }
    }

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
    fn most_battered_fraction_tracks_the_worse_off_team() {
        let integrity = VehicleIntegrity {
            player: MAX_INTEGRITY,
            opponent: MAX_INTEGRITY / 4.0,
        };
        assert_near(integrity.most_battered_fraction(), 0.25);
    }

    #[test]
    fn full_integrity_reports_a_pristine_fraction() {
        assert_near(VehicleIntegrity::default().most_battered_fraction(), 1.0);
    }

    #[test]
    fn most_battered_fraction_clamps_into_the_unit_range() {
        let wrecked = VehicleIntegrity {
            player: 0.0,
            opponent: 0.0,
        };
        assert_near(wrecked.most_battered_fraction(), 0.0);

        let overfilled = VehicleIntegrity {
            player: MAX_INTEGRITY * 2.0,
            opponent: MAX_INTEGRITY * 2.0,
        };
        assert_near(overfilled.most_battered_fraction(), 1.0);
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
    fn no_damage_when_no_cars_are_touching() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS + 1.0, 0.0))];
        let damage = ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn touching_opponents_each_wear_down_their_own_team() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = ram_damage(&cars);
        assert_near(damage.player, RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn same_team_contact_deals_no_damage() {
        let cars = [blue(Vec2::ZERO), blue(Vec2::new(10.0, 0.0))];
        let damage = ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn outnumbered_team_takes_damage_per_car_in_contact() {
        // Two reds bracket a single blue; both reds and the blue are in contact.
        let cars = [
            blue(Vec2::ZERO),
            red(Vec2::new(50.0, 0.0)),
            red(Vec2::new(-50.0, 0.0)),
        ];
        let damage = ram_damage(&cars);
        assert_near(damage.player, RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 2.0 * RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn combined_sums_both_teams_damage() {
        let total = TeamDamage {
            player: 0.25,
            opponent: 0.5,
        }
        .combined(TeamDamage {
            player: 1.0,
            opponent: 0.25,
        });
        assert_near(total.player, 1.25);
        assert_near(total.opponent, 0.75);
    }

    #[test]
    fn no_nitro_means_no_ram_bonus() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = nitro_ram_damage(&cars, RamBoost::default());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn boosted_player_ram_wears_the_opponent() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = nitro_ram_damage(
            &cars,
            RamBoost {
                player: true,
                opponent: false,
            },
        );
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, NITRO_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn boosted_opponent_ram_wears_the_player() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = nitro_ram_damage(
            &cars,
            RamBoost {
                player: false,
                opponent: true,
            },
        );
        assert_near(damage.player, NITRO_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn boosted_car_out_of_contact_deals_no_bonus() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS + 1.0, 0.0))];
        let damage = nitro_ram_damage(
            &cars,
            RamBoost {
                player: true,
                opponent: true,
            },
        );
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn both_teams_boosting_each_wear_the_enemy() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = nitro_ram_damage(
            &cars,
            RamBoost {
                player: true,
                opponent: true,
            },
        );
        assert_near(damage.player, NITRO_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, NITRO_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn same_team_contact_deals_no_nitro_bonus() {
        let cars = [blue(Vec2::ZERO), blue(Vec2::new(10.0, 0.0))];
        let damage = nitro_ram_damage(
            &cars,
            RamBoost {
                player: true,
                opponent: true,
            },
        );
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn nitro_bonus_scales_per_boosted_car_in_contact() {
        // Two boosted reds bracket a single blue: both reds are charging the
        // lone blue, so the player team eats two ram hits this frame.
        let cars = [
            blue(Vec2::ZERO),
            red(Vec2::new(50.0, 0.0)),
            red(Vec2::new(-50.0, 0.0)),
        ];
        let damage = nitro_ram_damage(
            &cars,
            RamBoost {
                player: false,
                opponent: true,
            },
        );
        assert_near(damage.player, 2.0 * NITRO_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn an_empty_handed_car_bleeds_no_carrier_bonus() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = carrier_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_rammed_blue_carrier_wears_the_player_team() {
        let cars = [
            blue_carrier(Vec2::ZERO),
            red(Vec2::new(RAM_RADIUS - 10.0, 0.0)),
        ];
        let damage = carrier_ram_damage(&cars);
        assert_near(damage.player, FLAG_CARRIER_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_rammed_red_carrier_wears_the_opponent_team() {
        let cars = [
            red_carrier(Vec2::ZERO),
            blue(Vec2::new(RAM_RADIUS - 10.0, 0.0)),
        ];
        let damage = carrier_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, FLAG_CARRIER_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn a_carrier_out_of_contact_bleeds_no_carrier_bonus() {
        let cars = [
            blue_carrier(Vec2::ZERO),
            red(Vec2::new(RAM_RADIUS + 1.0, 0.0)),
        ];
        let damage = carrier_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_carrier_touched_only_by_a_teammate_bleeds_no_carrier_bonus() {
        // A blue carrier escorted by a blue teammate is not being defended
        // against, so the carrier tax must not fire.
        let cars = [blue_carrier(Vec2::ZERO), blue(Vec2::new(10.0, 0.0))];
        let damage = carrier_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn carrier_bonus_scales_per_defender_in_contact() {
        // Two reds bracket the lone blue carrier; the carrier eats the tax once
        // per frame regardless of how many defenders crowd it, because the tax
        // is charged to the carrier, not summed per defender.
        let cars = [
            blue_carrier(Vec2::ZERO),
            red(Vec2::new(50.0, 0.0)),
            red(Vec2::new(-50.0, 0.0)),
        ];
        let damage = carrier_ram_damage(&cars);
        assert_near(damage.player, FLAG_CARRIER_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn system_adds_carrier_ram_bonus_when_a_carrier_collides() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        app.add_system(ram_damage_system);
        let carrier = app
            .world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)))
            .id();
        // The blue carrier hauls the red flag, held by the human player.
        app.world.spawn((
            CtfFlag {
                team: crate::gameplay::ctf::FlagTeam::Red,
                home: Vec2::new(500.0, 0.0),
                holder: Some(carrier),
            },
            Transform::from_translation(Vec3::ZERO),
        ));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        // Base scrape wears both teams 0.25; the blue carrier also bleeds the
        // carrier tax on top because the red defender is trading paint with it.
        assert_near(
            integrity.player,
            MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME - FLAG_CARRIER_RAM_DAMAGE_PER_FRAME,
        );
        assert_near(integrity.opponent, MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn system_adds_nitro_ram_bonus_when_a_boosted_team_collides() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        let mut boosts = NitroBoosts::default();
        boosts.trigger_opponent();
        app.insert_resource(boosts);
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        // Base scrape wears both teams 0.25; the boosted reds also ram the
        // player team for the nitro bonus on top.
        assert_near(
            integrity.player,
            MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME - NITRO_RAM_DAMAGE_PER_FRAME,
        );
        assert_near(integrity.opponent, MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn system_wears_down_both_teams_when_cars_collide() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        assert_near(integrity.player, MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME);
        assert_near(integrity.opponent, MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn system_leaves_integrity_alone_once_the_match_is_decided() {
        use crate::gameplay::ctf::CtfMatchWinner;
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        app.insert_resource(CtfMatchResult {
            winner: Some(CtfMatchWinner::Player),
        });
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        assert_near(integrity.player, MAX_INTEGRITY);
        assert_near(integrity.opponent, MAX_INTEGRITY);
    }

    #[test]
    fn entering_match_resets_integrity_to_full() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: 12.0,
            opponent: 34.0,
        });
        app.add_system(reset_vehicle_integrity);

        app.update();

        assert_eq!(
            *app.world.resource::<VehicleIntegrity>(),
            VehicleIntegrity::default()
        );
    }

    fn player_stub() -> Player {
        Player {
            movement_speed: 0.0,
            rotation_speed: 0.0,
            engine_max_speed_multiplier: 0.0,
            forward_max_speed_base: 0.0,
            backward_max_speed_base: 0.0,
            wheels_turning_multiplier: 0.0,
        }
    }

    fn virtual_player_stub(team: AiTeam) -> VirtualPlayer {
        VirtualPlayer {
            team,
            movement_speed: 0.0,
            rotation_speed: 0.0,
            waypoints: vec![],
            current_waypoint: 0,
        }
    }
}
