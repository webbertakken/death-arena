//! The ram-damage classification cluster: how a frame's car positions and
//! headings classify into the per-team durability each angle of ram inflicts.
//!
//! The pure geometry-and-damage half of the combat model, split from the
//! ram/wreck/stun/surge ECS *mechanics* in the parent `combat` module that drive
//! it (and the wreck *pricing policy* already carved into `economy`, the per-team
//! frame timers into `timers`). Every function here is a pure rule over a slice of
//! [`RamCar`] contacts, together with the live nitro/surge/shield flags and the
//! arena half-extents, producing the [`TeamDamage`] each side bleeds this frame:
//! the base scrape, the nitro/surge/carrier bleeds and every directional angle
//! (aggressor, broadside, rear-end, head-on, pincer, wall and corner crush),
//! folded together and shield-mitigated by [`frame_ram_damage`]. Nothing here
//! touches the ECS world; the parent's [`super::ram_damage_system`] feeds these
//! results into the per-team [`super::VehicleIntegrity`] pool.

use super::{
    WreckSurges, AGGRESSOR_RAM_ALIGNMENT, AGGRESSOR_RAM_DAMAGE_PER_FRAME,
    BROADSIDE_RAM_DAMAGE_PER_FRAME, BROADSIDE_RAM_FLANK_THRESHOLD,
    CORNER_CRUSH_RAM_DAMAGE_PER_FRAME, FLAG_CARRIER_RAM_DAMAGE_PER_FRAME,
    HEAD_ON_RAM_DAMAGE_PER_FRAME, NITRO_RAM_DAMAGE_PER_FRAME, PINCER_MAX_EXTRA_ATTACKERS,
    PINCER_MIN_ATTACKERS, PINCER_RAM_DAMAGE_PER_EXTRA_ATTACKER, PINCER_RAM_DAMAGE_PER_FRAME,
    RAM_DAMAGE_PER_FRAME, RAM_RADIUS, REAR_END_RAM_DAMAGE_PER_FRAME, SHIELD_DAMAGE_MULTIPLIER,
    SURGE_RAM_DAMAGE_PER_FRAME, WALL_CRUSH_MARGIN, WALL_CRUSH_RAM_DAMAGE_PER_FRAME,
};
use crate::gameplay::pickup::{ArmourBoosts, NitroBoosts};
use crate::gameplay::virtual_player::ai::AiTeam;
use bevy::prelude::*;

/// A car considered for ram damage this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RamCar {
    pub team: AiTeam,
    pub position: Vec2,
    /// The car's facing direction, used by [`aggressor_ram_damage`] to tell a
    /// committed head-first charge from an incidental side-scrape.
    pub forward: Vec2,
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

/// Which teams are surging from a fresh wreck this frame, for offensive ram
/// bonuses.
///
/// The wreck-reward mirror of [`RamBoost`]: where a nitro boost makes a team's
/// rams bite, a wreck surge does the same for the team that just landed a kill,
/// carrying the kill's adrenaline into its next hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RamSurge {
    pub player: bool,
    pub opponent: bool,
}

impl RamSurge {
    /// Reads the live surge timers into the teams that are currently surging.
    #[must_use]
    pub const fn from_surges(surges: WreckSurges) -> Self {
        Self {
            player: surges.player_frames > 0,
            opponent: surges.opponent_frames > 0,
        }
    }

    const fn is_team_surging(self, team: AiTeam) -> bool {
        match team {
            AiTeam::Blue => self.player,
            AiTeam::Red => self.opponent,
        }
    }
}

/// Which teams have their shield up this frame, for defensive damage mitigation.
///
/// The defensive mirror of [`RamBoost`]: where a boost makes a team's rams bite,
/// a shield blunts the ram damage that team *takes*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RamShield {
    pub player: bool,
    pub opponent: bool,
}

impl RamShield {
    /// Reads the live shield timers into the teams that are currently armoured.
    #[must_use]
    pub const fn from_armour(boosts: &ArmourBoosts) -> Self {
        Self {
            player: boosts.is_player_active(),
            opponent: boosts.is_opponent_active(),
        }
    }
}

