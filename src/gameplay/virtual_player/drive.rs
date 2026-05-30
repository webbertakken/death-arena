use crate::gameplay::main::{BOUNDS, TIME_STEP};
use crate::gameplay::virtual_player::ai::{compute_steering, next_waypoint};
use crate::gameplay::virtual_player::VirtualPlayer;
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;

/// Distance (world units) at which a virtual player considers a waypoint
/// reached and advances to the next one.
const WAYPOINT_ARRIVE_RADIUS: f32 = 80.0;

/// Drives every [`VirtualPlayer`] towards its current patrol waypoint, applying
/// the same movement/rotation model the human player uses.
pub fn virtual_player_drive_system(mut query: Query<(&mut VirtualPlayer, &mut Transform)>) {
    for (mut ai, mut transform) in &mut query {
        if ai.waypoints.is_empty() {
            continue;
        }

        let position = transform.translation.xy();
        let forward = (transform.rotation * Vec3::Y).xy();
        let target = ai.waypoints[ai.current_waypoint];

        let intent = compute_steering(position, forward, target, WAYPOINT_ARRIVE_RADIUS);

        if intent == crate::gameplay::virtual_player::ai::SteeringIntent::IDLE {
            ai.current_waypoint = next_waypoint(ai.current_waypoint, ai.waypoints.len());
            continue;
        }

        // Rotation: positive steer turns left (counter-clockwise).
        transform.rotate_z(intent.steer * ai.rotation_speed * TIME_STEP);

        // Translation along the (rotated) forward vector.
        let movement_direction = transform.rotation * Vec3::Y;
        let movement_distance = intent.throttle * ai.movement_speed * TIME_STEP;
        transform.translation += movement_direction * movement_distance;

        // Keep opponents inside the arena, just like the player.
        let extents = Vec3::new(BOUNDS.x / 2.0, BOUNDS.y / 2.0, 0.0);
        transform.translation.x = transform.translation.x.clamp(-extents.x, extents.x);
        transform.translation.y = transform.translation.y.clamp(-extents.y, extents.y);
        transform.translation.z = 4.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::virtual_player::VirtualPlayer;

    fn app_with_system() -> App {
        let mut app = App::new();
        app.add_system(virtual_player_drive_system);
        app
    }

    fn spawn_ai(app: &mut App, waypoints: Vec<Vec2>) -> Entity {
        app.world
            .spawn((
                VirtualPlayer {
                    movement_speed: 500.0,
                    rotation_speed: f32::to_radians(360.0),
                    waypoints,
                    current_waypoint: 0,
                },
                Transform::from_translation(Vec3::new(0.0, 0.0, 4.0)),
            ))
            .id()
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
}
