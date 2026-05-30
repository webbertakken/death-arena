use crate::gameplay::pickup::collect::nearest_collectible;
use crate::gameplay::pickup::{Pickup, PickupKind, PickupRespawns, Score, PICKUP_RADIUS};
use crate::gameplay::player::Player;
use crate::gameplay::virtual_player::VirtualPlayer;
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;

/// Collects the pickup the player is currently driving over.
///
/// Only the nearest in-range pickup is banked per frame (deterministic via
/// [`nearest_collectible`]); at 60 FPS a tight cluster is still cleared almost
/// instantly while the behaviour stays predictable and testable.
pub fn pickup_collection_system(
    mut commands: Commands,
    mut respawns: ResMut<PickupRespawns>,
    mut score: ResMut<Score>,
    player_query: Query<&Transform, With<Player>>,
    virtual_player_query: Query<&Transform, (With<VirtualPlayer>, Without<Player>)>,
    pickup_query: Query<(Entity, &Transform, &Pickup)>,
) {
    let pickups: Vec<(Entity, PickupKind, Vec2)> = pickup_query
        .iter()
        .map(|(entity, transform, pickup)| (entity, pickup.kind, transform.translation.xy()))
        .collect();
    let mut available: Vec<(Entity, PickupKind, Vec2)> = pickups;

    if let Ok(player_transform) = player_query.get_single() {
        let collector = player_transform.translation.xy();
        let positions: Vec<Vec2> = available.iter().map(|&(_, _, pos)| pos).collect();

        if let Some(index) = nearest_collectible(collector, &positions, PICKUP_RADIUS) {
            let (entity, kind, position) = available.swap_remove(index);
            score.collect(kind);
            respawns.queue(kind, position);
            commands.entity(entity).despawn_recursive();
        }
    }

    for virtual_player_transform in &virtual_player_query {
        let collector = virtual_player_transform.translation.xy();
        let positions: Vec<Vec2> = available.iter().map(|&(_, _, pos)| pos).collect();

        if let Some(index) = nearest_collectible(collector, &positions, PICKUP_RADIUS) {
            let (entity, kind, position) = available.swap_remove(index);
            respawns.queue(kind, position);
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn advance_respawns(respawns: &mut PickupRespawns) -> Vec<(PickupKind, Vec2)> {
    let mut ready = Vec::new();
    let mut waiting = Vec::with_capacity(respawns.pending.len());

    for mut pending in respawns.pending.drain(..) {
        pending.frames_remaining = pending.frames_remaining.saturating_sub(1);
        if pending.frames_remaining == 0 {
            ready.push((pending.kind, pending.position));
        } else {
            waiting.push(pending);
        }
    }

    respawns.pending = waiting;
    ready
}

/// Returns collected pickups to the arena after their cooldown expires.
pub fn pickup_respawn_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut respawns: ResMut<PickupRespawns>,
) {
    let texture = asset_server.load("textures/wrench.png");
    for (kind, position) in advance_respawns(&mut respawns) {
        commands.spawn((
            Name::new("Pickup"),
            Pickup { kind },
            SpriteBundle {
                texture: texture.clone(),
                transform: Transform {
                    translation: position.extend(super::spawn::PICKUP_Z),
                    scale: Vec3::splat(0.15),
                    ..default()
                },
                ..default()
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::pickup::PICKUP_RESPAWN_FRAMES;

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
        app.init_resource::<PickupRespawns>();
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
        app.init_resource::<PickupRespawns>();
        app.add_system(pickup_collection_system);
        let pickup = spawn_pickup(&mut app, PickupKind::Cash, Vec3::ZERO);

        app.update();

        assert!(app.world.get_entity(pickup).is_some());
        assert_eq!(app.world.resource::<Score>().collected, 0);
    }

    #[test]
    fn virtual_player_steals_pickup_without_banking_player_score() {
        let mut app = App::new();
        app.init_resource::<Score>();
        app.init_resource::<PickupRespawns>();
        app.add_system(pickup_collection_system);
        app.world.spawn((
            VirtualPlayer {
                movement_speed: 0.0,
                rotation_speed: 0.0,
                waypoints: vec![],
                current_waypoint: 0,
            },
            Transform::from_translation(Vec3::ZERO),
        ));
        let pickup = spawn_pickup(&mut app, PickupKind::Cash, Vec3::new(10.0, 0.0, 0.0));

        app.update();

        assert!(
            app.world.get_entity(pickup).is_none(),
            "opponent should remove stolen pickup"
        );
        assert_eq!(app.world.resource::<Score>().collected, 0);
        assert_eq!(app.world.resource::<Score>().cash, 0);
    }

    #[test]
    fn player_gets_priority_when_sharing_pickup_with_virtual_player() {
        let mut app = app_with_player_at(Vec3::ZERO);
        app.world.spawn((
            VirtualPlayer {
                movement_speed: 0.0,
                rotation_speed: 0.0,
                waypoints: vec![],
                current_waypoint: 0,
            },
            Transform::from_translation(Vec3::ZERO),
        ));
        let pickup = spawn_pickup(&mut app, PickupKind::Cash, Vec3::new(10.0, 0.0, 0.0));

        app.update();

        assert!(app.world.get_entity(pickup).is_none());
        assert_eq!(app.world.resource::<Score>().collected, 1);
        assert_eq!(
            app.world.resource::<Score>().cash,
            PickupKind::Cash.bounty()
        );
    }

    #[test]
    fn collected_pickup_is_queued_for_respawn() {
        let mut app = app_with_player_at(Vec3::ZERO);
        spawn_pickup(&mut app, PickupKind::Nitro, Vec3::new(10.0, 20.0, 0.0));

        app.update();

        let respawns = app.world.resource::<PickupRespawns>();
        assert_eq!(respawns.pending.len(), 1);
        assert_eq!(respawns.pending[0].kind, PickupKind::Nitro);
        assert_eq!(respawns.pending[0].position, Vec2::new(10.0, 20.0));
        assert_eq!(respawns.pending[0].frames_remaining, PICKUP_RESPAWN_FRAMES);
    }

    #[test]
    fn respawn_queue_releases_pickup_after_cooldown() {
        let mut respawns = PickupRespawns::default();
        respawns.queue(PickupKind::Repair, Vec2::new(-12.0, 34.0));
        respawns.pending[0].frames_remaining = 1;

        let ready = advance_respawns(&mut respawns);

        assert_eq!(ready, vec![(PickupKind::Repair, Vec2::new(-12.0, 34.0))]);
        assert!(respawns.pending.is_empty());
    }

    #[test]
    fn respawn_queue_keeps_pickup_until_cooldown_expires() {
        let mut respawns = PickupRespawns::default();
        respawns.queue(PickupKind::Cash, Vec2::new(1.0, 2.0));

        let ready = advance_respawns(&mut respawns);

        assert!(ready.is_empty());
        assert_eq!(respawns.pending.len(), 1);
        assert_eq!(
            respawns.pending[0].frames_remaining,
            PICKUP_RESPAWN_FRAMES - 1
        );
    }
}