/// Blunts the ram damage a shielded team takes by [`SHIELD_DAMAGE_MULTIPLIER`].
///
/// Applied once to the already-summed frame damage, so a shield mitigates every
/// ram source at once (base scrape, nitro charge, aggressor hit, carrier bleed).
/// An unshielded team's damage passes through untouched.
#[must_use]
pub fn armour_mitigated_damage(damage: TeamDamage, shield: RamShield) -> TeamDamage {
    TeamDamage {
        player: if shield.player {
            damage.player * SHIELD_DAMAGE_MULTIPLIER
        } else {
            damage.player
        },
        opponent: if shield.opponent {
            damage.opponent * SHIELD_DAMAGE_MULTIPLIER
        } else {
            damage.opponent
        },
    }
}

/// Sums every ram-damage source for the frame and applies shield mitigation.
///
/// Folds the base scrape together with the nitro charge, the wreck surge, the
/// flag-carrier bleed and every directional bonus (aggressor, broadside,
/// rear-end, head-on, pincer, wall and corner crush), then blunts the total for
/// any shielded team. Pulled out of [`super::ram_damage_system`] so the system stays
/// under the line gate and the whole damage stack can be exercised in isolation.
#[must_use]
pub fn frame_ram_damage(
    cars: &[RamCar],
    boost: RamBoost,
    surge: RamSurge,
    shield: RamShield,
    half_extents: Vec2,
) -> TeamDamage {
    let raw_damage = ram_damage(cars)
        .combined(nitro_ram_damage(cars, boost))
        .combined(surge_ram_damage(cars, surge))
        .combined(carrier_ram_damage(cars))
        .combined(aggressor_ram_damage(cars))
        .combined(broadside_ram_damage(cars))
        .combined(rear_end_ram_damage(cars))
        .combined(head_on_ram_damage(cars))
        .combined(pincer_ram_damage(cars))
        .combined(wall_crush_ram_damage(cars, half_extents))
        .combined(corner_crush_ram_damage(cars, half_extents));
    // A team with its shield up shrugs off part of every ram it eats this frame.
    armour_mitigated_damage(raw_damage, shield)
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

/// Computes the bonus ram damage cars surging from a fresh wreck inflict on the
/// enemy.
///
/// The wreck-reward mirror of [`nitro_ram_damage`]: for every surging car in
/// contact with an opposing car, the *enemy* team bleeds
/// [`SURGE_RAM_DAMAGE_PER_FRAME`] on top of the base [`ram_damage`] scrape, so the
/// adrenaline of a kill is carried into the surging team's next hit. The hit lands
/// on whoever the surging car is trading paint with, the same contact rule the
/// nitro charge uses and needing no aim, so it rewards pressing a reeling enemy in
/// the surge window the wreck opened. Charged once per surging car in contact,
/// mirroring the per-aggressor model of [`nitro_ram_damage`].
#[must_use]
pub fn surge_ram_damage(cars: &[RamCar], surge: RamSurge) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, car) in cars.iter().enumerate() {
        if !surge.is_team_surging(car.team) {
            continue;
        }

        let in_contact = cars.iter().enumerate().any(|(other_index, other)| {
            other_index != index
                && other.team != car.team
                && other.position.distance_squared(car.position) <= radius_sq
        });
        if in_contact {
            // The enemy of the surging car eats the extra hit.
            match car.team {
                AiTeam::Blue => damage.opponent += SURGE_RAM_DAMAGE_PER_FRAME,
                AiTeam::Red => damage.player += SURGE_RAM_DAMAGE_PER_FRAME,
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

/// Computes the bonus ram damage cars charging head-first into an enemy deal.
///
/// A car is "charging" when an opposing car sits within [`RAM_RADIUS`] and
/// inside the forward cone set by [`AGGRESSOR_RAM_ALIGNMENT`]. For every such
/// car the *enemy* team bleeds [`AGGRESSOR_RAM_DAMAGE_PER_FRAME`] on top of the
/// base [`ram_damage`] scrape, so lining up a ram beats stumbling into one. A
/// head-on collision charges both cars, wearing both teams down at once.
#[must_use]
pub fn aggressor_ram_damage(cars: &[RamCar]) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, car) in cars.iter().enumerate() {
        let Some(heading) = car.forward.try_normalize() else {
            continue;
        };

        let is_charging = cars.iter().enumerate().any(|(other_index, other)| {
            if other_index == index || other.team == car.team {
                return false;
            }
            let offset = other.position - car.position;
            if offset.length_squared() > radius_sq {
                return false;
            }
            offset
                .try_normalize()
                .is_some_and(|direction| heading.dot(direction) >= AGGRESSOR_RAM_ALIGNMENT)
        });
        if is_charging {
            // The enemy the charging car is aiming at eats the extra hit.
            match car.team {
                AiTeam::Blue => damage.opponent += AGGRESSOR_RAM_DAMAGE_PER_FRAME,
                AiTeam::Red => damage.player += AGGRESSOR_RAM_DAMAGE_PER_FRAME,
            }
        }
    }

    damage
}

/// Computes the bonus ram damage cars caught side-on by a charging enemy take.
///
/// A car is "broadsided" when an opposing car is within [`RAM_RADIUS`], is
/// charging it (the striker's nose inside the [`AGGRESSOR_RAM_ALIGNMENT`] cone,
/// the same commitment the aggressor bonus demands) and strikes from the
/// victim's flank (the approach falling inside the side arc set by
/// [`BROADSIDE_RAM_FLANK_THRESHOLD`]). Every broadsided car bleeds
/// [`BROADSIDE_RAM_DAMAGE_PER_FRAME`] into its *own* team's pool on top of the
/// base [`ram_damage`] scrape, so a clean T-bone wears the struck team down
/// faster than a head-on meeting. Charged once per struck car however many
/// enemies pile into its flank, mirroring [`carrier_ram_damage`].
#[must_use]
pub fn broadside_ram_damage(cars: &[RamCar]) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, victim) in cars.iter().enumerate() {
        let Some(victim_heading) = victim.forward.try_normalize() else {
            continue;
        };

        let is_broadsided = cars.iter().enumerate().any(|(other_index, striker)| {
            if other_index == index || striker.team == victim.team {
                return false;
            }
            let to_striker = striker.position - victim.position;
            if to_striker.length_squared() > radius_sq {
                return false;
            }
            let Some(approach) = to_striker.try_normalize() else {
                return false;
            };
            // The striker is committing to the hit: its nose is on the victim.
            let charging = striker
                .forward
                .try_normalize()
                .is_some_and(|heading| heading.dot(-approach) >= AGGRESSOR_RAM_ALIGNMENT);
            // The victim is caught square: the strike falls on its side arc.
            let flanked = victim_heading.dot(approach).abs() <= BROADSIDE_RAM_FLANK_THRESHOLD;
            charging && flanked
        });
        if is_broadsided {
            match victim.team {
                AiTeam::Blue => damage.player += BROADSIDE_RAM_DAMAGE_PER_FRAME,
                AiTeam::Red => damage.opponent += BROADSIDE_RAM_DAMAGE_PER_FRAME,
            }
        }
    }

    damage
}

