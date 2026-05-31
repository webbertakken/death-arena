#[cfg(test)]
mod tests {
    use crate::gameplay::main::BOUNDS;
    use crate::gameplay::pickup::NitroBoosts;
    use crate::gameplay::player::car::{FrontLeftWheel, FrontRightWheel};
    use crate::gameplay::player::movement::car_movement_system;
    use crate::gameplay::player::Player;
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

    fn spawn_wheels(app: &mut App, player_entity: Entity) -> (Entity, Entity) {
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
