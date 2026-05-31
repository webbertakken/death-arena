use crate::gameplay::ctf::{CtfFlag, FlagTeam};
use crate::gameplay::main::{BOUNDS, TIME_STEP};
use crate::gameplay::pickup::{NitroBoosts, Pickup};
use crate::gameplay::player::Player;
use crate::gameplay::virtual_player::ai::{
    choose_capture_the_flag_target, choose_driving_target, compute_steering, next_waypoint, AiTeam,
    DrivingChoices, DrivingTarget, FlagTarget, PickupTarget,
};
use crate::gameplay::virtual_player::VirtualPlayer;
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;

/// Distance (world units) at which a virtual player considers a waypoint
/// reached and advances to the next one.
const WAYPOINT_ARRIVE_RADIUS: f32 = 80.0;
const PICKUP_PURSUIT_RADIUS: f32 = 450.0;
const PLAYER_PURSUIT_RADIUS: f32 = 500.0;

/// Drives every [`VirtualPlayer`] towards its current patrol waypoint, applying
/// the same movement/rotation model the human player uses.
pub fn virtual_player_drive_system(
    mut query: Query<(Entity, &mut VirtualPlayer, &mut Transform)>,
    player_query: Query<&Transform, (With<Player>, Without<VirtualPlayer>)>,
    pickup_query: Query<(&Transform, &Pickup), Without<VirtualPlayer>>,
    flag_query: Query<(&Transform, &CtfFlag), Without<VirtualPlayer>>,
    nitro_boosts: Option<Res<NitroBoosts>>,
) {
    let player_position = player_query
        .get_single()
        .ok()
        .map(|transform| transform.translation.xy());
    let mut available_pickups: Vec<PickupTarget> = pickup_query
        .iter()
        .map(|(transform, pickup)| PickupTarget {
            position: transform.translation.xy(),
            bounty: pickup.kind.bounty(),
        })
        .collect();
    let flags: Vec<FlagTarget> = flag_query
        .iter()
        .map(|(transform, flag)| FlagTarget {
            team: match flag.team {
                FlagTeam::Blue => AiTeam::Blue,
                FlagTeam::Red => AiTeam::Red,
            },
            home: flag.home,
            position: transform.translation.xy(),
            holder: flag.holder,
        })
        .collect();
    let mut claimed_ctf_targets: Vec<DrivingTarget> = Vec::new();

    for (entity, mut ai, mut transform) in &mut query {
        let position = transform.translation.xy();
        let forward = (transform.rotation * Vec3::Y).xy();
        let ctf_target =
            choose_capture_the_flag_target(entity, ai.team, &flags).and_then(|target| {
                coordinate_ctf_target(target, ai.team, &flags, &claimed_ctf_targets)
            });
        if let Some(target) = ctf_target {
            if should_coordinate_ctf_target(target) {
                claimed_ctf_targets.push(target);
            }
        }
        let Some(target) = choose_driving_target(
            position,
            DrivingChoices {
                waypoints: &ai.waypoints,
                current_waypoint: ai.current_waypoint,
                ctf_target,
                pickups: &available_pickups,
                pickup_pursuit_radius: PICKUP_PURSUIT_RADIUS,
                player_position: player_position_for_team(ai.team, player_position),
                player_pursuit_radius: PLAYER_PURSUIT_RADIUS,
            },
        ) else {
            continue;
        };

        if let DrivingTarget::Pickup(target_position) = target {
            if let Some(index) = available_pickups
                .iter()
                .position(|pickup| pickup.position == target_position)
            {
                available_pickups.swap_remove(index);
            }
        }

        let intent = compute_steering(position, forward, target.position(), WAYPOINT_ARRIVE_RADIUS);

        if intent == crate::gameplay::virtual_player::ai::SteeringIntent::IDLE {
            if matches!(target, DrivingTarget::PatrolWaypoint(_)) {
                ai.current_waypoint = next_waypoint(ai.current_waypoint, ai.waypoints.len());
            }
            continue;
        }

        // Rotation: positive steer turns left (counter-clockwise).
        transform.rotate_z(intent.steer * ai.rotation_speed * TIME_STEP);

        // Translation along the (rotated) forward vector.
        let movement_direction = transform.rotation * Vec3::Y;
        let nitro_multiplier = nitro_boosts
            .as_ref()
            .map_or(1.0, |boosts| nitro_multiplier_for_team(boosts, ai.team));
        let movement_distance = intent.throttle * ai.movement_speed * nitro_multiplier * TIME_STEP;
        transform.translation += movement_direction * movement_distance;

        // Keep opponents inside the arena, just like the player.
        let extents = Vec3::new(BOUNDS.x / 2.0, BOUNDS.y / 2.0, 0.0);
        transform.translation.x = transform.translation.x.clamp(-extents.x, extents.x);
        transform.translation.y = transform.translation.y.clamp(-extents.y, extents.y);
        transform.translation.z = 4.0;
    }
}

