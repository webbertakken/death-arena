use crate::gameplay::pickup::collect::nearest_collectible;
use crate::gameplay::pickup::{
    NitroBoosts, OpponentScore, Pickup, PickupKind, PickupRespawns, Score, PICKUP_RADIUS,
};
use crate::gameplay::player::Player;
use crate::gameplay::virtual_player::ai::AiTeam;
use crate::gameplay::virtual_player::VirtualPlayer;
use bevy::ecs::system::SystemParam;
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;

#[derive(SystemParam)]
pub struct PickupCollectionParams<'w, 's> {
    respawns: ResMut<'w, PickupRespawns>,
    nitro_boosts: ResMut<'w, NitroBoosts>,
    score: ResMut<'w, Score>,
    opponent_score: ResMut<'w, OpponentScore>,
    player_query: Query<'w, 's, &'static Transform, With<Player>>,
    virtual_player_query:
        Query<'w, 's, (&'static VirtualPlayer, &'static Transform), Without<Player>>,
    pickup_query: Query<'w, 's, (Entity, &'static Transform, &'static Pickup)>,
}

/// Collects the pickup the player is currently driving over.
///
/// Only the nearest in-range pickup is banked per frame (deterministic via
/// [`nearest_collectible`]); at 60 FPS a tight cluster is still cleared almost
/// instantly while the behaviour stays predictable and testable.
pub fn pickup_collection_system(mut commands: Commands, mut params: PickupCollectionParams) {
    let pickups: Vec<(Entity, PickupKind, Vec2)> = params
        .pickup_query
        .iter()
        .map(|(entity, transform, pickup)| (entity, pickup.kind, transform.translation.xy()))
        .collect();
    let mut available: Vec<(Entity, PickupKind, Vec2)> = pickups;

    if let Ok(player_transform) = params.player_query.get_single() {
        let collector = player_transform.translation.xy();
        let positions: Vec<Vec2> = available.iter().map(|&(_, _, pos)| pos).collect();

        if let Some(index) = nearest_collectible(collector, &positions, PICKUP_RADIUS) {
            let (entity, kind, position) = available.swap_remove(index);
            params.score.collect(kind);
            if kind == PickupKind::Nitro {
                params.nitro_boosts.trigger_player();
            }
            params.respawns.queue(kind, position);
            commands.entity(entity).despawn_recursive();
        }
    }

    for (virtual_player, virtual_player_transform) in &params.virtual_player_query {
        let collector = virtual_player_transform.translation.xy();
        let positions: Vec<Vec2> = available.iter().map(|&(_, _, pos)| pos).collect();

        if let Some(index) = nearest_collectible(collector, &positions, PICKUP_RADIUS) {
            let (entity, kind, position) = available.swap_remove(index);
            collect_for_team(
                virtual_player.team,
                kind,
                &mut params.score,
                &mut params.opponent_score,
                &mut params.nitro_boosts,
            );
            params.respawns.queue(kind, position);
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn collect_for_team(
    team: AiTeam,
    kind: PickupKind,
    score: &mut Score,
    opponent_score: &mut OpponentScore,
    nitro_boosts: &mut NitroBoosts,
) {
    match team {
        AiTeam::Blue => {
            score.collect(kind);
            if kind == PickupKind::Nitro {
                nitro_boosts.trigger_player();
            }
        }
        AiTeam::Red => {
            opponent_score.collect(kind);
            if kind == PickupKind::Nitro {
                nitro_boosts.trigger_opponent();
            }
        }
    }
}

/// Advances active nitro timers by one fixed frame.
pub fn nitro_boost_decay_system(mut nitro_boosts: ResMut<NitroBoosts>) {
    nitro_boosts.tick();
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
    use crate::gameplay::virtual_player::ai::AiTeam;

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
        app.init_resource::<OpponentScore>();
        app.init_resource::<PickupRespawns>();
        app.init_resource::<NitroBoosts>();
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

    fn spawn_virtual_player(app: &mut App, team: AiTeam, position: Vec3) -> Entity {
        app.world
            .spawn((
                VirtualPlayer {
                    team,
                    movement_speed: 0.0,
                    rotation_speed: 0.0,
                    waypoints: vec![],
                    current_waypoint: 0,
                },
                Transform::from_translation(position),
            ))
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
        app.init_resource::<OpponentScore>();
        app.init_resource::<PickupRespawns>();
        app.init_resource::<NitroBoosts>();
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
        app.init_resource::<OpponentScore>();
        app.init_resource::<PickupRespawns>();
        app.init_resource::<NitroBoosts>();
        app.add_system(pickup_collection_system);
        spawn_virtual_player(&mut app, AiTeam::Red, Vec3::ZERO);
        let pickup = spawn_pickup(&mut app, PickupKind::Cash, Vec3::new(10.0, 0.0, 0.0));

        app.update();

        assert!(
            app.world.get_entity(pickup).is_none(),
            "opponent should remove stolen pickup"
        );
        assert_eq!(app.world.resource::<Score>().collected, 0);
        assert_eq!(app.world.resource::<Score>().cash, 0);
        assert_eq!(app.world.resource::<OpponentScore>().collected, 1);
        assert_eq!(
            app.world.resource::<OpponentScore>().cash,
            PickupKind::Cash.bounty()
        );
    }

    #[test]
    fn teammate_pickup_banks_player_score() {
        let mut app = App::new();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<PickupRespawns>();
        app.init_resource::<NitroBoosts>();
        app.add_system(pickup_collection_system);
        spawn_virtual_player(&mut app, AiTeam::Blue, Vec3::ZERO);
        let pickup = spawn_pickup(&mut app, PickupKind::Cash, Vec3::new(10.0, 0.0, 0.0));

        app.update();

        assert!(
            app.world.get_entity(pickup).is_none(),
            "teammate should collect the pickup"
        );
        assert_eq!(app.world.resource::<Score>().collected, 1);
        assert_eq!(
            app.world.resource::<Score>().cash,
            PickupKind::Cash.bounty()
        );
        assert_eq!(app.world.resource::<OpponentScore>().collected, 0);
        assert_eq!(app.world.resource::<OpponentScore>().cash, 0);
    }

    #[test]
    fn nitro_pickup_arms_player_boost() {
        let mut app = app_with_player_at(Vec3::ZERO);
        spawn_pickup(&mut app, PickupKind::Nitro, Vec3::new(10.0, 0.0, 0.0));

        app.update();

        let boosts = app.world.resource::<NitroBoosts>();
        assert_eq!(
            boosts.player_frames,
            crate::gameplay::pickup::NITRO_BOOST_FRAMES
        );
        assert_eq!(boosts.opponent_frames, 0);
    }

    #[test]
    fn nitro_pickup_arms_opponent_boost() {
        let mut app = App::new();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<PickupRespawns>();
        app.init_resource::<NitroBoosts>();
        app.add_system(pickup_collection_system);
        spawn_virtual_player(&mut app, AiTeam::Red, Vec3::ZERO);
        spawn_pickup(&mut app, PickupKind::Nitro, Vec3::new(10.0, 0.0, 0.0));

        app.update();

        let boosts = app.world.resource::<NitroBoosts>();
        assert_eq!(boosts.player_frames, 0);
        assert_eq!(
            boosts.opponent_frames,
            crate::gameplay::pickup::NITRO_BOOST_FRAMES
        );
    }

    #[test]
    fn teammate_nitro_pickup_arms_player_boost() {
        let mut app = App::new();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<PickupRespawns>();
        app.init_resource::<NitroBoosts>();
        app.add_system(pickup_collection_system);
        spawn_virtual_player(&mut app, AiTeam::Blue, Vec3::ZERO);
        spawn_pickup(&mut app, PickupKind::Nitro, Vec3::new(10.0, 0.0, 0.0));

        app.update();

        let boosts = app.world.resource::<NitroBoosts>();
        assert_eq!(
            boosts.player_frames,
            crate::gameplay::pickup::NITRO_BOOST_FRAMES
        );
        assert_eq!(boosts.opponent_frames, 0);
    }

    #[test]
    fn nitro_boost_decay_counts_down() {
        let mut app = App::new();
        app.init_resource::<NitroBoosts>();
        app.add_system(nitro_boost_decay_system);
        app.world.resource_mut::<NitroBoosts>().trigger_player();

        app.update();

        assert_eq!(
            app.world.resource::<NitroBoosts>().player_frames,
            crate::gameplay::pickup::NITRO_BOOST_FRAMES - 1
        );
    }

    #[test]
    fn player_gets_priority_when_sharing_pickup_with_virtual_player() {
        let mut app = app_with_player_at(Vec3::ZERO);
        spawn_virtual_player(&mut app, AiTeam::Red, Vec3::ZERO);
        let pickup = spawn_pickup(&mut app, PickupKind::Cash, Vec3::new(10.0, 0.0, 0.0));

        app.update();

        assert!(app.world.get_entity(pickup).is_none());
        assert_eq!(app.world.resource::<Score>().collected, 1);
        assert_eq!(
            app.world.resource::<Score>().cash,
            PickupKind::Cash.bounty()
        );
        assert_eq!(app.world.resource::<OpponentScore>().collected, 0);
        assert_eq!(app.world.resource::<OpponentScore>().cash, 0);
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