/// Computes the bonus ram damage cars run down from behind by a charging enemy
/// take.
///
/// A car is "rear-ended" when an opposing car is within [`RAM_RADIUS`], is
/// charging it (the striker's nose inside the [`AGGRESSOR_RAM_ALIGNMENT`] cone,
/// the same commitment the aggressor and broadside bonuses demand) and strikes
/// from the victim's rear arc, the wedge *behind* the flank arc set by
/// [`BROADSIDE_RAM_FLANK_THRESHOLD`]. Every rear-ended car bleeds
/// [`REAR_END_RAM_DAMAGE_PER_FRAME`] into its *own* team's pool on top of the
/// base [`ram_damage`] scrape, so running a fleeing foe down wears it faster
/// than meeting it head-on. The rear arc starts exactly where the flank arc
/// ends, so a single strike is ever only a flank *or* a rear hit, never both.
/// Charged once per struck car however many enemies pile into its tail,
/// mirroring [`broadside_ram_damage`].
#[must_use]
pub fn rear_end_ram_damage(cars: &[RamCar]) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, victim) in cars.iter().enumerate() {
        let Some(victim_heading) = victim.forward.try_normalize() else {
            continue;
        };

        let is_rear_ended = cars.iter().enumerate().any(|(other_index, striker)| {
            if other_index == index || striker.team == victim.team {
                return false;
            }
            let to_striker = striker.position - victim.position;
            if to_striker.length_squared() > radius_sq {
                return false;
            }
            let Some(approach) = to_striker.try_normalize() else {
                return false;
            };
            // The striker is committing to the hit: its nose is on the victim.
            let charging = striker
                .forward
                .try_normalize()
                .is_some_and(|heading| heading.dot(-approach) >= AGGRESSOR_RAM_ALIGNMENT);
            // The victim is caught from behind: the strike falls past its flank
            // arc, onto the rear wedge where it faces dead away from the striker.
            let rear = victim_heading.dot(approach) < -BROADSIDE_RAM_FLANK_THRESHOLD;
            charging && rear
        });
        if is_rear_ended {
            match victim.team {
                AiTeam::Blue => damage.player += REAR_END_RAM_DAMAGE_PER_FRAME,
                AiTeam::Red => damage.opponent += REAR_END_RAM_DAMAGE_PER_FRAME,
            }
        }
    }

    damage
}

