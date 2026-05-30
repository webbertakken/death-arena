use crate::gameplay::pickup::collect::nearest_collectible;
use crate::gameplay::pickup::{Pickup, PickupKind, Score, PICKUP_RADIUS};
use crate::gameplay::player::Player;
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;

/// Collects the pickup the player is currently driving over.
///
/// Only the nearest in-range pickup is banked per frame (deterministic via
/// [`nearest_collectible`]); at 60 FPS a tight cluster is still cleared almost
/// instantly while the behaviour stays predictable and testable.
pub fn pickup_collection_system(
    mut commands: Commands,
    mut score: ResMut<Score>,
    player_query: Query<&Transform, With<Player>>,
    pickup_query: Query<(Entity, &Transform, &Pickup)>,
) {
    let Ok(player_transform) = player_query.get_single() else {
        return;
    };
    let collector = player_transform.translation.xy();

    let pickups: Vec<(Entity, PickupKind, Vec2)> = pickup_query
        .iter()
        .map(|(entity, transform, pickup)| (entity, pickup.kind, transform.translation.xy()))
        .collect();
    let positions: Vec<Vec2> = pickups.iter().map(|&(_, _, pos)| pos).collect();

    if let Some(index) = nearest_collectible(collector, &positions, PICKUP_RADIUS) {
        let (entity, kind, _) = pickups[index];
        score.collect(kind);
        commands.entity(entity).despawn_recursive();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_player() -> Player {
        Player {
            movement_speed: 0.0,
            rotation_speed: 0.0,
            engine_max_speed_multiplier: 0.0,
            forward_max_speed_base: 0.0,
            backward_max_speed_base: 0.0,
            wheels_turning_multiplier: 0.0,
        }
    }

    fn app_with_player_at(position: Vec3) -> App {
        let mut app = App::new();
        app.init_resource::<Score>();
        app.add_system(pickup_collection_system);
        app.world
            .spawn((test_player(), Transform::from_translation(position)));
        app
    }

    fn spawn_pickup(app: &mut App, kind: PickupKind, position: Vec3) -> Entity {
        app.world
            .spawn((Pickup { kind }, Transform::from_translation(position)))
            .id()
    }

    #[test]
    fn drives_over_pickup_banks_its_bounty_and_despawns_it() {
        let mut app = app_with_player_at(Vec3::ZERO);
        let pickup = spawn_pickup(&mut app, PickupKind::Cash, Vec3::new(10.0, 0.0, 0.0));

        app.update();

        assert!(
            app.world.get_entity(pickup).is_none(),
            "collected pickup should be despawned"
        );
        let score = app.world.resource::<Score>();
        assert_eq!(score.collected, 1);
        assert_eq!(score.cash, PickupKind::Cash.bounty());
    }

    #[test]
    fn leaves_out_of_range_pickups_untouched() {
        let mut app = app_with_player_at(Vec3::ZERO);
        let pickup = spawn_pickup(
            &mut app,
            PickupKind::Cash,
            Vec3::new(PICKUP_RADIUS + 50.0, 0.0, 0.0),
        );

        app.update();

        assert!(
            app.world.get_entity(pickup).is_some(),
            "distant pickup should survive"
        );
        assert_eq!(app.world.resource::<Score>().collected, 0);
    }

    #[test]
    fn collects_only_the_nearest_pickup_per_frame() {
        let mut app = app_with_player_at(Vec3::ZERO);
        let near = spawn_pickup(&mut app, PickupKind::Cash, Vec3::new(10.0, 0.0, 0.0));
        let far = spawn_pickup(&mut app, PickupKind::Nitro, Vec3::new(80.0, 0.0, 0.0));

        app.update();

        assert!(app.world.get_entity(near).is_none(), "nearest collected");
        assert!(
            app.world.get_entity(far).is_some(),
            "second pickup waits for next frame"
        );
        assert_eq!(app.world.resource::<Score>().collected, 1);
    }

    #[test]
    fn does_nothing_without_a_player() {
        let mut app = App::new();
        app.init_resource::<Score>();
        app.add_system(pickup_collection_system);
        let pickup = spawn_pickup(&mut app, PickupKind::Cash, Vec3::ZERO);

        app.update();

        assert!(app.world.get_entity(pickup).is_some());
        assert_eq!(app.world.resource::<Score>().collected, 0);
    }
}
