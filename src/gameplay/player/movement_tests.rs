#[cfg(test)]
mod tests {
    use crate::gameplay::combat::{VehicleIntegrity, WreckStuns, WreckSurges};
    use crate::gameplay::ctf::{CtfFlag, CtfMatchResult, CtfMatchWinner, FlagTeam};
    use crate::gameplay::main::BOUNDS;
    use crate::gameplay::pickup::{NitroBoosts, SabotageEffects};
    use crate::gameplay::player::car::{FrontLeftWheel, FrontRightWheel};
    use crate::gameplay::player::movement::car_movement_system;
    use crate::gameplay::player::Player;
    use crate::gameplay::virtual_player::ai::AiTeam;
    use crate::gameplay::virtual_player::VirtualPlayer;
    use bevy::prelude::*;

    fn setup_test_app() -> App {
        let mut app = App::new();
        app.init_resource::<Input<KeyCode>>();
        app.add_system(car_movement_system);
        app
    }

    fn spawn_player(app: &mut App, translation: Vec3) -> Entity {
        app.world
            .spawn((
                Player {
                    movement_speed: 500.0,
                    rotation_speed: f32::to_radians(360.0),
                    engine_max_speed_multiplier: 1.0,
                    forward_max_speed_base: 1.0,
                    backward_max_speed_base: 1.0,
                    wheels_turning_multiplier: 1.0,
                },
                Transform::from_translation(translation),
            ))
            .id()
    }

    fn spawn_wheels(app: &mut App, _player_entity: Entity) -> (Entity, Entity) {
        let left_wheel_id = app.world.spawn((FrontLeftWheel, Transform::default())).id();
        let right_wheel_id = app
            .world
            .spawn((FrontRightWheel, Transform::default()))
            .id();

        // We need to make sure the wheels are not part of the same entity as the player
        // if we use the filter in car_movement_system, but the system uses
        // Query<(&FrontLeftWheel, &mut Transform), FilterFrontLeftWheel>
        // where FilterFrontLeftWheel = (Without<Player>, Without<FrontRightWheel>)
        // Wait, the query in car_movement_system is:
        // mut front_left_wheel_query: Query<(&FrontLeftWheel, &mut Transform), FilterFrontLeftWheel>
        // FilterFrontLeftWheel = (Without<Player>, Without<FrontRightWheel>)
        // This means the front_left_wheel entity must NOT have a Player component and NOT have a FrontRightWheel component.

        (left_wheel_id, right_wheel_id)
    }

    /// Spawns a virtual car the human can draft behind, facing straight up the
    /// arena (identity rotation) so its wake points along `+Y`.
    fn spawn_leading_car(app: &mut App, translation: Vec3) -> Entity {
        app.world
            .spawn((
                VirtualPlayer {
                    team: AiTeam::Red,
                    movement_speed: 500.0,
                    rotation_speed: f32::to_radians(360.0),
                    waypoints: vec![Vec2::new(0.0, 1000.0)],
                    current_waypoint: 0,
                    player_pursuit_radius: 500.0,
                    pickup_pursuit_radius: 450.0,
                    corner_throttle: 0.3,
                },
                Transform::from_translation(translation),
            ))
            .id()
    }

    #[test]
    fn test_car_movement_forward() {
        let mut app = setup_test_app();

        let player_id = spawn_player(&mut app, Vec3::new(0.0, 0.0, 5.0));
        let (left_wheel_id, _right_wheel_id) = spawn_wheels(&mut app, player_id);

        // Simulate "Up" key press
        let mut input = app.world.resource_mut::<Input<KeyCode>>();
        input.press(KeyCode::Up);

        // Run the app for one frame
        app.update();

        // Check if movement happened
        let player_transform = app.world.get::<Transform>(player_id).unwrap();

        // Initial rotation is 0, so forward is Y.
        assert!(player_transform.translation.y > 0.0);

        // Check if wheels rotated
        let fl_wheel_transform = app.world.get::<Transform>(left_wheel_id).unwrap();
        assert!(fl_wheel_transform.rotation.to_euler(EulerRot::XYZ).2 != 0.0);
    }

    #[test]
    fn finished_match_stops_player_movement() {
        let mut app = setup_test_app();
        app.insert_resource(CtfMatchResult {
            winner: Some(CtfMatchWinner::Player),
        });
        let player_id = spawn_player(&mut app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut app, player_id);
        app.world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);

        app.update();

