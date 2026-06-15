use crate::gameplay::combat::VehicleIntegrity;
use crate::gameplay::ctf::CtfMatchResult;
use crate::gameplay::pickup::{
    ArmourBoosts, NitroBoosts, OpponentScore, Pickup, PickupKind, PickupRespawns, SabotageEffects,
    Score, PICKUP_RADIUS,
};
use crate::gameplay::player::Player;
use crate::gameplay::virtual_player::ai::AiTeam;
use crate::gameplay::virtual_player::VirtualPlayer;
use bevy::ecs::system::SystemParam;
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;

#[derive(SystemParam)]
pub struct PickupCollectionParams<'w, 's> {
    match_result: Option<Res<'w, CtfMatchResult>>,
    respawns: ResMut<'w, PickupRespawns>,
    nitro_boosts: ResMut<'w, NitroBoosts>,
    armour_boosts: ResMut<'w, ArmourBoosts>,
    sabotage_effects: ResMut<'w, SabotageEffects>,
    integrity: Option<ResMut<'w, VehicleIntegrity>>,
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
/// [`nearest_claimed_pickup`]); at 60 FPS a tight cluster is still cleared almost
/// instantly while the behaviour stays predictable and testable.
pub fn pickup_collection_system(mut commands: Commands, mut params: PickupCollectionParams) {
    if params
        .match_result
        .as_ref()
        .is_some_and(|result| result.winner.is_some())
    {
        return;
    }

    let pickups: Vec<(Entity, PickupKind, Vec2)> = params
        .pickup_query
        .iter()
        .map(|(entity, transform, pickup)| (entity, pickup.kind, transform.translation.xy()))
        .collect();
    let mut available: Vec<(Entity, PickupKind, Vec2)> = pickups;

    let collectors = pickup_collectors(&params.player_query, &params.virtual_player_query);

    for (collector_index, collector) in collectors.iter().copied().enumerate() {
        if let Some(index) = nearest_claimed_pickup(collector_index, &collectors, &available) {
            let (entity, kind, position) = available.swap_remove(index);
            collect_for_team(
                collector.team,
                kind,
                &mut params.score,
                &mut params.opponent_score,
                &mut params.nitro_boosts,
                &mut params.armour_boosts,
                &mut params.sabotage_effects,
                params.integrity.as_deref_mut(),
            );
            params.respawns.queue(kind, position);
            commands.entity(entity).despawn_recursive();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PickupCollector {
    team: AiTeam,
    position: Vec2,
}

fn pickup_collectors(
    player_query: &Query<&Transform, With<Player>>,
    virtual_player_query: &Query<(&VirtualPlayer, &Transform), Without<Player>>,
) -> Vec<PickupCollector> {
    let mut collectors = Vec::new();

    if let Ok(player_transform) = player_query.get_single() {
        collectors.push(PickupCollector {
            team: AiTeam::Blue,
            position: player_transform.translation.xy(),
        });
    }

    collectors.extend(
        virtual_player_query
            .iter()
            .map(|(virtual_player, transform)| PickupCollector {
                team: virtual_player.team,
                position: transform.translation.xy(),
            }),
    );

    collectors
}

fn nearest_claimed_pickup(
    collector_index: usize,
    collectors: &[PickupCollector],
    pickups: &[(Entity, PickupKind, Vec2)],
) -> Option<usize> {
    pickups
        .iter()
        .enumerate()
        .filter_map(|(pickup_index, &(_, _, position))| {
            collector_claim_distance_sq(collector_index, collectors, position)
                .map(|distance_sq| (pickup_index, distance_sq))
        })
        .min_by(|(a_index, a_dist), (b_index, b_dist)| {
            a_dist
                .partial_cmp(b_dist)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a_index.cmp(b_index))
        })
        .map(|(pickup_index, _)| pickup_index)
}

fn collector_claim_distance_sq(
    collector_index: usize,
    collectors: &[PickupCollector],
    pickup_position: Vec2,
) -> Option<f32> {
    let collector_distance_sq = collectors
        .get(collector_index)?
        .position
        .distance_squared(pickup_position);
    if collector_distance_sq > PICKUP_RADIUS * PICKUP_RADIUS {
        return None;
    }

    collectors
        .iter()
        .enumerate()
        .filter_map(|(index, collector)| {
            let distance_sq = collector.position.distance_squared(pickup_position);
            (distance_sq <= PICKUP_RADIUS * PICKUP_RADIUS).then_some((index, distance_sq))
        })
        .min_by(|(a_index, a_dist), (b_index, b_dist)| {
            a_dist
                .partial_cmp(b_dist)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a_index.cmp(b_index))
        })
        .filter(|(winner_index, _)| *winner_index == collector_index)
        .map(|(_, distance_sq)| distance_sq)
}

#[allow(clippy::too_many_arguments)]
fn collect_for_team(
    team: AiTeam,
    kind: PickupKind,
    score: &mut Score,
    opponent_score: &mut OpponentScore,
    nitro_boosts: &mut NitroBoosts,
    armour_boosts: &mut ArmourBoosts,
    sabotage_effects: &mut SabotageEffects,
    integrity: Option<&mut VehicleIntegrity>,
) {
    match team {
        AiTeam::Blue => {
            score.collect(kind);
            if kind == PickupKind::Nitro {
                nitro_boosts.trigger_player();
            }
            if kind == PickupKind::Shield {
                armour_boosts.trigger_player();
            }
            // A sabotage charge slows the *enemy* team: blue grabs it, red bogs.
            if kind == PickupKind::Sabotage {
                sabotage_effects.sabotage_opponent();
            }
        }
        AiTeam::Red => {
            opponent_score.collect(kind);
            if kind == PickupKind::Nitro {
                nitro_boosts.trigger_opponent();
            }
            if kind == PickupKind::Shield {
                armour_boosts.trigger_opponent();
            }
            if kind == PickupKind::Sabotage {
                sabotage_effects.sabotage_player();
            }
        }
    }

    if kind == PickupKind::Repair {
        if let Some(integrity) = integrity {
            integrity.repair(team);
        }
    }
}

/// Advances active nitro timers by one fixed frame.
pub fn nitro_boost_decay_system(
    match_result: Option<Res<CtfMatchResult>>,
    mut nitro_boosts: ResMut<NitroBoosts>,
) {
    if match_result
        .as_ref()
        .is_some_and(|result| result.winner.is_some())
    {
        return;
    }

    nitro_boosts.tick();
}

/// Advances active shield armour timers by one fixed frame.
pub fn armour_boost_decay_system(
    match_result: Option<Res<CtfMatchResult>>,
    mut armour_boosts: ResMut<ArmourBoosts>,
) {
    if match_result
        .as_ref()
        .is_some_and(|result| result.winner.is_some())
    {
        return;
    }

    armour_boosts.tick();
}

/// Advances active engine-sabotage timers by one fixed frame.
pub fn sabotage_effect_decay_system(
    match_result: Option<Res<CtfMatchResult>>,
    mut sabotage_effects: ResMut<SabotageEffects>,
) {
    if match_result
        .as_ref()
        .is_some_and(|result| result.winner.is_some())
    {
        return;
    }

    sabotage_effects.tick();
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
    match_result: Option<Res<CtfMatchResult>>,
    mut respawns: ResMut<PickupRespawns>,
) {
    if match_result
        .as_ref()
        .is_some_and(|result| result.winner.is_some())
    {
        return;
    }

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
    use crate::gameplay::ctf::{CtfMatchResult, CtfMatchWinner};
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
        app.init_resource::<ArmourBoosts>();
        app.init_resource::<SabotageEffects>();
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
                    player_pursuit_radius: 0.0,
                    pickup_pursuit_radius: 0.0,
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
        app.init_resource::<ArmourBoosts>();
        app.init_resource::<SabotageEffects>();
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
        app.init_resource::<ArmourBoosts>();
        app.init_resource::<SabotageEffects>();
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
        app.init_resource::<ArmourBoosts>();
        app.init_resource::<SabotageEffects>();
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
    fn repair_pickup_restores_player_integrity() {
        let mut app = app_with_player_at(Vec3::ZERO);
        app.insert_resource(VehicleIntegrity {
            player: 50.0,
            opponent: 50.0,
        });
        spawn_pickup(&mut app, PickupKind::Repair, Vec3::new(10.0, 0.0, 0.0));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        assert!(
            (integrity.player - (50.0 + crate::gameplay::combat::REPAIR_INTEGRITY)).abs() < 1e-4,
            "player integrity not repaired: {}",
            integrity.player
        );
        assert!(
            (integrity.opponent - 50.0).abs() < 1e-4,
            "opponent integrity should be untouched: {}",
            integrity.opponent
        );
    }

    #[test]
    fn repair_pickup_restores_opponent_integrity() {
        let mut app = App::new();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<PickupRespawns>();
        app.init_resource::<NitroBoosts>();
        app.init_resource::<ArmourBoosts>();
        app.init_resource::<SabotageEffects>();
        app.insert_resource(VehicleIntegrity {
            player: 50.0,
            opponent: 50.0,
        });
        app.add_system(pickup_collection_system);
        spawn_virtual_player(&mut app, AiTeam::Red, Vec3::ZERO);
        spawn_pickup(&mut app, PickupKind::Repair, Vec3::new(10.0, 0.0, 0.0));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        assert!(
            (integrity.opponent - (50.0 + crate::gameplay::combat::REPAIR_INTEGRITY)).abs() < 1e-4,
            "opponent integrity not repaired: {}",
            integrity.opponent
        );
        assert!(
            (integrity.player - 50.0).abs() < 1e-4,
            "player integrity should be untouched: {}",
            integrity.player
        );
    }

    #[test]
    fn nitro_pickup_arms_opponent_boost() {
        let mut app = App::new();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<PickupRespawns>();
        app.init_resource::<NitroBoosts>();
        app.init_resource::<ArmourBoosts>();
        app.init_resource::<SabotageEffects>();
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
        app.init_resource::<ArmourBoosts>();
        app.init_resource::<SabotageEffects>();
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
        app.init_resource::<ArmourBoosts>();
        app.init_resource::<SabotageEffects>();
        app.add_system(nitro_boost_decay_system);
        app.world.resource_mut::<NitroBoosts>().trigger_player();

        app.update();

        assert_eq!(
            app.world.resource::<NitroBoosts>().player_frames,
            crate::gameplay::pickup::NITRO_BOOST_FRAMES - 1
        );
    }

    #[test]
    fn finished_match_pauses_nitro_boost_decay() {
        let mut app = App::new();
        app.init_resource::<NitroBoosts>();
        app.init_resource::<ArmourBoosts>();
        app.init_resource::<SabotageEffects>();
        app.insert_resource(CtfMatchResult {
            winner: Some(CtfMatchWinner::Player),
        });
        app.add_system(nitro_boost_decay_system);
        app.world.resource_mut::<NitroBoosts>().trigger_player();

        app.update();

        assert_eq!(
            app.world.resource::<NitroBoosts>().player_frames,
            crate::gameplay::pickup::NITRO_BOOST_FRAMES
        );
    }

    #[test]
    fn shield_pickup_arms_player_armour() {
        let mut app = app_with_player_at(Vec3::ZERO);
        spawn_pickup(&mut app, PickupKind::Shield, Vec3::new(10.0, 0.0, 0.0));

        app.update();

        let boosts = app.world.resource::<ArmourBoosts>();
        assert_eq!(
            boosts.player_frames,
            crate::gameplay::pickup::SHIELD_BOOST_FRAMES
        );
        assert_eq!(boosts.opponent_frames, 0);
        assert_eq!(
            app.world.resource::<NitroBoosts>(),
            &NitroBoosts::default(),
            "a shield must not arm nitro"
        );
    }

    #[test]
    fn shield_pickup_arms_opponent_armour() {
        let mut app = App::new();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<PickupRespawns>();
        app.init_resource::<NitroBoosts>();
        app.init_resource::<ArmourBoosts>();
        app.init_resource::<SabotageEffects>();
        app.add_system(pickup_collection_system);
        spawn_virtual_player(&mut app, AiTeam::Red, Vec3::ZERO);
        spawn_pickup(&mut app, PickupKind::Shield, Vec3::new(10.0, 0.0, 0.0));

        app.update();

        let boosts = app.world.resource::<ArmourBoosts>();
        assert_eq!(boosts.player_frames, 0);
        assert_eq!(
            boosts.opponent_frames,
            crate::gameplay::pickup::SHIELD_BOOST_FRAMES
        );
        assert_eq!(
            app.world.resource::<OpponentScore>().cash,
            PickupKind::Shield.bounty(),
            "a shield should still pay its modest bounty"
        );
    }

    #[test]
    fn armour_boost_decay_counts_down() {
        let mut app = App::new();
        app.init_resource::<ArmourBoosts>();
        app.init_resource::<SabotageEffects>();
        app.add_system(armour_boost_decay_system);
        app.world.resource_mut::<ArmourBoosts>().trigger_player();

        app.update();

        assert_eq!(
            app.world.resource::<ArmourBoosts>().player_frames,
            crate::gameplay::pickup::SHIELD_BOOST_FRAMES - 1
        );
    }

    #[test]
    fn finished_match_pauses_armour_boost_decay() {
        let mut app = App::new();
        app.init_resource::<ArmourBoosts>();
        app.init_resource::<SabotageEffects>();
        app.insert_resource(CtfMatchResult {
            winner: Some(CtfMatchWinner::Player),
        });
        app.add_system(armour_boost_decay_system);
        app.world.resource_mut::<ArmourBoosts>().trigger_player();

        app.update();

        assert_eq!(
            app.world.resource::<ArmourBoosts>().player_frames,
            crate::gameplay::pickup::SHIELD_BOOST_FRAMES
        );
    }

    #[test]
    fn sabotage_pickup_slows_the_opponent_when_player_team_collects() {
        let mut app = app_with_player_at(Vec3::ZERO);
        spawn_pickup(&mut app, PickupKind::Sabotage, Vec3::new(10.0, 0.0, 0.0));

        app.update();

        let effects = app.world.resource::<SabotageEffects>();
        assert_eq!(
            effects.opponent_frames,
            crate::gameplay::pickup::SABOTAGE_FRAMES,
            "the player team's grab should bog the opponents down"
        );
        assert_eq!(
            effects.player_frames, 0,
            "a team must never sabotage its own engines"
        );
        assert_eq!(
            app.world.resource::<Score>().cash,
            PickupKind::Sabotage.bounty(),
            "a sabotage should still pay its modest bounty"
        );
    }

    #[test]
    fn sabotage_pickup_slows_the_player_team_when_opponent_collects() {
        let mut app = App::new();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<PickupRespawns>();
        app.init_resource::<NitroBoosts>();
        app.init_resource::<ArmourBoosts>();
        app.init_resource::<SabotageEffects>();
        app.add_system(pickup_collection_system);
        spawn_virtual_player(&mut app, AiTeam::Red, Vec3::ZERO);
        spawn_pickup(&mut app, PickupKind::Sabotage, Vec3::new(10.0, 0.0, 0.0));

        app.update();

        let effects = app.world.resource::<SabotageEffects>();
        assert_eq!(
            effects.player_frames,
            crate::gameplay::pickup::SABOTAGE_FRAMES,
            "the opponents' grab should bog the player team down"
        );
        assert_eq!(effects.opponent_frames, 0);
        assert_eq!(
            app.world.resource::<OpponentScore>().cash,
            PickupKind::Sabotage.bounty()
        );
    }

    #[test]
    fn sabotage_effect_decay_counts_down() {
        let mut app = App::new();
        app.init_resource::<SabotageEffects>();
        app.add_system(sabotage_effect_decay_system);
        app.world
            .resource_mut::<SabotageEffects>()
            .sabotage_player();

        app.update();

        assert_eq!(
            app.world.resource::<SabotageEffects>().player_frames,
            crate::gameplay::pickup::SABOTAGE_FRAMES - 1
        );
    }

    #[test]
    fn finished_match_pauses_sabotage_effect_decay() {
        let mut app = App::new();
        app.init_resource::<SabotageEffects>();
        app.insert_resource(CtfMatchResult {
            winner: Some(CtfMatchWinner::Player),
        });
        app.add_system(sabotage_effect_decay_system);
        app.world
            .resource_mut::<SabotageEffects>()
            .sabotage_player();

        app.update();

        assert_eq!(
            app.world.resource::<SabotageEffects>().player_frames,
            crate::gameplay::pickup::SABOTAGE_FRAMES
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
    fn closer_opponent_wins_contested_pickup() {
        let mut app = app_with_player_at(Vec3::ZERO);
        spawn_virtual_player(&mut app, AiTeam::Red, Vec3::new(90.0, 0.0, 0.0));
        let pickup = spawn_pickup(&mut app, PickupKind::Cash, Vec3::new(100.0, 0.0, 0.0));

        app.update();

        assert!(app.world.get_entity(pickup).is_none());
        assert_eq!(app.world.resource::<Score>().collected, 0);
        assert_eq!(app.world.resource::<Score>().cash, 0);
        assert_eq!(app.world.resource::<OpponentScore>().collected, 1);
        assert_eq!(
            app.world.resource::<OpponentScore>().cash,
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
    fn finished_match_leaves_pickups_and_score_unchanged() {
        let mut app = app_with_player_at(Vec3::ZERO);
        app.insert_resource(CtfMatchResult {
            winner: Some(CtfMatchWinner::Player),
        });
        let pickup = spawn_pickup(&mut app, PickupKind::Cash, Vec3::new(10.0, 0.0, 0.0));

        app.update();

        assert!(
            app.world.get_entity(pickup).is_some(),
            "post-match pickup should stay in the arena"
        );
        assert_eq!(app.world.resource::<Score>().collected, 0);
        assert_eq!(app.world.resource::<Score>().cash, 0);
        assert!(app.world.resource::<PickupRespawns>().pending.is_empty());
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

    #[test]
    fn finished_match_pauses_pickup_respawns() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugin(bevy::asset::AssetPlugin::default());
        app.init_resource::<PickupRespawns>();
        app.insert_resource(CtfMatchResult {
            winner: Some(CtfMatchWinner::Opponents),
        });
        app.add_system(pickup_respawn_system);
        app.world
            .resource_mut::<PickupRespawns>()
            .queue(PickupKind::Repair, Vec2::new(-12.0, 34.0));
        app.world.resource_mut::<PickupRespawns>().pending[0].frames_remaining = 1;

        app.update();

        let respawns = app.world.resource::<PickupRespawns>();
        assert_eq!(respawns.pending.len(), 1);
        assert_eq!(respawns.pending[0].frames_remaining, 1);
        assert_eq!(
            app.world.query::<&Pickup>().iter(&app.world).count(),
            0,
            "post-match respawn should stay paused"
        );
    }
}
