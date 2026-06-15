use crate::gameplay::main::BOUNDS;
use crate::gameplay::virtual_player::ai::{AiTeam, MIN_THROTTLE};
use crate::gameplay::virtual_player::VirtualPlayer;
use bevy::prelude::*;

/// Returns a rectangular patrol route that hugs the inside of the arena, giving
/// opponents the classic Death Rally "lapping the track" feel.
pub fn arena_patrol_route() -> Vec<Vec2> {
    let x = BOUNDS.x / 2.0 - 250.0;
    let y = BOUNDS.y / 2.0 - 250.0;
    vec![
        Vec2::new(x, y),
        Vec2::new(-x, y),
        Vec2::new(-x, -y),
        Vec2::new(x, -y),
    ]
}

/// A driving personality: a coherent speed/cornering trade-off that gives each
/// opponent a distinct feel, the classic Death Rally roster of rival drivers.
///
/// Every archetype is a genuine trade-off rather than a strict upgrade, and the
/// roster mirrors the same set of personalities across both teams (see
/// [`spawn_roster`]) so variety never tips the balance one side's way.
#[derive(Debug, Clone, Copy, PartialEq)]
struct DriverProfile {
    /// Linear speed in metres per second.
    movement_speed: f32,
    /// Rotation speed in degrees per second (converted to radians at spawn).
    turn_degrees: f32,
    /// World-space radius within which the driver breaks off patrol to hunt the
    /// human player. A behavioural trait, not a power stat: a wider radius hunts
    /// more eagerly but abandons the patrol lap and the flag objective sooner, so
    /// it trades discipline for aggression rather than being a strict upgrade.
    player_pursuit_radius: f32,
    /// World-space radius within which the driver breaks off to scavenge a
    /// trackside pickup. The personality's greed axis: a wider radius detours for
    /// loot more eagerly but abandons the patrol lap and the flag objective sooner,
    /// so it trades discipline for greed rather than being a strict upgrade.
    pickup_pursuit_radius: f32,
    /// Throttle floor the driver keeps through a corner: how hard it stays on the
    /// gas when the target is off to the side. The personality's commitment axis: a
    /// higher floor barrels through on a wider line that covers ground faster but
    /// overshoots the apex, a lower one eases off for a tighter, more precise line,
    /// so it trades line precision for corner speed rather than being a strict
    /// upgrade.
    corner_throttle: f32,
}

impl DriverProfile {
    /// Balanced baseline driver, preserving the original uniform 420 m/s,
    /// 300 deg/s, 500-unit-pursuit, 450-unit-greed feel. The neutral all-rounder
    /// every other archetype is measured against, and the filler opposite the human
    /// player so the AI rosters stay perfectly mirrored.
    const ALL_ROUNDER: Self = Self {
        movement_speed: 420.0,
        turn_degrees: 300.0,
        player_pursuit_radius: 500.0,
        pickup_pursuit_radius: 450.0,
        corner_throttle: MIN_THROTTLE,
    };
    /// Straight-line specialist: quicker down the open lane, lazier through a
    /// corner, a hot-headed hunter that runs the player down from further out, and
    /// the greediest scavenger, breaking off for loot from the widest range.
    const SPRINTER: Self = Self {
        movement_speed: 450.0,
        turn_degrees: 280.0,
        player_pursuit_radius: 580.0,
        pickup_pursuit_radius: 520.0,
        corner_throttle: 0.42,
    };
    /// Cornering specialist: sharper turn-in, a touch slower flat out, the most
    /// disciplined of the three, staying on its line until the player is genuinely
    /// close, and the least greedy, leaving distant pickups for the objective.
    const TECHNICIAN: Self = Self {
        movement_speed: 390.0,
        turn_degrees: 320.0,
        player_pursuit_radius: 420.0,
        pickup_pursuit_radius: 380.0,
        corner_throttle: 0.20,
    };
    /// Single-minded ambusher: it lives to run a rival down, not to race or loot.
    /// The corner of the trade-off space the original trio never covered, splitting
    /// apart the behavioural axes the others kept locked together. The sprinter
    /// hunts keenly *and* scavenges greedily; the technician is shy of both; the
    /// ambusher decouples them, hunting from further out than the all-rounder yet
    /// leaving the cash bags on the track for the objective. It commits to the chase
    /// line with a reckless corner throttle, and buys all that aggression on the
    /// mobility frontier: a touch slower flat out than the baseline, paid back with
    /// a sharper turn-in, so it is a genuine trade-off and never a strict upgrade.
    const AMBUSHER: Self = Self {
        movement_speed: 405.0,
        turn_degrees: 310.0,
        player_pursuit_radius: 560.0,
        pickup_pursuit_radius: 400.0,
        corner_throttle: 0.38,
    };
}