        let player_transform = app.world.get::<Transform>(player_id).unwrap();
        assert_eq!(player_transform.translation, Vec3::new(0.0, 0.0, 5.0));
    }

    #[test]
    fn movement_system_skips_frame_without_player() {
        let mut app = setup_test_app();
        app.update();
    }

    #[test]
    fn movement_system_skips_frame_without_front_wheels() {
        let mut app = setup_test_app();
        let player_id = spawn_player(&mut app, Vec3::new(0.0, 0.0, 5.0));
        app.world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);

        app.update();

        let player_transform = app.world.get::<Transform>(player_id).unwrap();
        assert_eq!(player_transform.translation, Vec3::new(0.0, 0.0, 5.0));
    }

    #[test]
    fn nitro_boost_increases_forward_distance() {
        let mut normal_app = setup_test_app();
        let normal_player = spawn_player(&mut normal_app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut normal_app, normal_player);
        normal_app
            .world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        normal_app.update();
        let normal_y = normal_app
            .world
            .get::<Transform>(normal_player)
            .unwrap()
            .translation
            .y;

        let mut boosted_app = setup_test_app();
        boosted_app.init_resource::<NitroBoosts>();
        boosted_app
            .world
            .resource_mut::<NitroBoosts>()
            .trigger_player();
        let boosted_player = spawn_player(&mut boosted_app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut boosted_app, boosted_player);
        boosted_app
            .world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        boosted_app.update();
        let boosted_y = boosted_app
            .world
            .get::<Transform>(boosted_player)
            .unwrap()
            .translation
            .y;

        assert!(
            boosted_y > normal_y,
            "normal={normal_y}, boosted={boosted_y}"
        );
    }

    #[test]
    fn drafting_behind_a_car_ahead_increases_forward_distance() {
        // Control: no car ahead, so there is no wake to catch.
        let mut lone_app = setup_test_app();
        let lone = spawn_player(&mut lone_app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut lone_app, lone);
        lone_app
            .world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        lone_app.update();
        let lone_y = lone_app.world.get::<Transform>(lone).unwrap().translation.y;

        // Drafting: a virtual car sits directly ahead on the same heading, so the
        // human catches its slipstream and is towed further in the frame.
        let mut draft_app = setup_test_app();
        let drafter = spawn_player(&mut draft_app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut draft_app, drafter);
        spawn_leading_car(&mut draft_app, Vec3::new(0.0, 200.0, 4.0));
        draft_app
            .world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        draft_app.update();
        let drafting_y = draft_app
            .world
            .get::<Transform>(drafter)
            .unwrap()
            .translation
            .y;

        assert!(
            drafting_y > lone_y,
            "drafting should tow the human further: lone={lone_y}, drafting={drafting_y}"
        );
    }

    #[test]
    fn a_flag_carrying_human_catches_no_slipstream() {
        // The human hauling a flag, measured with and without a virtual car planted
        // directly ahead. The flag spoils the draft, so the human covers the
        // identical ground either way: the slipstream never speeds a flag run.
        fn carrier_y(with_leader: bool) -> f32 {
            let mut app = setup_test_app();
            let player = spawn_player(&mut app, Vec3::new(0.0, 0.0, 5.0));
            spawn_wheels(&mut app, player);
            app.world.spawn(CtfFlag {
                team: FlagTeam::Blue,
                home: Vec2::new(0.0, -1000.0),
                holder: Some(player),
            });
            if with_leader {
                spawn_leading_car(&mut app, Vec3::new(0.0, 200.0, 4.0));
            }
            app.world
                .resource_mut::<Input<KeyCode>>()
                .press(KeyCode::Up);
            app.update();
            app.world.get::<Transform>(player).unwrap().translation.y
        }

        let alone = carrier_y(false);
        let with_leader = carrier_y(true);
        assert!(
            (alone - with_leader).abs() <= 1e-3,
            "a flag-carrying human must catch no slipstream: alone={alone}, with_leader={with_leader}"
        );
    }

    #[test]
    fn battered_integrity_reduces_forward_distance() {
        let mut healthy_app = setup_test_app();
        let healthy_player = spawn_player(&mut healthy_app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut healthy_app, healthy_player);
        healthy_app
            .world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        healthy_app.update();
        let healthy_y = healthy_app
            .world
            .get::<Transform>(healthy_player)
            .unwrap()
            .translation
            .y;

        let mut wrecked_app = setup_test_app();
        wrecked_app.insert_resource(VehicleIntegrity {
            player: 0.0,
            opponent: 100.0,
        });
        let wrecked_player = spawn_player(&mut wrecked_app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut wrecked_app, wrecked_player);
        wrecked_app
            .world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        wrecked_app.update();
        let wrecked_y = wrecked_app
            .world
            .get::<Transform>(wrecked_player)
            .unwrap()
            .translation
            .y;

        assert!(
            wrecked_y > 0.0 && wrecked_y < healthy_y,
            "healthy={healthy_y}, wrecked={wrecked_y}"
        );
    }

    #[test]
    fn a_wreck_spin_out_reduces_forward_distance() {
        let mut healthy_app = setup_test_app();
        let healthy_player = spawn_player(&mut healthy_app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut healthy_app, healthy_player);
        healthy_app
            .world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        healthy_app.update();
        let healthy_y = healthy_app
            .world
            .get::<Transform>(healthy_player)
            .unwrap()
            .translation
            .y;

        let mut stunned_app = setup_test_app();
        let mut stuns = WreckStuns::default();
        stuns.trigger_player();
        stunned_app.insert_resource(stuns);
        let stunned_player = spawn_player(&mut stunned_app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut stunned_app, stunned_player);
        stunned_app
            .world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        stunned_app.update();
        let stunned_y = stunned_app
            .world
            .get::<Transform>(stunned_player)
            .unwrap()
            .translation
            .y;

        assert!(
            stunned_y > 0.0 && stunned_y < healthy_y,
            "a spun-out player should crawl forward: healthy={healthy_y}, stunned={stunned_y}"
        );
    }

    #[test]
    fn an_opponent_spin_out_does_not_slow_the_player() {
        let mut healthy_app = setup_test_app();
        let healthy_player = spawn_player(&mut healthy_app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut healthy_app, healthy_player);
        healthy_app
            .world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        healthy_app.update();
        let healthy_y = healthy_app
            .world
            .get::<Transform>(healthy_player)
            .unwrap()
            .translation
            .y;

        let mut app = setup_test_app();
        let mut stuns = WreckStuns::default();
        stuns.trigger_opponent();
        app.insert_resource(stuns);
        let player = spawn_player(&mut app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut app, player);
        app.world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        app.update();
        let player_y = app.world.get::<Transform>(player).unwrap().translation.y;

        assert!(
            (player_y - healthy_y).abs() < 1e-4,
            "the opponents' spin-out must not slow the player: healthy={healthy_y}, player={player_y}"
        );
    }

    #[test]
    fn a_fresh_kill_surge_increases_forward_distance() {
        let mut normal_app = setup_test_app();
        let normal_player = spawn_player(&mut normal_app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut normal_app, normal_player);
        normal_app
            .world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        normal_app.update();
        let normal_y = normal_app
            .world
            .get::<Transform>(normal_player)
            .unwrap()
            .translation
            .y;

        let mut surging_app = setup_test_app();
        let mut surges = WreckSurges::default();
        surges.trigger_player();
        surging_app.insert_resource(surges);
        let surging_player = spawn_player(&mut surging_app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut surging_app, surging_player);
        surging_app
            .world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        surging_app.update();
        let surging_y = surging_app
            .world
            .get::<Transform>(surging_player)
            .unwrap()
            .translation
            .y;

        assert!(
            surging_y > normal_y,
            "a fresh-kill surge should speed the player up: normal={normal_y}, surging={surging_y}"
        );
    }

    #[test]
    fn an_opponent_surge_does_not_speed_the_player() {
        let mut normal_app = setup_test_app();
        let normal_player = spawn_player(&mut normal_app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut normal_app, normal_player);
        normal_app
            .world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        normal_app.update();
        let normal_y = normal_app
            .world
            .get::<Transform>(normal_player)
            .unwrap()
            .translation
            .y;

        let mut app = setup_test_app();
        let mut surges = WreckSurges::default();
        surges.trigger_opponent();
        app.insert_resource(surges);
        let player = spawn_player(&mut app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut app, player);
        app.world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        app.update();
        let player_y = app.world.get::<Transform>(player).unwrap().translation.y;

        assert!(
            (player_y - normal_y).abs() < 1e-4,
            "the opponents' surge must not speed the player: normal={normal_y}, player={player_y}"
        );
    }

    #[test]
    fn a_sabotaged_player_team_crawls_forward() {
        let mut healthy_app = setup_test_app();
        let healthy_player = spawn_player(&mut healthy_app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut healthy_app, healthy_player);
        healthy_app
            .world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        healthy_app.update();
        let healthy_y = healthy_app
            .world
            .get::<Transform>(healthy_player)
            .unwrap()
            .translation
            .y;

        let mut sabotaged_app = setup_test_app();
        let mut effects = SabotageEffects::default();
        effects.sabotage_player();
        sabotaged_app.insert_resource(effects);
        let sabotaged_player = spawn_player(&mut sabotaged_app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut sabotaged_app, sabotaged_player);
        sabotaged_app
            .world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        sabotaged_app.update();
        let sabotaged_y = sabotaged_app
            .world
            .get::<Transform>(sabotaged_player)
            .unwrap()
            .translation
            .y;

        assert!(
            sabotaged_y > 0.0 && sabotaged_y < healthy_y,
            "a sabotaged player should crawl forward: healthy={healthy_y}, sabotaged={sabotaged_y}"
        );
    }

    #[test]
    fn sabotaging_the_opponent_does_not_slow_the_player() {
        let mut healthy_app = setup_test_app();
        let healthy_player = spawn_player(&mut healthy_app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut healthy_app, healthy_player);
        healthy_app
            .world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        healthy_app.update();
        let healthy_y = healthy_app
            .world
            .get::<Transform>(healthy_player)
            .unwrap()
            .translation
            .y;

        let mut app = setup_test_app();
        let mut effects = SabotageEffects::default();
        effects.sabotage_opponent();
        app.insert_resource(effects);
        let player = spawn_player(&mut app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut app, player);
        app.world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        app.update();
        let player_y = app.world.get::<Transform>(player).unwrap().translation.y;

        assert!(
            (player_y - healthy_y).abs() < 1e-4,
            "sabotaging red must not slow the blue player: healthy={healthy_y}, player={player_y}"
        );
    }

    #[test]
    fn carrying_the_enemy_flag_reduces_forward_distance() {
        use crate::gameplay::ctf::FLAG_CARRIER_SPEED_MULTIPLIER;

        let mut empty_handed_app = setup_test_app();
        let empty_handed_player = spawn_player(&mut empty_handed_app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut empty_handed_app, empty_handed_player);
        empty_handed_app
            .world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        empty_handed_app.update();
        let empty_handed_y = empty_handed_app
            .world
            .get::<Transform>(empty_handed_player)
            .unwrap()
            .translation
            .y;

        let mut carrier_app = setup_test_app();
        let carrier = spawn_player(&mut carrier_app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut carrier_app, carrier);
        // The human plays the blue team, so it hauls the captured red flag home.
        carrier_app.world.spawn((
            CtfFlag {
                team: FlagTeam::Red,
                home: Vec2::new(0.0, 1000.0),
                holder: Some(carrier),
            },
            Transform::from_translation(Vec3::ZERO),
        ));
        carrier_app
            .world
            .resource_mut::<Input<KeyCode>>()
            .press(KeyCode::Up);
        carrier_app.update();
        let carrier_y = carrier_app
            .world
            .get::<Transform>(carrier)
            .unwrap()
            .translation
            .y;

        assert!(
            carrier_y > 0.0 && carrier_y < empty_handed_y,
            "empty_handed={empty_handed_y}, carrier={carrier_y}"
        );
        assert!(
            (carrier_y - empty_handed_y * FLAG_CARRIER_SPEED_MULTIPLIER).abs() <= 1e-3,
            "carrier should move at the flag-carrier multiplier: \
             empty_handed={empty_handed_y}, carrier={carrier_y}"
        );
    }

    #[test]
    fn test_car_movement_backward() {
        let mut app = setup_test_app();

        let player_id = spawn_player(&mut app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut app, player_id);

        // Simulate "Down" key press
        let mut input = app.world.resource_mut::<Input<KeyCode>>();
        input.press(KeyCode::Down);

        app.update();

        let player_transform = app.world.get::<Transform>(player_id).unwrap();

        // Should move in negative Y
        assert!(player_transform.translation.y < 0.0);
    }

    #[test]
    fn test_car_rotation_left() {
        let mut app = setup_test_app();

        let player_id = spawn_player(&mut app, Vec3::new(0.0, 0.0, 5.0));
        spawn_wheels(&mut app, player_id);

        // Press Up and Left
        let mut input = app.world.resource_mut::<Input<KeyCode>>();
        input.press(KeyCode::Up);
        input.press(KeyCode::Left);

        app.update();

        let player_transform = app.world.get::<Transform>(player_id).unwrap();

        // Rotation is around Z axis.
        let (_, _, z_rot) = player_transform.rotation.to_euler(EulerRot::XYZ);
        assert!(z_rot != 0.0);
    }

    #[test]
    fn test_car_bounds_clamping() {
        let mut app = setup_test_app();

        let extents_x = BOUNDS.x / 2.0;
        let extents_y = BOUNDS.y / 2.0;

        // Spawn player at the edge
        let player_id = spawn_player(&mut app, Vec3::new(extents_x, extents_y, 5.0));
        spawn_wheels(&mut app, player_id);

        // Press Up to try and move out of bounds
        let mut input = app.world.resource_mut::<Input<KeyCode>>();
        input.press(KeyCode::Up);

        app.update();

        let player_transform = app.world.get::<Transform>(player_id).unwrap();

        // Should be clamped
        assert!(player_transform.translation.x <= extents_x + 0.001);
        assert!(player_transform.translation.y <= extents_y + 0.001);
    }
}
