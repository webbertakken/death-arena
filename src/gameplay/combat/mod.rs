use crate::gameplay::ctf::CtfMatchResult;
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
}

/// Durability each team loses from ramming in a single frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamDamage {
    pub player: f32,
    pub opponent: f32,
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

/// Wears down both teams whenever their cars are trading paint.
pub fn ram_damage_system(
    match_result: Option<Res<CtfMatchResult>>,
    mut integrity: ResMut<VehicleIntegrity>,
    player_query: Query<&Transform, With<Player>>,
    virtual_player_query: Query<(&VirtualPlayer, &Transform), Without<Player>>,
) {
    if match_result
        .as_ref()
        .is_some_and(|result| result.winner.is_some())
    {
        return;
    }

    let mut cars: Vec<RamCar> = Vec::new();
    if let Ok(transform) = player_query.get_single() {
        cars.push(RamCar {
            team: AiTeam::Blue,
            position: transform.translation.xy(),
        });
    }
    cars.extend(
        virtual_player_query
            .iter()
            .map(|(virtual_player, transform)| RamCar {
                team: virtual_player.team,
                position: transform.translation.xy(),
            }),
    );

    integrity.apply_damage(ram_damage(&cars));
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
        }
    }

    fn red(position: Vec2) -> RamCar {
        RamCar {
            team: AiTeam::Red,
            position,
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