/// Each archetype is a genuine trade-off, never a strict upgrade: more top speed
/// is bought with lazier cornering, enforced at compile time so the roster can
/// never drift into a profile that is simply better on both axes.
const _: () = assert!(
    DriverProfile::SPRINTER.movement_speed > DriverProfile::ALL_ROUNDER.movement_speed
        && DriverProfile::ALL_ROUNDER.movement_speed > DriverProfile::TECHNICIAN.movement_speed
);
const _: () = assert!(
    DriverProfile::SPRINTER.turn_degrees < DriverProfile::ALL_ROUNDER.turn_degrees
        && DriverProfile::ALL_ROUNDER.turn_degrees < DriverProfile::TECHNICIAN.turn_degrees
);
/// Personalities stay in a tight band around the 420 m/s, 300 deg/s baseline so
/// they are felt without breaking the tuned chase and escort balance: even the
/// slowest chaser still outpaces the fastest flag carrier, which is slowed to a
/// fraction of its own top speed. Enforced at compile time.
const _: () = assert!(
    DriverProfile::SPRINTER.movement_speed <= 480.0
        && DriverProfile::TECHNICIAN.movement_speed >= 360.0
        && DriverProfile::SPRINTER.turn_degrees >= 260.0
        && DriverProfile::TECHNICIAN.turn_degrees <= 340.0
);
/// Hunting eagerness is the personality's third trade-off axis and lines up with
/// the speed one: the straight-line sprinter that struggles in corners is also
/// the keenest to peel off and run the player down, while the disciplined
/// technician hunts most conservatively and the all-rounder sits between. More
/// pursuit is bought with less objective discipline, never a strict upgrade,
/// enforced at compile time so the roster can never drift into an archetype that
/// hunts wider *and* corners better.
const _: () = assert!(
    DriverProfile::SPRINTER.player_pursuit_radius
        > DriverProfile::ALL_ROUNDER.player_pursuit_radius
        && DriverProfile::ALL_ROUNDER.player_pursuit_radius
            > DriverProfile::TECHNICIAN.player_pursuit_radius
);
/// Pursuit stays in a tight band around the 500-unit baseline so personality is
/// felt without breaking the tuned chase balance: even the keenest sprinter never
/// hunts from absurd range, and even the most disciplined technician never
/// ignores a player driving right past it. Enforced at compile time.
const _: () = assert!(
    DriverProfile::SPRINTER.player_pursuit_radius <= 640.0
        && DriverProfile::TECHNICIAN.player_pursuit_radius >= 360.0
);
/// Pickup greed is the personality's fourth trade-off axis and lines up with the
/// others: the impulsive sprinter that struggles in corners is also the keenest
/// to peel off and scavenge a bag, while the disciplined technician scavenges most
/// conservatively and the all-rounder sits between. More greed is bought with less
/// objective discipline, never a strict upgrade, enforced at compile time so the
/// roster can never drift into an archetype that scavenges wider *and* corners
/// better.
const _: () = assert!(
    DriverProfile::SPRINTER.pickup_pursuit_radius
        > DriverProfile::ALL_ROUNDER.pickup_pursuit_radius
        && DriverProfile::ALL_ROUNDER.pickup_pursuit_radius
            > DriverProfile::TECHNICIAN.pickup_pursuit_radius
);
/// Greed stays in a tight band around the 450-unit baseline (the former uniform
/// global) so personality is felt without breaking the tuned pickup-contest
/// balance: even the greediest sprinter never scavenges from absurd range, and
/// even the most disciplined technician never ignores a bag it drives right past.
/// Enforced at compile time.
const _: () = assert!(
    DriverProfile::SPRINTER.pickup_pursuit_radius <= 580.0
        && DriverProfile::TECHNICIAN.pickup_pursuit_radius >= 340.0
);
/// Cornering commitment is the personality's fifth trade-off axis and lines up
/// with the others: the impulsive sprinter that struggles in corners is also the
/// one that barrels through them on a wide line, the disciplined technician eases
/// off for a tight, precise line, and the all-rounder sits between on the neutral
/// [`MIN_THROTTLE`] baseline. More corner speed is bought with a wider line, never
/// a strict upgrade, enforced at compile time so the roster can never drift into
/// an archetype that corners faster *and* on a tighter line.
const _: () = assert!(
    DriverProfile::SPRINTER.corner_throttle > DriverProfile::ALL_ROUNDER.corner_throttle
        && DriverProfile::ALL_ROUNDER.corner_throttle > DriverProfile::TECHNICIAN.corner_throttle
);
/// Commitment stays a sane throttle fraction around the [`MIN_THROTTLE`] baseline
/// so personality is felt without breaking the tuned chase: even the most reckless
/// sprinter still eases off enough to come round a corner, and even the most
/// disciplined technician keeps rolling so it never stalls mid-turn. Enforced at
/// compile time.
const _: () = assert!(
    DriverProfile::SPRINTER.corner_throttle <= 0.5
        && DriverProfile::TECHNICIAN.corner_throttle >= 0.15
);
/// The ambusher's signature is a genuine decoupling, enforced at compile time: it
/// hunts keener than the baseline yet scavenges more reluctantly, the trade the
/// original trio (keen-on-both sprinter, shy-of-both technician, neutral
/// all-rounder) never struck. This is what makes the fourth archetype a new corner
/// of the space rather than a numeric remix of the existing three.
const _: () = assert!(
    DriverProfile::AMBUSHER.player_pursuit_radius
        > DriverProfile::ALL_ROUNDER.player_pursuit_radius
        && DriverProfile::AMBUSHER.pickup_pursuit_radius
            < DriverProfile::ALL_ROUNDER.pickup_pursuit_radius
);
/// The ambusher's aggression is bought on the same speed/turn frontier as the
/// trio, never for free, enforced at compile time: slower flat out than the
/// all-rounder but sharper through a corner, so the archetype is a genuine
/// trade-off and not a strict mobility upgrade.
const _: () = assert!(
    DriverProfile::AMBUSHER.movement_speed < DriverProfile::ALL_ROUNDER.movement_speed
        && DriverProfile::AMBUSHER.turn_degrees > DriverProfile::ALL_ROUNDER.turn_degrees
);
/// The ambusher sharpens the roster without flattening the trio's extremes,
/// enforced at compile time: the sprinter stays the keenest hunter and most
/// reckless through a corner, and the technician the least greedy scavenger, so
/// the interior archetype never steals another's identity.
const _: () = assert!(
    DriverProfile::AMBUSHER.player_pursuit_radius < DriverProfile::SPRINTER.player_pursuit_radius
        && DriverProfile::AMBUSHER.corner_throttle < DriverProfile::SPRINTER.corner_throttle
        && DriverProfile::AMBUSHER.pickup_pursuit_radius
            > DriverProfile::TECHNICIAN.pickup_pursuit_radius
);
/// The ambusher stays inside the same tight bands as the trio so its personality
/// is felt without breaking the tuned chase and escort balance, enforced at
/// compile time: its speed and turn sit in the same 360..=480 / 260..=340 mobility
/// band, and its hunting, greed and corner commitment in the same ranges the trio
/// is held to, so even this keen hunter never chases from absurd range.
const _: () = assert!(
    DriverProfile::AMBUSHER.movement_speed <= 480.0
        && DriverProfile::AMBUSHER.movement_speed >= 360.0
        && DriverProfile::AMBUSHER.turn_degrees >= 260.0
        && DriverProfile::AMBUSHER.turn_degrees <= 340.0
        && DriverProfile::AMBUSHER.player_pursuit_radius <= 640.0
        && DriverProfile::AMBUSHER.pickup_pursuit_radius <= 580.0
        && DriverProfile::AMBUSHER.pickup_pursuit_radius >= 340.0
        && DriverProfile::AMBUSHER.corner_throttle <= 0.5
        && DriverProfile::AMBUSHER.corner_throttle >= 0.15
);