/// Computes the bonus ram damage two cars meeting nose-to-nose inflict on both
/// teams.
///
/// A car is in a "head-on" when an opposing car within [`RAM_RADIUS`] is charging
/// it nose-first (the other car's heading inside the [`AGGRESSOR_RAM_ALIGNMENT`]
/// cone) *while this car is charging straight back the same way*: both noses on
/// each other, the Death Rally game of chicken neither side flinched from. Every
/// car caught in such a meeting bleeds [`HEAD_ON_RAM_DAMAGE_PER_FRAME`] into its
/// *own* team's pool on top of the base [`ram_damage`] scrape and the mutual
/// [`aggressor_ram_damage`] charge, so a smash wears *both* teams down at once.
/// Unlike the one-sided [`broadside_ram_damage`] flank and
/// [`rear_end_ram_damage`] run-down, whose punishment falls on a victim that
/// cannot retaliate, a head-on shares the cost, which is why out-positioning a
/// foe into a T-bone or a tail charge beats meeting it head-on. Charged once per
/// car however many enemies it is trading noses with, mirroring the per-victim
/// model of [`broadside_ram_damage`].
#[must_use]
pub fn head_on_ram_damage(cars: &[RamCar]) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, car) in cars.iter().enumerate() {
        let Some(heading) = car.forward.try_normalize() else {
            continue;
        };

        let is_head_on = cars.iter().enumerate().any(|(other_index, other)| {
            if other_index == index || other.team == car.team {
                return false;
            }
            let to_other = other.position - car.position;
            if to_other.length_squared() > radius_sq {
                return false;
            }
            let Some(approach) = to_other.try_normalize() else {
                return false;
            };
            // This car is committing nose-first into the other.
            if heading.dot(approach) < AGGRESSOR_RAM_ALIGNMENT {
                return false;
            }
            // The other car is charging straight back, nose on this one.
            other.forward.try_normalize().is_some_and(|other_heading| {
                other_heading.dot(-approach) >= AGGRESSOR_RAM_ALIGNMENT
            })
        });
        if is_head_on {
            match car.team {
                AiTeam::Blue => damage.player += HEAD_ON_RAM_DAMAGE_PER_FRAME,
                AiTeam::Red => damage.opponent += HEAD_ON_RAM_DAMAGE_PER_FRAME,
            }
        }
    }

    damage
}

/// The pincer bite a single surrounded car takes from `attacker_count` enemies
/// hemming it in at once.
///
/// Below [`PINCER_MIN_ATTACKERS`] there is no pincer (just a ram, covered by the
/// base scrape), so the bonus is zero. At the minimum it is
/// [`PINCER_RAM_DAMAGE_PER_FRAME`]; every further attacker adds
/// [`PINCER_RAM_DAMAGE_PER_EXTRA_ATTACKER`] up to [`PINCER_MAX_EXTRA_ATTACKERS`]
/// extra, topping out at [`super::PINCER_MAX_RAM_DAMAGE_PER_FRAME`]. The single charge
/// scales with the swarm but never stacks into one hit per attacker, mirroring
/// the per-victim model of [`broadside_ram_damage`].
///
/// Accumulates the per-extra step without a `usize`-to-`f32` count cast (the
/// pedantic clippy gate forbids it), keeping the module's near-zero-cast
/// convention.
#[must_use]
pub const fn pincer_ram_bonus(attacker_count: usize) -> f32 {
    if attacker_count < PINCER_MIN_ATTACKERS {
        return 0.0;
    }
    let extra = attacker_count - PINCER_MIN_ATTACKERS;
    let capped = if extra > PINCER_MAX_EXTRA_ATTACKERS {
        PINCER_MAX_EXTRA_ATTACKERS
    } else {
        extra
    };
    // Accumulate the per-extra step by repeated addition: a small bounded loop
    // that avoids a `usize`-to-`f32` count cast the pedantic clippy gate forbids.
    let mut bonus = PINCER_RAM_DAMAGE_PER_FRAME;
    let mut step = 0;
    while step < capped {
        bonus += PINCER_RAM_DAMAGE_PER_EXTRA_ATTACKER;
        step += 1;
    }
    bonus
}