const fn nitro_multiplier_for_team(boosts: &NitroBoosts, team: AiTeam) -> f32 {
    match team {
        AiTeam::Blue => boosts.player_multiplier(),
        AiTeam::Red => boosts.opponent_multiplier(),
    }
}

const fn should_coordinate_ctf_target(target: DrivingTarget) -> bool {
    matches!(
        target,
        DrivingTarget::EnemyFlag(_) | DrivingTarget::EscortFlagCarrier(_)
    )
}

const fn player_position_for_team(team: AiTeam, player_position: Option<Vec2>) -> Option<Vec2> {
    match team {
        AiTeam::Blue => None,
        AiTeam::Red => player_position,
    }
}

fn coordinate_ctf_target(
    target: DrivingTarget,
    team: AiTeam,
    flags: &[FlagTarget],
    claimed_targets: &[DrivingTarget],
) -> Option<DrivingTarget> {
    if !should_coordinate_ctf_target(target) || !claimed_targets.contains(&target) {
        return Some(target);
    }

    flags
        .iter()
        .find(|flag| flag.team == team)
        .map(|flag| DrivingTarget::DefendHomeBase(flag.home))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::ctf::{CtfFlag, FlagTeam};
    use crate::gameplay::virtual_player::VirtualPlayer;

    fn app_with_system() -> App {
        let mut app = App::new();
        app.add_system(virtual_player_drive_system);
        app
    }

    fn spawn_player(app: &mut App, position: Vec3) -> Entity {
        app.world
            .spawn((
                Player {
                    movement_speed: 0.0,
                    rotation_speed: 0.0,
                    engine_max_speed_multiplier: 0.0,
                    forward_max_speed_base: 0.0,
                    backward_max_speed_base: 0.0,
                    wheels_turning_multiplier: 0.0,
                },
                Transform::from_translation(position),
            ))
            .id()
    }

    fn spawn_ai(app: &mut App, waypoints: Vec<Vec2>) -> Entity {
        spawn_ai_on_team(app, AiTeam::Red, waypoints)
    }

    fn spawn_ai_on_team(app: &mut App, team: AiTeam, waypoints: Vec<Vec2>) -> Entity {
        app.world
            .spawn((
                VirtualPlayer {
                    team,
                    movement_speed: 500.0,
                    rotation_speed: f32::to_radians(360.0),
                    waypoints,
                    current_waypoint: 0,
                },
                Transform::from_translation(Vec3::new(0.0, 0.0, 4.0)),
            ))
            .id()
    }

    fn spawn_ai_at(app: &mut App, waypoints: Vec<Vec2>, translation: Vec3) -> Entity {
        app.world
            .spawn((
                VirtualPlayer {
                    team: AiTeam::Red,
                    movement_speed: 500.0,
                    rotation_speed: f32::to_radians(360.0),
                    waypoints,
                    current_waypoint: 0,
                },
                Transform::from_translation(translation),
            ))
            .id()
    }

    fn spawn_flag(
        app: &mut App,
        team: FlagTeam,
        home: Vec2,
        position: Vec3,
        holder: Option<Entity>,
    ) {
        app.world.spawn((
            CtfFlag { team, home, holder },
            Transform::from_translation(position),
        ));
    }

    fn one_frame_ai_y(team: AiTeam, nitro: Option<fn(&mut NitroBoosts)>) -> f32 {
        let mut app = app_with_system();
        if let Some(trigger) = nitro {
            app.init_resource::<NitroBoosts>();
            trigger(&mut app.world.resource_mut::<NitroBoosts>());
        }
        let ai = spawn_ai_on_team(&mut app, team, vec![Vec2::new(0.0, 1000.0)]);

        app.update();

        app.world.get::<Transform>(ai).unwrap().translation.y
    }

    #[test]
    fn moves_towards_a_distant_waypoint() {
        let mut app = app_with_system();
        // Facing +Y by default, waypoint straight ahead.
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.y > 0.0,
            "expected forward movement, y={}",
            transform.translation.y
        );
    }

    #[test]
    fn advances_waypoint_once_arrived() {
        let mut app = app_with_system();
        // Start already on top of the first waypoint so it should advance.
        let ai = spawn_ai(&mut app, vec![Vec2::ZERO, Vec2::new(500.0, 0.0)]);

        app.update();

        let vp = app.world.get::<VirtualPlayer>(ai).unwrap();
        assert_eq!(vp.current_waypoint, 1);
    }

    #[test]
    fn stays_within_arena_bounds() {
        let mut app = app_with_system();
        let edge = Vec3::new(BOUNDS.x / 2.0, BOUNDS.y / 2.0, 4.0);
        let ai = app
            .world
            .spawn((
                VirtualPlayer {
                    team: AiTeam::Red,
                    movement_speed: 5000.0,
                    rotation_speed: f32::to_radians(360.0),
                    waypoints: vec![Vec2::new(BOUNDS.x, BOUNDS.y)],
                    current_waypoint: 0,
                },
                Transform::from_translation(edge),
            ))
            .id();

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(transform.translation.x <= BOUNDS.x / 2.0 + 1e-3);
        assert!(transform.translation.y <= BOUNDS.y / 2.0 + 1e-3);
    }

    #[test]
    fn idle_ai_without_waypoints_does_not_panic() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![]);

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert_eq!(transform.translation, Vec3::new(0.0, 0.0, 4.0));
    }

    #[test]
    fn pursues_nearby_pickup_before_patrol_waypoint() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
        ));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected opponent to turn towards pickup, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn pursues_nearby_player_before_patrol_waypoint() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_player(&mut app, Vec3::new(200.0, 0.0, 5.0));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected opponent to turn towards player, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn blue_virtual_player_does_not_chase_human_teammate() {
        let mut app = app_with_system();
        let ai = spawn_ai_on_team(&mut app, AiTeam::Blue, vec![Vec2::new(0.0, 1000.0)]);
        spawn_player(&mut app, Vec3::new(200.0, 0.0, 5.0));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x.abs() < 1e-4,
            "expected blue teammate to stay on patrol, x={}",
            transform.translation.x
        );
        assert!(
            transform.translation.y > 0.0,
            "expected blue teammate to keep moving, y={}",
            transform.translation.y
        );
    }

    #[test]
    fn pickup_stays_higher_priority_than_player_chase() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_player(&mut app, Vec3::new(200.0, 0.0, 5.0));
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Repair,
            },
            Transform::from_translation(Vec3::new(-200.0, 0.0, 2.0)),
        ));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x < 0.0,
            "expected opponent to prioritise pickup, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn pursues_richer_pickup_before_closer_pickup() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Repair,
            },
            Transform::from_translation(Vec3::new(-25.0, 0.0, 2.0)),
        ));
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(150.0, 0.0, 2.0)),
        ));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected opponent to turn towards richer pickup, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn only_one_virtual_player_pursues_a_shared_pickup() {
        let mut app = app_with_system();
        let first_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let second_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
        ));

        app.update();

        let first_transform = app.world.get::<Transform>(first_ai).unwrap();
        let second_transform = app.world.get::<Transform>(second_ai).unwrap();

        assert!(
            first_transform.translation.x > 0.0,
            "expected first opponent to claim pickup, x={}",
            first_transform.translation.x
        );
        assert!(
            second_transform.translation.x.abs() < 1e-4,
            "expected second opponent to keep patrol line, x={}",
            second_transform.translation.x
        );
        assert!(
            second_transform.translation.y > 0.0,
            "expected second opponent to keep moving, y={}",
            second_transform.translation.y
        );
    }

    #[test]
    fn pursues_blue_flag_before_pickup_or_patrol_waypoint() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-200.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
        ));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x < 0.0,
            "expected opponent to turn towards blue flag, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn blue_virtual_player_pursues_red_flag() {
        let mut app = app_with_system();
        let ai = spawn_ai_on_team(&mut app, AiTeam::Blue, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-500.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(200.0, 0.0, 2.0),
            None,
        );

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected blue opponent to turn towards red flag, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn attacker_detours_for_closer_pickup_along_blue_flag_push() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-400.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(-100.0, 0.0, 2.0)),
        ));

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x < 0.0,
            "expected attacker to stay on the flag-side pickup lane, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn flag_carrier_returns_to_red_base() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
            Some(ai),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected flag carrier to turn towards red base, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn flag_carrier_hunts_stolen_home_flag_before_red_base() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let player = spawn_player(&mut app, Vec3::new(-800.0, 0.0, 5.0));
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
            Some(ai),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(-800.0, 0.0, 2.0),
            Some(player),
        );

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x < 0.0,
            "expected flag carrier to defend stolen home flag, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn teammate_escorts_flag_carrier_before_pickup_or_patrol_waypoint() {
        let mut app = app_with_system();
        let carrier = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let escort = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-200.0, 0.0, 2.0),
            Some(carrier),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
        ));

        app.update();

        let transform = app.world.get::<Transform>(escort).unwrap();
        assert!(
            transform.translation.x < 0.0,
            "expected escort to turn towards flag carrier, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn only_one_teammate_escorts_flag_carrier() {
        let mut app = app_with_system();
        let carrier = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let first_escort = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let second_escort = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-200.0, 0.0, 2.0),
            Some(carrier),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );

        app.update();

        let first_transform = app.world.get::<Transform>(first_escort).unwrap();
        let second_transform = app.world.get::<Transform>(second_escort).unwrap();

        assert!(
            first_transform.translation.x < 0.0,
            "expected first teammate to escort the carrier, x={}",
            first_transform.translation.x
        );
        assert!(
            second_transform.translation.x > 0.0,
            "expected spare teammate to defend the red base, x={}",
            second_transform.translation.x
        );
    }

    #[test]
    fn defender_chases_stolen_red_flag_before_enemy_flag() {
        let mut app = app_with_system();
        let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let player = spawn_player(&mut app, Vec3::new(200.0, 0.0, 5.0));
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-200.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(200.0, 0.0, 2.0),
            Some(player),
        );

        app.update();

        let transform = app.world.get::<Transform>(ai).unwrap();
        assert!(
            transform.translation.x > 0.0,
            "expected defender to turn towards stolen red flag, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn all_virtual_players_hunt_stolen_red_flag() {
        let mut app = app_with_system();
        let first_ai = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(0.0, -50.0, 4.0),
        );
        let second_ai = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(0.0, 50.0, 4.0),
        );
        let player = spawn_player(&mut app, Vec3::new(800.0, 0.0, 5.0));
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-500.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(800.0, 0.0, 2.0),
            Some(player),
        );

        app.update();

        let first_transform = app.world.get::<Transform>(first_ai).unwrap();
        let second_transform = app.world.get::<Transform>(second_ai).unwrap();
        assert!(
            first_transform.translation.x > 0.0,
            "first opponent should hunt the flag carrier, x={}",
            first_transform.translation.x
        );
        assert!(
            second_transform.translation.x > 0.0,
            "second opponent should also hunt the flag carrier, x={}",
            second_transform.translation.x
        );
    }

    #[test]
    fn only_one_virtual_player_pursues_a_shared_enemy_flag() {
        let mut app = app_with_system();
        let first_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let second_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-200.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
        ));

        app.update();

        let first_transform = app.world.get::<Transform>(first_ai).unwrap();
        let second_transform = app.world.get::<Transform>(second_ai).unwrap();

        assert!(
            first_transform.translation.x < 0.0,
            "expected first opponent to claim the blue flag, x={}",
            first_transform.translation.x
        );
        assert!(
            second_transform.translation.x > 0.0,
            "expected second opponent to race for another objective, x={}",
            second_transform.translation.x
        );
    }

    #[test]
    fn spare_attacker_defends_home_base_when_enemy_flag_is_claimed() {
        let mut app = app_with_system();
        let first_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let second_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-200.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );

        app.update();

        let first_transform = app.world.get::<Transform>(first_ai).unwrap();
        let second_transform = app.world.get::<Transform>(second_ai).unwrap();

        assert!(
            first_transform.translation.x < 0.0,
            "expected first opponent to claim the blue flag, x={}",
            first_transform.translation.x
        );
        assert!(
            second_transform.translation.x > 0.0,
            "expected spare opponent to defend the red base, x={}",
            second_transform.translation.x
        );
    }

    #[test]
    fn pickup_detour_still_reserves_enemy_flag_attack_role() {
        let mut app = app_with_system();
        let first_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        let second_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(-500.0, 0.0),
            Vec3::new(-400.0, 0.0, 2.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(500.0, 0.0),
            Vec3::new(500.0, 0.0, 2.0),
            None,
        );
        app.world.spawn((
            Pickup {
                kind: crate::gameplay::pickup::PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(-100.0, 0.0, 2.0)),
        ));

        app.update();

        let first_transform = app.world.get::<Transform>(first_ai).unwrap();
        let second_transform = app.world.get::<Transform>(second_ai).unwrap();

        assert!(
            first_transform.translation.x < 0.0,
            "expected attacker to detour towards pickup on the flag lane, x={}",
            first_transform.translation.x
        );
        assert!(
            second_transform.translation.x > 0.0,
            "expected spare opponent to defend once attack lane is reserved, x={}",
            second_transform.translation.x
        );
    }

    #[test]
    fn nitro_boost_increases_virtual_player_distance() {
        let normal_y = one_frame_ai_y(AiTeam::Red, None);
        let boosted_y = one_frame_ai_y(AiTeam::Red, Some(NitroBoosts::trigger_opponent));

        assert!(
            boosted_y > normal_y,
            "normal={normal_y}, boosted={boosted_y}"
        );
    }

    #[test]
    fn player_team_nitro_boosts_blue_virtual_players() {
        let normal_y = one_frame_ai_y(AiTeam::Blue, None);
        let boosted_y = one_frame_ai_y(AiTeam::Blue, Some(NitroBoosts::trigger_player));

        assert!(
            boosted_y > normal_y,
            "normal={normal_y}, boosted={boosted_y}"
        );
    }

    #[test]
    fn opponent_nitro_does_not_boost_blue_virtual_players() {
        let normal_y = one_frame_ai_y(AiTeam::Blue, None);
        let opponent_boosted_y = one_frame_ai_y(AiTeam::Blue, Some(NitroBoosts::trigger_opponent));

        assert!(
            (opponent_boosted_y - normal_y).abs() < 1e-4,
            "normal={normal_y}, opponent_boosted={opponent_boosted_y}"
        );
    }
}