#[derive(Debug, Clone, Copy, PartialEq)]
struct VirtualPlayerSpawn {
    name: &'static str,
    team: AiTeam,
    start_waypoint: usize,
    translation: Vec3,
    profile: DriverProfile,
}

const fn spawn_roster() -> [VirtualPlayerSpawn; 7] {
    [
        VirtualPlayerSpawn {
            name: "Teammate 1",
            team: AiTeam::Blue,
            start_waypoint: 3,
            translation: Vec3::new(-430.0, 200.0, 4.0),
            profile: DriverProfile::SPRINTER,
        },
        VirtualPlayerSpawn {
            name: "Teammate 2",
            team: AiTeam::Blue,
            start_waypoint: 2,
            translation: Vec3::new(0.0, -380.0, 4.0),
            profile: DriverProfile::TECHNICIAN,
        },
        VirtualPlayerSpawn {
            name: "Teammate 3",
            team: AiTeam::Blue,
            start_waypoint: 0,
            translation: Vec3::new(-430.0, -200.0, 4.0),
            profile: DriverProfile::AMBUSHER,
        },
        VirtualPlayerSpawn {
            name: "Opponent 1",
            team: AiTeam::Red,
            start_waypoint: 0,
            translation: Vec3::new(430.0, 200.0, 4.0),
            profile: DriverProfile::SPRINTER,
        },
        VirtualPlayerSpawn {
            name: "Opponent 2",
            team: AiTeam::Red,
            start_waypoint: 1,
            translation: Vec3::new(0.0, 380.0, 4.0),
            profile: DriverProfile::TECHNICIAN,
        },
        VirtualPlayerSpawn {
            name: "Opponent 3",
            team: AiTeam::Red,
            start_waypoint: 2,
            translation: Vec3::new(430.0, -200.0, 4.0),
            profile: DriverProfile::AMBUSHER,
        },
        // Opponent 4 fills the slot opposite the human player with the neutral
        // baseline, keeping both teams on the identical multiset of profiles.
        VirtualPlayerSpawn {
            name: "Opponent 4",
            team: AiTeam::Red,
            start_waypoint: 3,
            translation: Vec3::new(160.0, 380.0, 4.0),
            profile: DriverProfile::ALL_ROUNDER,
        },
    ]
}