/// Computes the bonus ram damage cars hemmed in by a pincer of enemies take.
///
/// A car is "pincered" when at least [`PINCER_MIN_ATTACKERS`] opposing cars sit
/// within [`RAM_RADIUS`] at once: a gang-up with no lane left to escape. Every
/// such car bleeds [`pincer_ram_bonus`] for the size of its swarm into its *own*
/// team's pool on top of the base [`ram_damage`] scrape, so being outnumbered at
/// a point wears the surrounded team down faster, and the more foes pile in the
/// harder it is ground down, the Death Rally "they swarmed me" punishment.
/// Charged once per surrounded car (the single charge scales with the swarm but
/// never stacks per attacker), mirroring [`broadside_ram_damage`]; the bonus
/// rewards the converging team coordination the virtual-player brain already
/// drives (massing defenders, the finish-off hunter) without needing the aim the
/// directional bonuses demand.
#[must_use]
pub fn pincer_ram_damage(cars: &[RamCar]) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, victim) in cars.iter().enumerate() {
        let attackers = cars
            .iter()
            .enumerate()
            .filter(|&(other_index, other)| {
                other_index != index
                    && other.team != victim.team
                    && other.position.distance_squared(victim.position) <= radius_sq
            })
            .count();
        let bonus = pincer_ram_bonus(attackers);
        if bonus > 0.0 {
            match victim.team {
                AiTeam::Blue => damage.player += bonus,
                AiTeam::Red => damage.opponent += bonus,
            }
        }
    }

    damage
}

/// Per-axis report of which arena walls a `striker_position` is shoving a car at
/// `victim_position` into: `(pinned against a left/right wall, pinned against a
/// top/bottom wall)`.
///
/// On each axis the victim must sit within [`WALL_CRUSH_MARGIN`] of a wall and the
/// striker must be on its open side, so the approach drives the victim *into* the
/// boundary rather than away from it. [`is_pinned_against_wall`] needs either axis
/// pinned; [`is_pinned_in_corner`] needs both at once.
#[must_use]
pub fn wall_shove(
    victim_position: Vec2,
    striker_position: Vec2,
    half_extents: Vec2,
) -> (bool, bool) {
    let against_positive_x = victim_position.x >= half_extents.x - WALL_CRUSH_MARGIN;
    let against_negative_x = victim_position.x <= -(half_extents.x - WALL_CRUSH_MARGIN);
    let against_positive_y = victim_position.y >= half_extents.y - WALL_CRUSH_MARGIN;
    let against_negative_y = victim_position.y <= -(half_extents.y - WALL_CRUSH_MARGIN);
    let push = victim_position - striker_position;
    let horizontally_pinned =
        (against_positive_x && push.x > 0.0) || (against_negative_x && push.x < 0.0);
    let vertically_pinned =
        (against_positive_y && push.y > 0.0) || (against_negative_y && push.y < 0.0);
    (horizontally_pinned, vertically_pinned)
}

/// Whether a charging `striker_position` is shoving a car at `victim_position`
/// into an arena wall it is pinned against.
///
/// True when the charge drives the victim into either an x-axis side wall or a
/// y-axis end wall (see [`wall_shove`]), so a car wedged into a corner is pinned
/// by a charge toward either face.
#[must_use]
pub fn is_pinned_against_wall(
    victim_position: Vec2,
    striker_position: Vec2,
    half_extents: Vec2,
) -> bool {
    let (horizontally_pinned, vertically_pinned) =
        wall_shove(victim_position, striker_position, half_extents);
    horizontally_pinned || vertically_pinned
}

/// Whether a charging `striker_position` is wedging a car at `victim_position`
/// into an arena corner, shoving it into two perpendicular walls at once.
///
/// Where [`is_pinned_against_wall`] needs only one axis pinned, a corner needs
/// both (see [`wall_shove`]): the victim is within [`WALL_CRUSH_MARGIN`] of a side
/// wall and an end wall, and the charge drives it into each, sealing the escape
/// lane the boundary would otherwise leave open along the single wall.
#[must_use]
pub fn is_pinned_in_corner(
    victim_position: Vec2,
    striker_position: Vec2,
    half_extents: Vec2,
) -> bool {
    let (horizontally_pinned, vertically_pinned) =
        wall_shove(victim_position, striker_position, half_extents);
    horizontally_pinned && vertically_pinned
}

