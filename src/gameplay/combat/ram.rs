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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::combat::PINCER_MAX_RAM_DAMAGE_PER_FRAME;
    use crate::gameplay::main::BOUNDS;

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
            // Facing +Y, perpendicular to the +X contact axis these helpers
            // place cars on, so the base ram tests never trip the aggressor cone.
            forward: Vec2::Y,
            carrying_flag: false,
        }
    }

    fn red(position: Vec2) -> RamCar {
        RamCar {
            team: AiTeam::Red,
            position,
            forward: Vec2::Y,
            carrying_flag: false,
        }
    }

    /// A blue car at `position` charging head-first towards `target`.
    fn blue_facing(position: Vec2, target: Vec2) -> RamCar {
        RamCar {
            forward: (target - position).normalize_or_zero(),
            ..blue(position)
        }
    }

    /// A red car at `position` charging head-first towards `target`.
    fn red_facing(position: Vec2, target: Vec2) -> RamCar {
        RamCar {
            forward: (target - position).normalize_or_zero(),
            ..red(position)
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

    /// The arena half-extents the wall-crush tests pin cars against, matching the
    /// real arena's `BOUNDS / 2.0`.
    fn arena_half() -> Vec2 {
        BOUNDS / 2.0
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
    fn no_surge_means_no_ram_bonus() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = surge_ram_damage(&cars, RamSurge::default());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_surging_player_ram_wears_the_opponent() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = surge_ram_damage(
            &cars,
            RamSurge {
                player: true,
                opponent: false,
            },
        );
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, SURGE_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn a_surging_opponent_ram_wears_the_player() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = surge_ram_damage(
            &cars,
            RamSurge {
                player: false,
                opponent: true,
            },
        );
        assert_near(damage.player, SURGE_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_surging_car_out_of_contact_deals_no_bonus() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS + 1.0, 0.0))];
        let damage = surge_ram_damage(
            &cars,
            RamSurge {
                player: true,
                opponent: true,
            },
        );
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn same_team_contact_deals_no_surge_bonus() {
        let cars = [blue(Vec2::ZERO), blue(Vec2::new(10.0, 0.0))];
        let damage = surge_ram_damage(
            &cars,
            RamSurge {
                player: true,
                opponent: true,
            },
        );
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn surge_bonus_scales_per_surging_car_in_contact() {
        // Two surging reds bracket a single blue: both reds are pressing the lone
        // blue, so the player team eats two surge hits this frame.
        let cars = [
            blue(Vec2::ZERO),
            red(Vec2::new(50.0, 0.0)),
            red(Vec2::new(-50.0, 0.0)),
        ];
        let damage = surge_ram_damage(
            &cars,
            RamSurge {
                player: false,
                opponent: true,
            },
        );
        assert_near(damage.player, 2.0 * SURGE_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn ram_surge_reads_active_surge_timers() {
        let mut surges = WreckSurges::default();
        surges.trigger_opponent();
        let surge = RamSurge::from_surges(surges);
        assert!(!surge.player, "an idle team should not be surging");
        assert!(surge.opponent, "a freshly-wrecking team should be surging");
    }

    #[test]
    fn frame_ram_damage_folds_the_surge_into_the_stack_then_shields() {
        // A surging, shielded player car trading paint with a red: the frame total
        // is base scrape + surge, halved for the shielded player, proving the helper
        // both folds the surge into the stack and mitigates the whole frame.
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = frame_ram_damage(
            &cars,
            RamBoost::default(),
            RamSurge {
                player: true,
                opponent: false,
            },
            RamShield {
                player: true,
                opponent: false,
            },
            BOUNDS / 2.0,
        );
        assert_near(
            damage.player,
            RAM_DAMAGE_PER_FRAME * SHIELD_DAMAGE_MULTIPLIER,
        );
        assert_near(
            damage.opponent,
            RAM_DAMAGE_PER_FRAME + SURGE_RAM_DAMAGE_PER_FRAME,
        );
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
    fn a_side_scrape_inflicts_no_aggressor_bonus() {
        // Both cars face +Y while touching along the X axis: neither is charging
        // the other, so only the base scrape (handled elsewhere) applies.
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = aggressor_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_charging_blue_car_wears_the_opponent_it_aims_at() {
        let enemy = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [blue_facing(Vec2::ZERO, enemy), red(enemy)];
        let damage = aggressor_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, AGGRESSOR_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn a_charging_red_car_wears_the_player_it_aims_at() {
        let enemy = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [red_facing(Vec2::ZERO, enemy), blue(enemy)];
        let damage = aggressor_ram_damage(&cars);
        assert_near(damage.player, AGGRESSOR_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_head_on_collision_charges_both_teams() {
        let blue_pos = Vec2::ZERO;
        let red_pos = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [
            blue_facing(blue_pos, red_pos),
            red_facing(red_pos, blue_pos),
        ];
        let damage = aggressor_ram_damage(&cars);
        assert_near(damage.player, AGGRESSOR_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, AGGRESSOR_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn a_charge_at_a_distant_enemy_inflicts_no_aggressor_bonus() {
        let enemy = Vec2::new(RAM_RADIUS + 1.0, 0.0);
        let cars = [blue_facing(Vec2::ZERO, enemy), red(enemy)];
        let damage = aggressor_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn charging_a_teammate_inflicts_no_aggressor_bonus() {
        let mate = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [blue_facing(Vec2::ZERO, mate), blue(mate)];
        let damage = aggressor_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn facing_just_inside_the_cone_charges_but_just_outside_does_not() {
        // Place the enemy on the X axis and aim the car at the cone's edge by
        // rotating its heading until the dot product brackets the threshold.
        let enemy = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let inside_angle = AGGRESSOR_RAM_ALIGNMENT.acos() - 0.01;
        let outside_angle = AGGRESSOR_RAM_ALIGNMENT.acos() + 0.01;

        let inside = [
            RamCar {
                forward: Vec2::new(inside_angle.cos(), inside_angle.sin()),
                ..blue(Vec2::ZERO)
            },
            red(enemy),
        ];
        assert_near(
            aggressor_ram_damage(&inside).opponent,
            AGGRESSOR_RAM_DAMAGE_PER_FRAME,
        );

        let outside = [
            RamCar {
                forward: Vec2::new(outside_angle.cos(), outside_angle.sin()),
                ..blue(Vec2::ZERO)
            },
            red(enemy),
        ];
        assert_near(aggressor_ram_damage(&outside).opponent, 0.0);
    }

    #[test]
    fn aggressor_bonus_scales_per_charging_car_in_contact() {
        // Two reds both charge a lone blue from either side: the player team
        // eats one aggressor hit per charging car this frame.
        let blue_pos = Vec2::ZERO;
        let cars = [
            blue(blue_pos),
            red_facing(Vec2::new(50.0, 0.0), blue_pos),
            red_facing(Vec2::new(-50.0, 0.0), blue_pos),
        ];
        let damage = aggressor_ram_damage(&cars);
        assert_near(damage.player, 2.0 * AGGRESSOR_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_degenerate_heading_inflicts_no_aggressor_bonus() {
        let enemy = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [
            RamCar {
                forward: Vec2::ZERO,
                ..blue(Vec2::ZERO)
            },
            red(enemy),
        ];
        let damage = aggressor_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_side_on_charge_broadsides_the_struck_team() {
        // A red car charges in from the blue car's flank: blue faces +Y while
        // the red striker comes from +X with its nose on blue's door.
        let victim = Vec2::ZERO;
        let striker = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [blue(victim), red_facing(striker, victim)];
        let damage = broadside_ram_damage(&cars);
        assert_near(damage.player, BROADSIDE_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_flank_charge_wears_the_red_team_it_t_bones() {
        // The mirror: a blue car charges a red car square in the side.
        let victim = Vec2::ZERO;
        let striker = Vec2::new(-(RAM_RADIUS - 10.0), 0.0);
        let cars = [red(victim), blue_facing(striker, victim)];
        let damage = broadside_ram_damage(&cars);
        assert_near(damage.opponent, BROADSIDE_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.player, 0.0);
    }

    #[test]
    fn a_head_on_charge_is_no_broadside() {
        // Nose to nose: each car is hit on its front, not its flank, so the
        // broadside bonus stays silent and only the aggressor charge applies.
        let blue_pos = Vec2::ZERO;
        let red_pos = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [
            blue_facing(blue_pos, red_pos),
            red_facing(red_pos, blue_pos),
        ];
        let damage = broadside_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_parallel_scrape_is_no_broadside() {
        // Two cars running side by side, both facing +Y: a flank position alone
        // earns no broadside without a striker charging into it.
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = broadside_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_glancing_front_quarter_charge_is_no_broadside() {
        // The striker charges from 30 degrees off the victim's nose: inside the
        // aggressor cone but short of the side arc, a frontal clip not a T-bone.
        let victim = Vec2::ZERO;
        let angle = std::f32::consts::FRAC_PI_6;
        let striker = Vec2::new(angle.sin(), angle.cos()) * (RAM_RADIUS - 10.0);
        let cars = [blue(victim), red_facing(striker, victim)];
        let damage = broadside_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_broadside_needs_contact() {
        // A perfect side-on charge just out of ram range deals nothing.
        let victim = Vec2::ZERO;
        let striker = Vec2::new(RAM_RADIUS + 1.0, 0.0);
        let cars = [blue(victim), red_facing(striker, victim)];
        let damage = broadside_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_double_flank_charges_the_victim_once() {
        // Two reds T-bone a lone blue from both flanks: the struck car bleeds a
        // single broadside, not one per striker (the per-victim model).
        let victim = Vec2::ZERO;
        let cars = [
            blue(victim),
            red_facing(Vec2::new(50.0, 0.0), victim),
            red_facing(Vec2::new(-50.0, 0.0), victim),
        ];
        let damage = broadside_ram_damage(&cars);
        assert_near(damage.player, BROADSIDE_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_degenerate_heading_inflicts_no_broadside() {
        // A victim with no facing cannot be judged side-on, so it is skipped.
        let striker = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [
            RamCar {
                forward: Vec2::ZERO,
                ..blue(Vec2::ZERO)
            },
            red_facing(striker, Vec2::ZERO),
        ];
        let damage = broadside_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_tail_charge_rear_ends_the_struck_team() {
        // A red car runs the blue car down from directly behind: blue faces +Y
        // while the red striker chases from -Y with its nose on blue's tail.
        let victim = Vec2::ZERO;
        let striker = Vec2::new(0.0, -(RAM_RADIUS - 10.0));
        let cars = [blue(victim), red_facing(striker, victim)];
        let damage = rear_end_ram_damage(&cars);
        assert_near(damage.player, REAR_END_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_tail_charge_wears_the_red_team_it_runs_down() {
        // The mirror: a blue car runs a red car down from directly behind.
        let victim = Vec2::ZERO;
        let striker = Vec2::new(0.0, -(RAM_RADIUS - 10.0));
        let cars = [red(victim), blue_facing(striker, victim)];
        let damage = rear_end_ram_damage(&cars);
        assert_near(damage.opponent, REAR_END_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.player, 0.0);
    }

    #[test]
    fn a_head_on_charge_is_no_rear_end() {
        // Nose to nose: each car is struck on its front, not its tail, so the
        // rear-end bonus stays silent and only the aggressor charge applies.
        let blue_pos = Vec2::ZERO;
        let red_pos = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [
            blue_facing(blue_pos, red_pos),
            red_facing(red_pos, blue_pos),
        ];
        let damage = rear_end_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_side_on_charge_is_no_rear_end() {
        // A clean flank T-bone falls inside the side arc, short of the rear
        // wedge, so it earns a broadside but never a rear-end: the two arcs are
        // disjoint, and a single strike is one or the other, never both.
        let victim = Vec2::ZERO;
        let striker = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [blue(victim), red_facing(striker, victim)];
        assert_near(
            broadside_ram_damage(&cars).player,
            BROADSIDE_RAM_DAMAGE_PER_FRAME,
        );
        let damage = rear_end_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_tail_position_without_a_charge_is_no_rear_end() {
        // A red car sits dead behind the blue car but faces away (-Y), so it is
        // tailing without committing: a rear position alone earns no rear-end.
        let victim = Vec2::ZERO;
        let cars = [
            blue(victim),
            RamCar {
                forward: Vec2::NEG_Y,
                ..red(Vec2::new(0.0, -(RAM_RADIUS - 10.0)))
            },
        ];
        let damage = rear_end_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_rear_end_needs_contact() {
        // A perfect tail charge just out of ram range deals nothing.
        let victim = Vec2::ZERO;
        let striker = Vec2::new(0.0, -(RAM_RADIUS + 1.0));
        let cars = [blue(victim), red_facing(striker, victim)];
        let damage = rear_end_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_double_tail_charge_rear_ends_the_victim_once() {
        // Two reds pile into a lone blue's tail: the struck car bleeds a single
        // rear-end, not one per striker (the per-victim model).
        let victim = Vec2::ZERO;
        let cars = [
            blue(victim),
            red_facing(Vec2::new(0.0, -50.0), victim),
            red_facing(Vec2::new(0.0, -90.0), victim),
        ];
        let damage = rear_end_ram_damage(&cars);
        assert_near(damage.player, REAR_END_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_degenerate_heading_inflicts_no_rear_end() {
        // A victim with no facing cannot be judged from behind, so it is skipped.
        let striker = Vec2::new(0.0, -(RAM_RADIUS - 10.0));
        let cars = [
            RamCar {
                forward: Vec2::ZERO,
                ..blue(Vec2::ZERO)
            },
            red_facing(striker, Vec2::ZERO),
        ];
        let damage = rear_end_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_head_on_smash_wears_both_teams() {
        // Nose to nose: both cars commit a charge straight into each other, so
        // the smash bites both teams at once.
        let blue_pos = Vec2::ZERO;
        let red_pos = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [
            blue_facing(blue_pos, red_pos),
            red_facing(red_pos, blue_pos),
        ];
        let damage = head_on_ram_damage(&cars);
        assert_near(damage.player, HEAD_ON_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, HEAD_ON_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn a_one_sided_charge_is_no_head_on() {
        // The blue car charges nose-first while the red car keeps its +Y facing,
        // so the red is not charging back: a head-on needs both noses committed.
        let red_pos = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [blue_facing(Vec2::ZERO, red_pos), red(red_pos)];
        let damage = head_on_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_side_scrape_is_no_head_on() {
        // Both cars face +Y while touching along the X axis: neither nose is on
        // the other, so a parallel scrape earns no head-on smash.
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = head_on_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_head_on_smash_needs_contact() {
        // A perfect nose-to-nose pair just out of ram range deals nothing.
        let blue_pos = Vec2::ZERO;
        let red_pos = Vec2::new(RAM_RADIUS + 1.0, 0.0);
        let cars = [
            blue_facing(blue_pos, red_pos),
            red_facing(red_pos, blue_pos),
        ];
        let damage = head_on_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn charging_a_teammate_is_no_head_on() {
        // Two friendly cars meeting nose-to-nose are not enemies, so no smash.
        let a = Vec2::ZERO;
        let b = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [blue_facing(a, b), blue_facing(b, a)];
        let damage = head_on_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_degenerate_heading_is_no_head_on() {
        // One car has no facing, so the mutual-charge condition cannot hold and
        // neither side eats the smash.
        let blue_pos = Vec2::ZERO;
        let red_pos = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [
            RamCar {
                forward: Vec2::ZERO,
                ..blue(blue_pos)
            },
            red_facing(red_pos, blue_pos),
        ];
        let damage = head_on_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_head_on_charges_a_car_once_however_many_noses_it_meets() {
        // A lone blue noses into a wedge of two reds, both charging straight
        // back: the blue eats a single smash (the per-victim model) while each
        // red bleeds its own, so the two-car team takes twice the lone blue's.
        let blue_pos = Vec2::ZERO;
        let red_one = Vec2::new(100.0, 8.0);
        let red_two = Vec2::new(100.0, -8.0);
        let cars = [
            blue_facing(blue_pos, Vec2::new(100.0, 0.0)),
            red_facing(red_one, blue_pos),
            red_facing(red_two, blue_pos),
        ];
        let damage = head_on_ram_damage(&cars);
        assert_near(damage.player, HEAD_ON_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 2.0 * HEAD_ON_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn a_lone_ram_is_no_pincer() {
        // A single enemy in contact is just a ram, not a gang-up.
        let cars = [blue(Vec2::ZERO), red(Vec2::new(50.0, 0.0))];
        let damage = pincer_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn two_enemies_pincer_the_surrounded_team() {
        // Two reds bracket a lone blue: the blue is hemmed in by a pincer, while
        // each red faces only the single blue, so only the blue team bleeds.
        let cars = [
            blue(Vec2::ZERO),
            red(Vec2::new(50.0, 0.0)),
            red(Vec2::new(-50.0, 0.0)),
        ];
        let damage = pincer_ram_damage(&cars);
        assert_near(damage.player, PINCER_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_bigger_swarm_bites_harder_but_still_lands_once() {
        // Three reds swarm one blue: the struck car bleeds a single, swarm-scaled
        // pincer (the three-attacker bite), not one charge per attacker. The
        // per-victim model holds (mirroring the broadside bonus), it just scales.
        let cars = [
            blue(Vec2::ZERO),
            red(Vec2::new(50.0, 0.0)),
            red(Vec2::new(-50.0, 0.0)),
            red(Vec2::new(0.0, 50.0)),
        ];
        let damage = pincer_ram_damage(&cars);
        assert_near(damage.player, pincer_ram_bonus(3));
        assert!(
            damage.player > PINCER_RAM_DAMAGE_PER_FRAME,
            "a three-car swarm must out-bite a two-car pincer: {}",
            damage.player
        );
        assert!(
            damage.player < 3.0 * PINCER_RAM_DAMAGE_PER_FRAME,
            "the scaled charge must not stack one full pincer per attacker: {}",
            damage.player
        );
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn pincer_bonus_is_zero_below_the_minimum_gang_up() {
        assert_near(pincer_ram_bonus(0), 0.0);
        assert_near(pincer_ram_bonus(PINCER_MIN_ATTACKERS - 1), 0.0);
        assert_near(
            pincer_ram_bonus(PINCER_MIN_ATTACKERS),
            PINCER_RAM_DAMAGE_PER_FRAME,
        );
    }

    #[test]
    fn pincer_bonus_rises_with_every_extra_attacker() {
        let two = pincer_ram_bonus(2);
        let three = pincer_ram_bonus(3);
        let four = pincer_ram_bonus(4);
        assert_near(two, PINCER_RAM_DAMAGE_PER_FRAME);
        // Each extra attacker adds exactly one per-extra step to the bite.
        assert_near(two - PINCER_RAM_DAMAGE_PER_FRAME, 0.0);
        assert_near(three - two, PINCER_RAM_DAMAGE_PER_EXTRA_ATTACKER);
        assert_near(four - three, PINCER_RAM_DAMAGE_PER_EXTRA_ATTACKER);
        assert!(three > two && four > three, "swarm bite must escalate");
    }

    #[test]
    fn pincer_bonus_caps_at_the_swarm_ceiling() {
        let max = PINCER_MAX_RAM_DAMAGE_PER_FRAME;
        // One past the cap and a huge dogpile both land at the ceiling, no more.
        let beyond_cap = PINCER_MIN_ATTACKERS + PINCER_MAX_EXTRA_ATTACKERS + 1;
        assert_near(pincer_ram_bonus(beyond_cap), max);
        assert_near(pincer_ram_bonus(64), max);
        assert!(
            max < NITRO_RAM_DAMAGE_PER_FRAME,
            "even a maxed swarm must stay under the earned nitro charge: {max}"
        );
    }

    #[test]
    fn a_growing_swarm_grinds_the_victim_down_harder() {
        // The same lone blue, hemmed in by two then three then four reds, bleeds a
        // strictly heavier pincer each time another foe piles in.
        let two = pincer_ram_damage(&[
            blue(Vec2::ZERO),
            red(Vec2::new(50.0, 0.0)),
            red(Vec2::new(-50.0, 0.0)),
        ])
        .player;
        let three = pincer_ram_damage(&[
            blue(Vec2::ZERO),
            red(Vec2::new(50.0, 0.0)),
            red(Vec2::new(-50.0, 0.0)),
            red(Vec2::new(0.0, 50.0)),
        ])
        .player;
        let four = pincer_ram_damage(&[
            blue(Vec2::ZERO),
            red(Vec2::new(50.0, 0.0)),
            red(Vec2::new(-50.0, 0.0)),
            red(Vec2::new(0.0, 50.0)),
            red(Vec2::new(0.0, -50.0)),
        ])
        .player;
        assert!(
            three > two && four > three,
            "the surrounded team must bleed more as the swarm grows: {two} {three} {four}"
        );
    }

    #[test]
    fn friendly_crowding_is_no_pincer() {
        // A car flanked by its own teammates is not pincered: only enemies count.
        let cars = [
            blue(Vec2::ZERO),
            blue(Vec2::new(50.0, 0.0)),
            blue(Vec2::new(-50.0, 0.0)),
        ];
        let damage = pincer_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_pincer_needs_contact() {
        // Two reds bracket a blue but both sit out of ram range: no pincer.
        let cars = [
            blue(Vec2::ZERO),
            red(Vec2::new(RAM_RADIUS + 1.0, 0.0)),
            red(Vec2::new(-(RAM_RADIUS + 1.0), 0.0)),
        ];
        let damage = pincer_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_mutual_pincer_wears_both_teams() {
        // Two blues and two reds bunch together so every car has both enemies in
        // range: each of the four is pincered, so each team bleeds two pincers.
        let cars = [
            blue(Vec2::ZERO),
            blue(Vec2::new(20.0, 0.0)),
            red(Vec2::new(0.0, 20.0)),
            red(Vec2::new(20.0, 20.0)),
        ];
        let damage = pincer_ram_damage(&cars);
        assert_near(damage.player, 2.0 * PINCER_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 2.0 * PINCER_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn armour_halves_only_a_shielded_teams_damage() {
        let damage = TeamDamage {
            player: 2.0,
            opponent: 4.0,
        };
        let mitigated = armour_mitigated_damage(
            damage,
            RamShield {
                player: true,
                opponent: false,
            },
        );
        assert_near(mitigated.player, 2.0 * SHIELD_DAMAGE_MULTIPLIER);
        assert_near(mitigated.opponent, 4.0);
    }

    #[test]
    fn armour_passes_unshielded_damage_through_untouched() {
        let damage = TeamDamage {
            player: 2.0,
            opponent: 4.0,
        };
        let mitigated = armour_mitigated_damage(damage, RamShield::default());
        assert_near(mitigated.player, 2.0);
        assert_near(mitigated.opponent, 4.0);
    }

    #[test]
    fn ram_shield_reads_active_armour_timers() {
        let mut boosts = ArmourBoosts::default();
        boosts.trigger_opponent();
        let shield = RamShield::from_armour(&boosts);
        assert!(!shield.player, "an idle team should not be shielded");
        assert!(shield.opponent, "a triggered team should be shielded");
    }

    #[test]
    fn wall_crush_hits_a_car_pinned_against_a_wall_by_a_charging_enemy() {
        // A red victim shoved up against the +X wall; a blue striker charges +X
        // into it, leaving it nowhere to escape.
        let cars = [
            blue_facing(Vec2::new(810.0, 0.0), Vec2::new(900.0, 0.0)),
            red(Vec2::new(900.0, 0.0)),
        ];
        let damage = wall_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, WALL_CRUSH_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn wall_crush_spares_a_car_trading_paint_in_open_field() {
        // The same charge at the arena centre is just a ram, not a wall pin.
        let cars = [
            blue_facing(Vec2::new(-90.0, 0.0), Vec2::ZERO),
            red(Vec2::ZERO),
        ];
        let damage = wall_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn wall_crush_needs_a_charging_enemy_not_a_mere_wall_hugger() {
        // The red sits against the +X wall, but the blue alongside it faces +Y,
        // not into the victim, so it is not charging and no crush lands.
        let cars = [blue(Vec2::new(810.0, 0.0)), red(Vec2::new(900.0, 0.0))];
        let damage = wall_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn wall_crush_ignores_a_charge_from_the_wall_side() {
        // The blue striker is wedged between the red victim and the +X wall,
        // charging -X: it shoves the victim away from the wall, not into it.
        let cars = [
            blue_facing(Vec2::new(960.0, 0.0), Vec2::new(900.0, 0.0)),
            red(Vec2::new(900.0, 0.0)),
        ];
        let damage = wall_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn wall_crush_charges_a_pinned_victim_once_regardless_of_attackers() {
        // Two blue strikers both charge the red car pinned against the +X wall;
        // the victim eats the crush once, mirroring the per-victim broadside model.
        let cars = [
            red(Vec2::new(900.0, 0.0)),
            blue_facing(Vec2::new(820.0, 40.0), Vec2::new(900.0, 0.0)),
            blue_facing(Vec2::new(820.0, -40.0), Vec2::new(900.0, 0.0)),
        ];
        let damage = wall_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, WALL_CRUSH_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn wall_crush_wears_a_pinned_blue_victim_into_the_player_pool() {
        // Symmetry: a blue car pinned against the -X wall by a charging red
        // bleeds the player pool.
        let cars = [
            blue(Vec2::new(-900.0, 0.0)),
            red_facing(Vec2::new(-810.0, 0.0), Vec2::new(-900.0, 0.0)),
        ];
        let damage = wall_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, WALL_CRUSH_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn wall_crush_needs_the_enemy_within_ram_range() {
        // The blue charges the wall-pinned red, but from beyond ram range.
        let cars = [
            blue_facing(Vec2::new(700.0, 0.0), Vec2::new(900.0, 0.0)),
            red(Vec2::new(900.0, 0.0)),
        ];
        let damage = wall_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn wall_crush_spares_a_same_team_pin() {
        // A blue teammate charging another blue against the wall deals no crush.
        let cars = [
            blue(Vec2::new(900.0, 0.0)),
            blue_facing(Vec2::new(810.0, 0.0), Vec2::new(900.0, 0.0)),
        ];
        let damage = wall_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn wall_crush_pins_a_cornered_car() {
        // A red wedged into the +X/+Y corner, charged toward +X, is pinned.
        let cars = [
            red(Vec2::new(900.0, 500.0)),
            blue_facing(Vec2::new(810.0, 500.0), Vec2::new(900.0, 500.0)),
        ];
        let damage = wall_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, WALL_CRUSH_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn corner_crush_wears_a_car_wedged_into_a_corner() {
        // A red in the +X/+Y corner; a blue charges diagonally from the open
        // quadrant, shoving it into both walls at once with nowhere left to run.
        let cars = [
            red(Vec2::new(900.0, 500.0)),
            blue_facing(Vec2::new(820.0, 420.0), Vec2::new(900.0, 500.0)),
        ];
        let damage = corner_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, CORNER_CRUSH_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn corner_crush_spares_a_single_wall_pin() {
        // The red sits in the corner region but the blue charges straight +X, so
        // it is shoved into the +X wall only and can still escape along +Y: a wall
        // crush, not a corner crush.
        let cars = [
            red(Vec2::new(900.0, 500.0)),
            blue_facing(Vec2::new(810.0, 500.0), Vec2::new(900.0, 500.0)),
        ];
        let damage = corner_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn corner_crush_ignores_a_charge_that_frees_the_second_wall() {
        // The red is in the +X/+Y corner, but the blue strikes from above it,
        // shoving it into +X yet *away* from +Y: an escape lane stays open, so no
        // corner crush lands.
        let cars = [
            red(Vec2::new(900.0, 500.0)),
            blue_facing(Vec2::new(820.0, 560.0), Vec2::new(900.0, 500.0)),
        ];
        let damage = corner_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn corner_crush_spares_a_car_trading_paint_in_open_field() {
        // The same diagonal charge at the arena centre is just a ram.
        let cars = [
            red(Vec2::ZERO),
            blue_facing(Vec2::new(-80.0, -80.0), Vec2::ZERO),
        ];
        let damage = corner_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn corner_crush_needs_a_charging_enemy_not_a_mere_corner_loiterer() {
        // The blue sits in the open quadrant of the red's corner but faces away,
        // so it is not charging and no corner crush lands.
        let cars = [
            red(Vec2::new(900.0, 500.0)),
            blue_facing(Vec2::new(820.0, 420.0), Vec2::new(740.0, 340.0)),
        ];
        let damage = corner_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn corner_crush_needs_the_enemy_within_ram_range() {
        // The blue charges into the corner but from beyond ram range.
        let cars = [
            red(Vec2::new(900.0, 500.0)),
            blue_facing(Vec2::new(760.0, 360.0), Vec2::new(900.0, 500.0)),
        ];
        let damage = corner_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn corner_crush_charges_a_cornered_victim_once_regardless_of_attackers() {
        // Two blue strikers both wedge the red into the +X/+Y corner; the victim
        // eats the crush once, mirroring the per-victim wall-crush model.
        let cars = [
            red(Vec2::new(900.0, 500.0)),
            blue_facing(Vec2::new(820.0, 420.0), Vec2::new(900.0, 500.0)),
            blue_facing(Vec2::new(830.0, 430.0), Vec2::new(900.0, 500.0)),
        ];
        let damage = corner_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, CORNER_CRUSH_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn corner_crush_wears_a_pinned_blue_victim_into_the_player_pool() {
        // Symmetry: a blue car wedged into the -X/-Y corner by a charging red
        // bleeds the player pool.
        let cars = [
            blue(Vec2::new(-900.0, -500.0)),
            red_facing(Vec2::new(-820.0, -420.0), Vec2::new(-900.0, -500.0)),
        ];
        let damage = corner_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, CORNER_CRUSH_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn corner_crush_spares_a_same_team_pin() {
        // A blue teammate wedging another blue into the corner deals no crush.
        let cars = [
            blue(Vec2::new(900.0, 500.0)),
            blue_facing(Vec2::new(820.0, 420.0), Vec2::new(900.0, 500.0)),
        ];
        let damage = corner_crush_ram_damage(&cars, arena_half());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }
}