fn initial_car_rotation(spawn: Vec3, target: Vec2) -> Quat {
    let to_target = target - spawn.truncate();
    let Some(direction) = to_target.try_normalize() else {
        return Quat::IDENTITY;
    };
    let angle = Vec2::Y.perp_dot(direction).atan2(Vec2::Y.dot(direction));
    Quat::from_rotation_z(angle)
}

/// Spawns a small grid of virtual CTF drivers that patrol the arena, each with
/// its own [`DriverProfile`] so the roster of rivals feels distinct.
pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let chassis = asset_server.load("textures/car1/chassis1.png");
    let route = arena_patrol_route();

    for spawn in spawn_roster() {
        let target = route[spawn.start_waypoint];
        commands.spawn((
            Name::new(spawn.name),
            VirtualPlayer {
                team: spawn.team,
                movement_speed: spawn.profile.movement_speed,
                rotation_speed: f32::to_radians(spawn.profile.turn_degrees),
                waypoints: route.clone(),
                current_waypoint: spawn.start_waypoint,
                player_pursuit_radius: spawn.profile.player_pursuit_radius,
                pickup_pursuit_radius: spawn.profile.pickup_pursuit_radius,
                corner_throttle: spawn.profile.corner_throttle,
            },
            SpriteBundle {
                texture: chassis.clone(),
                transform: Transform {
                    translation: spawn.translation,
                    rotation: initial_car_rotation(spawn.translation, target),
                    scale: Vec3::new(0.2, 0.2, 0.2),
                },
                ..default()
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patrol_route_has_four_corners_inside_bounds() {
        let route = arena_patrol_route();
        assert_eq!(route.len(), 4);
        let max_x = BOUNDS.x / 2.0;
        let max_y = BOUNDS.y / 2.0;
        for point in route {
            assert!(point.x.abs() < max_x, "x out of bounds: {}", point.x);
            assert!(point.y.abs() < max_y, "y out of bounds: {}", point.y);
        }
    }

    #[test]
    fn roster_balances_human_team_against_opponents() {
        let roster = spawn_roster();

        let blue_teammates = roster
            .iter()
            .filter(|spawn| spawn.team == AiTeam::Blue)
            .count();
        let red_opponents = roster
            .iter()
            .filter(|spawn| spawn.team == AiTeam::Red)
            .count();

        assert_eq!(blue_teammates + 1, red_opponents);
    }

    #[test]
    fn roster_spawns_each_virtual_player_in_a_unique_position() {
        let roster = spawn_roster();
        for (index, spawn) in roster.iter().enumerate() {
            assert!(
                roster
                    .iter()
                    .skip(index + 1)
                    .all(|other| spawn.translation != other.translation),
                "{} shares a spawn position",
                spawn.name
            );
        }
    }

    #[test]
    fn initial_rotation_faces_first_patrol_waypoint() {
        let route = arena_patrol_route();
        let spawn = spawn_roster()[0];
        let target = route[spawn.start_waypoint];

        let rotation = initial_car_rotation(spawn.translation, target);
        let forward = rotation * Vec3::Y;

        assert!(
            forward
                .truncate()
                .dot(target - spawn.translation.truncate())
                > 0.0
        );
    }

    fn occurrences(profiles: &[DriverProfile], target: DriverProfile) -> usize {
        profiles
            .iter()
            .filter(|profile| **profile == target)
            .count()
    }

    fn is_same_multiset(left: &[DriverProfile], right: &[DriverProfile]) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .all(|profile| occurrences(left, *profile) == occurrences(right, *profile))
    }

    fn ai_profiles(team: AiTeam) -> Vec<DriverProfile> {
        spawn_roster()
            .into_iter()
            .filter(|spawn| spawn.team == team)
            .map(|spawn| spawn.profile)
            .collect()
    }

    fn assert_near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= f32::EPSILON,
            "actual={actual}, expected={expected}"
        );
    }

    #[test]
    fn the_all_rounder_preserves_the_original_uniform_driver() {
        // The neutral baseline keeps the pre-personality 420 m/s, 300 deg/s,
        // 500-unit-pursuit, 450-unit-greed feel, so only the sprinter and
        // technician depart from it and the human's mirror is untouched.
        assert_near(DriverProfile::ALL_ROUNDER.movement_speed, 420.0);
        assert_near(DriverProfile::ALL_ROUNDER.turn_degrees, 300.0);
        assert_near(DriverProfile::ALL_ROUNDER.player_pursuit_radius, 500.0);
        assert_near(DriverProfile::ALL_ROUNDER.pickup_pursuit_radius, 450.0);
        // The all-rounder corners on the original uniform throttle floor, so only
        // the sprinter and technician depart from the neutral baseline.
        assert_near(DriverProfile::ALL_ROUNDER.corner_throttle, MIN_THROTTLE);
    }

    #[test]
    fn personalities_hunt_with_distinct_eagerness() {
        // Aggression is a real personality axis, not flavour text: the three
        // archetypes each peel off after the player at a genuinely different range.
        let sprinter = DriverProfile::SPRINTER.player_pursuit_radius;
        let all_rounder = DriverProfile::ALL_ROUNDER.player_pursuit_radius;
        let technician = DriverProfile::TECHNICIAN.player_pursuit_radius;
        assert!(
            sprinter > all_rounder && all_rounder > technician,
            "expected distinct hunting eagerness, got sprinter={sprinter}, \
             all_rounder={all_rounder}, technician={technician}"
        );
    }

    #[test]
    fn personalities_scavenge_with_distinct_greed() {
        // Greed is a real personality axis, not flavour text: the three archetypes
        // each break off for a trackside pickup at a genuinely different range.
        let sprinter = DriverProfile::SPRINTER.pickup_pursuit_radius;
        let all_rounder = DriverProfile::ALL_ROUNDER.pickup_pursuit_radius;
        let technician = DriverProfile::TECHNICIAN.pickup_pursuit_radius;
        assert!(
            sprinter > all_rounder && all_rounder > technician,
            "expected distinct scavenging greed, got sprinter={sprinter}, \
             all_rounder={all_rounder}, technician={technician}"
        );
    }

    #[test]
    fn personalities_corner_with_distinct_commitment() {
        // Cornering commitment is a real personality axis, not flavour text: the
        // three archetypes each keep a genuinely different throttle floor through a
        // corner, so each rival takes a turn with its own line.
        let sprinter = DriverProfile::SPRINTER.corner_throttle;
        let all_rounder = DriverProfile::ALL_ROUNDER.corner_throttle;
        let technician = DriverProfile::TECHNICIAN.corner_throttle;
        assert!(
            sprinter > all_rounder && all_rounder > technician,
            "expected distinct cornering commitment, got sprinter={sprinter}, \
             all_rounder={all_rounder}, technician={technician}"
        );
    }

    #[test]
    fn roster_fields_at_least_three_distinct_personalities() {
        let roster = spawn_roster();
        let mut distinct: Vec<DriverProfile> = Vec::new();
        for spawn in roster {
            if !distinct.contains(&spawn.profile) {
                distinct.push(spawn.profile);
            }
        }
        assert!(
            distinct.len() >= 3,
            "expected a roster of distinct driving personalities, found {}",
            distinct.len()
        );
    }

    #[test]
    fn roster_fields_a_fourth_distinct_personality() {
        // The roster of rivals grows past the original trio: a fourth genuinely
        // distinct archetype joins the field instead of padding it out with a
        // duplicate all-rounder, so the human meets four different driving styles.
        let roster = spawn_roster();
        let mut distinct: Vec<DriverProfile> = Vec::new();
        for spawn in roster {
            if !distinct.contains(&spawn.profile) {
                distinct.push(spawn.profile);
            }
        }
        assert!(
            distinct.len() >= 4,
            "expected at least four distinct driving personalities, found {}",
            distinct.len()
        );
    }

    #[test]
    fn the_ambusher_decouples_keen_hunting_from_loot_greed() {
        // The ambusher is the corner of the trade-off space the original trio never
        // covered: its hunting and greed point opposite ways around the baseline. It
        // runs a rival down from further out than the all-rounder, yet is the less
        // tempted to peel off for loot, the single-minded hunter that leaves the
        // cash bags on the track.
        let ambusher_hunt = DriverProfile::AMBUSHER.player_pursuit_radius;
        let ambusher_greed = DriverProfile::AMBUSHER.pickup_pursuit_radius;
        let baseline_hunt = DriverProfile::ALL_ROUNDER.player_pursuit_radius;
        let baseline_greed = DriverProfile::ALL_ROUNDER.pickup_pursuit_radius;
        assert!(
            ambusher_hunt > baseline_hunt,
            "the ambusher ({ambusher_hunt}) must hunt keener than the baseline ({baseline_hunt})"
        );
        assert!(
            ambusher_greed < baseline_greed,
            "the ambusher ({ambusher_greed}) must scavenge more reluctantly than the baseline \
             ({baseline_greed})"
        );
    }

    #[test]
    fn no_other_archetype_decouples_hunting_from_greed_like_the_ambusher() {
        // The decoupling is what makes the ambusher genuinely new and not a remix:
        // the sprinter is keen on both the hunt and the loot, the technician shy of
        // both, the all-rounder neutral. Only the ambusher hunts harder while
        // scavenging softer than the baseline.
        let baseline_hunt = DriverProfile::ALL_ROUNDER.player_pursuit_radius;
        let baseline_greed = DriverProfile::ALL_ROUNDER.pickup_pursuit_radius;
        for profile in [
            DriverProfile::SPRINTER,
            DriverProfile::TECHNICIAN,
            DriverProfile::ALL_ROUNDER,
        ] {
            let hunts_harder = profile.player_pursuit_radius > baseline_hunt;
            let scavenges_softer = profile.pickup_pursuit_radius < baseline_greed;
            assert!(
                !(hunts_harder && scavenges_softer),
                "only the ambusher should hunt harder yet scavenge softer than the baseline"
            );
        }
    }

    #[test]
    fn the_ambusher_buys_its_aggression_on_the_mobility_frontier() {
        // Its keen, reckless aggression is never free: a touch slower flat out than
        // the all-rounder, paid back with a sharper turn-in, so the new archetype
        // sits on the same speed/turn frontier as the trio and is a genuine
        // trade-off rather than a strict upgrade.
        let ambusher_speed = DriverProfile::AMBUSHER.movement_speed;
        let ambusher_turn = DriverProfile::AMBUSHER.turn_degrees;
        let baseline_speed = DriverProfile::ALL_ROUNDER.movement_speed;
        let baseline_turn = DriverProfile::ALL_ROUNDER.turn_degrees;
        assert!(
            ambusher_speed < baseline_speed,
            "the ambusher ({ambusher_speed}) must give up top speed for its aggression"
        );
        assert!(
            ambusher_turn > baseline_turn,
            "the ambusher ({ambusher_turn}) must earn its keep with a sharper turn-in"
        );
    }

    #[test]
    fn the_ambusher_never_dethrones_the_trios_extremes() {
        // The interior archetype sharpens the roster without flattening the others:
        // the sprinter stays the keenest hunter and most reckless through a corner,
        // and the technician the least greedy scavenger.
        let ambusher_hunt = DriverProfile::AMBUSHER.player_pursuit_radius;
        let ambusher_throttle = DriverProfile::AMBUSHER.corner_throttle;
        let ambusher_greed = DriverProfile::AMBUSHER.pickup_pursuit_radius;
        let sprinter_hunt = DriverProfile::SPRINTER.player_pursuit_radius;
        let sprinter_throttle = DriverProfile::SPRINTER.corner_throttle;
        let technician_greed = DriverProfile::TECHNICIAN.pickup_pursuit_radius;
        assert!(
            ambusher_hunt < sprinter_hunt,
            "the sprinter ({sprinter_hunt}) must remain keener than the ambusher ({ambusher_hunt})"
        );
        assert!(
            ambusher_throttle < sprinter_throttle,
            "the sprinter must remain the most reckless through a corner"
        );
        assert!(
            ambusher_greed > technician_greed,
            "the technician ({technician_greed}) must remain less greedy than the ambusher \
             ({ambusher_greed})"
        );
    }

    #[test]
    fn ai_rosters_mirror_once_the_human_fills_the_blue_baseline() {
        // Blue fields three virtual drivers plus the human; Red fields four. The
        // human occupies Blue's fourth slot as the neutral all-rounder, so both
        // teams draw from the identical multiset of personalities: a perfectly
        // mirrored roster, never a side that is systematically faster.
        let mut blue = ai_profiles(AiTeam::Blue);
        blue.push(DriverProfile::ALL_ROUNDER);
        let red = ai_profiles(AiTeam::Red);
        assert!(
            is_same_multiset(&blue, &red),
            "teams must field the same set of driving personalities"
        );
    }

    #[test]
    fn ai_rosters_balance_aggregate_speed_and_turn_with_the_human() {
        // The multiset mirror implies equal aggregates: counting the human as the
        // baseline all-rounder, neither team has more top speed or more cornering
        // to share across its cars.
        let human = DriverProfile::ALL_ROUNDER;
        let blue = ai_profiles(AiTeam::Blue);
        let red = ai_profiles(AiTeam::Red);

        let blue_speed: f32 =
            blue.iter().map(|p| p.movement_speed).sum::<f32>() + human.movement_speed;
        let red_speed: f32 = red.iter().map(|p| p.movement_speed).sum();
        assert!(
            (blue_speed - red_speed).abs() <= f32::EPSILON,
            "aggregate speed must be level: blue={blue_speed}, red={red_speed}"
        );

        let blue_turn: f32 = blue.iter().map(|p| p.turn_degrees).sum::<f32>() + human.turn_degrees;
        let red_turn: f32 = red.iter().map(|p| p.turn_degrees).sum();
        assert!(
            (blue_turn - red_turn).abs() <= f32::EPSILON,
            "aggregate cornering must be level: blue={blue_turn}, red={red_turn}"
        );
    }

    #[test]
    fn ai_rosters_balance_aggregate_player_pursuit_with_the_human() {
        // The new aggression axis is balanced on the same terms as speed and
        // cornering: counting the human as the baseline all-rounder, neither team
        // fields more total hunting eagerness than the other, so one side is never
        // systematically keener to run the enemy down.
        let human = DriverProfile::ALL_ROUNDER;
        let blue = ai_profiles(AiTeam::Blue);
        let red = ai_profiles(AiTeam::Red);

        let blue_pursuit: f32 =
            blue.iter().map(|p| p.player_pursuit_radius).sum::<f32>() + human.player_pursuit_radius;
        let red_pursuit: f32 = red.iter().map(|p| p.player_pursuit_radius).sum();
        assert!(
            (blue_pursuit - red_pursuit).abs() <= f32::EPSILON,
            "aggregate player pursuit must be level: blue={blue_pursuit}, red={red_pursuit}"
        );
    }

    #[test]
    fn ai_rosters_balance_aggregate_pickup_pursuit_with_the_human() {
        // The new greed axis is balanced on the same terms as speed, cornering and
        // player pursuit: counting the human as the baseline all-rounder, neither
        // team fields more total scavenging greed than the other, so one side is
        // never systematically keener to peel off for loot.
        let human = DriverProfile::ALL_ROUNDER;
        let blue = ai_profiles(AiTeam::Blue);
        let red = ai_profiles(AiTeam::Red);

        let blue_greed: f32 =
            blue.iter().map(|p| p.pickup_pursuit_radius).sum::<f32>() + human.pickup_pursuit_radius;
        let red_greed: f32 = red.iter().map(|p| p.pickup_pursuit_radius).sum();
        assert!(
            (blue_greed - red_greed).abs() <= f32::EPSILON,
            "aggregate pickup pursuit must be level: blue={blue_greed}, red={red_greed}"
        );
    }

    #[test]
    fn ai_rosters_balance_aggregate_corner_commitment_with_the_human() {
        // The new cornering axis is balanced on the same terms as the others:
        // counting the human as the baseline all-rounder, neither team fields more
        // total corner commitment than the other, so one side is never
        // systematically faster (or tighter) through the turns.
        let human = DriverProfile::ALL_ROUNDER;
        let blue = ai_profiles(AiTeam::Blue);
        let red = ai_profiles(AiTeam::Red);

        let blue_corner: f32 =
            blue.iter().map(|p| p.corner_throttle).sum::<f32>() + human.corner_throttle;
        let red_corner: f32 = red.iter().map(|p| p.corner_throttle).sum();
        assert!(
            (blue_corner - red_corner).abs() <= f32::EPSILON,
            "aggregate corner commitment must be level: blue={blue_corner}, red={red_corner}"
        );
    }
}