/// Computes the bonus ram damage cars crushed against an arena wall by a
/// charging enemy take.
///
/// A car is "wall-crushed" when an opposing car is within [`RAM_RADIUS`], is
/// charging it (the striker's nose inside the [`AGGRESSOR_RAM_ALIGNMENT`] cone,
/// the same commitment the directional bonuses demand) and shoves it into a wall
/// it is pinned against (see [`is_pinned_against_wall`]). Every crushed car
/// bleeds [`WALL_CRUSH_RAM_DAMAGE_PER_FRAME`] into its *own* team's pool on top
/// of the base [`ram_damage`] scrape, so cornering a foe against the boundary
/// where it cannot escape wears it down faster than open-field trading. Charged
/// once per crushed car however many enemies pin it, mirroring the per-victim
/// model of [`broadside_ram_damage`].
#[must_use]
pub fn wall_crush_ram_damage(cars: &[RamCar], half_extents: Vec2) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, victim) in cars.iter().enumerate() {
        let is_crushed = cars.iter().enumerate().any(|(other_index, striker)| {
            if other_index == index || striker.team == victim.team {
                return false;
            }
            let to_victim = victim.position - striker.position;
            if to_victim.length_squared() > radius_sq {
                return false;
            }
            let Some(approach) = to_victim.try_normalize() else {
                return false;
            };
            // The striker is committing to the hit: its nose is on the victim.
            let charging = striker
                .forward
                .try_normalize()
                .is_some_and(|heading| heading.dot(approach) >= AGGRESSOR_RAM_ALIGNMENT);
            charging && is_pinned_against_wall(victim.position, striker.position, half_extents)
        });
        if is_crushed {
            match victim.team {
                AiTeam::Blue => damage.player += WALL_CRUSH_RAM_DAMAGE_PER_FRAME,
                AiTeam::Red => damage.opponent += WALL_CRUSH_RAM_DAMAGE_PER_FRAME,
            }
        }
    }

    damage
}

/// Computes the bonus ram damage cars wedged into an arena corner by a charging
/// enemy take, on top of the single-wall [`wall_crush_ram_damage`] they already
/// eat.
///
/// A car is "corner-crushed" when an opposing car is within [`RAM_RADIUS`], is
/// charging it (the striker's nose inside the [`AGGRESSOR_RAM_ALIGNMENT`] cone, the
/// same commitment the directional bonuses demand) and shoves it into two
/// perpendicular walls at once (see [`is_pinned_in_corner`]). Every cornered car
/// bleeds [`CORNER_CRUSH_RAM_DAMAGE_PER_FRAME`] into its *own* team's pool on top
/// of the base [`ram_damage`] scrape and the single-wall crush the corner already
/// trips, so wedging a foe where two walls meet and no escape lane is left wears it
/// down faster still. Charged once per cornered car however many enemies pin it,
/// mirroring the per-victim model of [`broadside_ram_damage`].
#[must_use]
pub fn corner_crush_ram_damage(cars: &[RamCar], half_extents: Vec2) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, victim) in cars.iter().enumerate() {
        let is_cornered = cars.iter().enumerate().any(|(other_index, striker)| {
            if other_index == index || striker.team == victim.team {
                return false;
            }
            let to_victim = victim.position - striker.position;
            if to_victim.length_squared() > radius_sq {
                return false;
            }
            let Some(approach) = to_victim.try_normalize() else {
                return false;
            };
            // The striker is committing to the hit: its nose is on the victim.
            let charging = striker
                .forward
                .try_normalize()
                .is_some_and(|heading| heading.dot(approach) >= AGGRESSOR_RAM_ALIGNMENT);
            charging && is_pinned_in_corner(victim.position, striker.position, half_extents)
        });
        if is_cornered {
            match victim.team {
                AiTeam::Blue => damage.player += CORNER_CRUSH_RAM_DAMAGE_PER_FRAME,
                AiTeam::Red => damage.opponent += CORNER_CRUSH_RAM_DAMAGE_PER_FRAME,
            }
        }
    }

    damage
}
