use crate::gameplay::player::Player;
use crate::gameplay::virtual_player::VirtualPlayer;
use crate::{App, AppState, Plugin};
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;

pub const FLAG_TOUCH_RADIUS: f32 = 120.0;
pub const BASE_CAPTURE_RADIUS: f32 = 160.0;

type HumanPlayerOnly = (With<Player>, Without<CtfFlag>);
type VirtualPlayerOnly = (With<VirtualPlayer>, Without<Player>, Without<CtfFlag>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagTeam {
    Blue,
    Red,
}

impl FlagTeam {
    pub const fn enemy(self) -> Self {
        match self {
            Self::Blue => Self::Red,
            Self::Red => Self::Blue,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct CtfFlag {
    pub team: FlagTeam,
    pub home: Vec2,
    pub holder: Option<Entity>,
}

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureScore {
    pub player: u32,
    pub opponents: u32,
}

impl CaptureScore {
    const fn capture_for(&mut self, collector: CollectorKind) {
        match collector {
            CollectorKind::Player => self.player += 1,
            CollectorKind::Opponent => self.opponents += 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CollectorKind {
    Player,
    Opponent,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CollectorState {
    entity: Entity,
    team: FlagTeam,
    kind: CollectorKind,
    position: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FlagState {
    entity: Entity,
    team: FlagTeam,
    home: Vec2,
    position: Vec2,
    holder: Option<Entity>,
}

#[must_use]
pub fn flag_team_from_asset_path(path: &str) -> Option<FlagTeam> {
    if path.contains("blue-flag") {
        Some(FlagTeam::Blue)
    } else if path.contains("red-flag") {
        Some(FlagTeam::Red)
    } else {
        None
    }
}

fn advance_capture_the_flag(
    flags: &mut [FlagState],
    collectors: &[CollectorState],
    score: &mut CaptureScore,
) {
    for collector in collectors {
        if try_score_carried_flag(flags, collector, score) {
            continue;
        }

        if let Some(flag_index) = nearest_touchable_enemy_flag(flags, collector) {
            flags[flag_index].holder = Some(collector.entity);
        }
    }

    for flag in flags {
        if let Some(holder) = flag.holder {
            if let Some(collector) = collectors
                .iter()
                .find(|collector| collector.entity == holder)
            {
                flag.position = collector.position;
            }
        }
    }
}

fn try_score_carried_flag(
    flags: &mut [FlagState],
    collector: &CollectorState,
    score: &mut CaptureScore,
) -> bool {
    let Some(carried_flag_index) = flags
        .iter()
        .position(|flag| flag.holder == Some(collector.entity) && flag.team != collector.team)
    else {
        return false;
    };

    let Some(own_flag) = flags.iter().find(|flag| flag.team == collector.team) else {
        return false;
    };

    let own_flag_is_home = own_flag.holder.is_none()
        && own_flag.position.distance_squared(own_flag.home) <= f32::EPSILON;
    if !own_flag_is_home
        || collector.position.distance_squared(own_flag.home)
            > BASE_CAPTURE_RADIUS * BASE_CAPTURE_RADIUS
    {
        return false;
    }

    score.capture_for(collector.kind);
    let carried_flag = &mut flags[carried_flag_index];
    carried_flag.holder = None;
    carried_flag.position = carried_flag.home;
    true
}

fn nearest_touchable_enemy_flag(flags: &[FlagState], collector: &CollectorState) -> Option<usize> {
    flags
        .iter()
        .enumerate()
        .filter_map(|(index, flag)| {
            let distance_sq = collector.position.distance_squared(flag.position);
            (flag.team == collector.team.enemy()
                && flag.holder.is_none()
                && distance_sq <= FLAG_TOUCH_RADIUS * FLAG_TOUCH_RADIUS)
                .then_some((index, distance_sq))
        })
        .min_by(|(a_index, a_dist), (b_index, b_dist)| {
            a_dist
                .partial_cmp(b_dist)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a_index.cmp(b_index))
        })
        .map(|(index, _)| index)
}

pub fn capture_the_flag_system(
    mut score: ResMut<CaptureScore>,
    mut flag_query: Query<(Entity, &mut CtfFlag, &mut Transform)>,
    player_query: Query<(Entity, &Transform), HumanPlayerOnly>,
    virtual_player_query: Query<(Entity, &Transform), VirtualPlayerOnly>,
) {
    let mut collectors = Vec::new();
    if let Ok((entity, transform)) = player_query.get_single() {
        collectors.push(CollectorState {
            entity,
            team: FlagTeam::Blue,
            kind: CollectorKind::Player,
            position: transform.translation.xy(),
        });
    }
    collectors.extend(
        virtual_player_query
            .iter()
            .map(|(entity, transform)| CollectorState {
                entity,
                team: FlagTeam::Red,
                kind: CollectorKind::Opponent,
                position: transform.translation.xy(),
            }),
    );

    let mut flags: Vec<FlagState> = flag_query
        .iter()
        .map(|(entity, flag, transform)| FlagState {
            entity,
            team: flag.team,
            home: flag.home,
            position: transform.translation.xy(),
            holder: flag.holder,
        })
        .collect();

    advance_capture_the_flag(&mut flags, &collectors, &mut score);

    for (entity, mut flag, mut transform) in &mut flag_query {
        if let Some(updated) = flags.iter().find(|updated| updated.entity == entity) {
            flag.holder = updated.holder;
            transform.translation.x = updated.position.x;
            transform.translation.y = updated.position.y;
        }
    }
}

#[derive(Default)]
pub struct CtfPlugin;

impl Plugin for CtfPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CaptureScore>().add_system_set(
            SystemSet::on_update(AppState::InGame).with_system(capture_the_flag_system),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(id: u32) -> Entity {
        Entity::from_raw(id)
    }

    fn blue_collector(position: Vec2) -> CollectorState {
        CollectorState {
            entity: entity(1),
            team: FlagTeam::Blue,
            kind: CollectorKind::Player,
            position,
        }
    }

    fn red_collector(position: Vec2) -> CollectorState {
        CollectorState {
            entity: entity(2),
            team: FlagTeam::Red,
            kind: CollectorKind::Opponent,
            position,
        }
    }

    fn flag(entity_id: u32, team: FlagTeam, home: Vec2) -> FlagState {
        FlagState {
            entity: entity(entity_id),
            team,
            home,
            position: home,
            holder: None,
        }
    }

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

    #[test]
    fn recognises_flag_sprite_paths() {
        assert_eq!(
            flag_team_from_asset_path("arenas/church_ctf_1/blue-flag.png"),
            Some(FlagTeam::Blue)
        );
        assert_eq!(
            flag_team_from_asset_path("arenas/church_ctf_1/red-flag.png"),
            Some(FlagTeam::Red)
        );
        assert_eq!(
            flag_team_from_asset_path("arenas/church_ctf_1/hedge.png"),
            None
        );
    }

    #[test]
    fn player_picks_up_red_flag_when_touching_it() {
        let mut flags = vec![
            flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0)),
            flag(11, FlagTeam::Red, Vec2::new(50.0, 0.0)),
        ];
        let collector = blue_collector(Vec2::ZERO);
        let mut score = CaptureScore::default();

        advance_capture_the_flag(&mut flags, &[collector], &mut score);

        assert_eq!(flags[1].holder, Some(collector.entity));
        assert_eq!(flags[1].position, collector.position);
        assert_eq!(score, CaptureScore::default());
    }

    #[test]
    fn player_scores_by_returning_enemy_flag_to_home_base() {
        let mut flags = vec![
            flag(10, FlagTeam::Blue, Vec2::ZERO),
            FlagState {
                holder: Some(entity(1)),
                ..flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0))
            },
        ];
        let collector = blue_collector(Vec2::new(10.0, 0.0));
        let mut score = CaptureScore::default();

        advance_capture_the_flag(&mut flags, &[collector], &mut score);

        assert_eq!(score.player, 1);
        assert_eq!(score.opponents, 0);
        assert_eq!(flags[1].holder, None);
        assert_eq!(flags[1].position, flags[1].home);
    }

    #[test]
    fn opponent_scores_by_returning_blue_flag_to_red_base() {
        let mut flags = vec![
            FlagState {
                holder: Some(entity(2)),
                ..flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0))
            },
            flag(11, FlagTeam::Red, Vec2::ZERO),
        ];
        let collector = red_collector(Vec2::new(0.0, -10.0));
        let mut score = CaptureScore::default();

        advance_capture_the_flag(&mut flags, &[collector], &mut score);

        assert_eq!(score.player, 0);
        assert_eq!(score.opponents, 1);
        assert_eq!(flags[0].holder, None);
        assert_eq!(flags[0].position, flags[0].home);
    }

    #[test]
    fn cannot_score_while_own_flag_is_stolen() {
        let mut flags = vec![
            FlagState {
                holder: Some(entity(2)),
                position: Vec2::new(200.0, 0.0),
                ..flag(10, FlagTeam::Blue, Vec2::ZERO)
            },
            FlagState {
                holder: Some(entity(1)),
                ..flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0))
            },
        ];
        let collector = blue_collector(Vec2::ZERO);
        let mut score = CaptureScore::default();

        advance_capture_the_flag(&mut flags, &[collector], &mut score);

        assert_eq!(score, CaptureScore::default());
        assert_eq!(flags[1].holder, Some(collector.entity));
    }

    #[test]
    fn system_tracks_player_capture_without_query_conflicts() {
        let mut app = App::new();
        app.init_resource::<CaptureScore>();
        app.add_system(capture_the_flag_system);
        let player = app
            .world
            .spawn((
                test_player(),
                Transform::from_translation(Vec3::new(10.0, 0.0, 5.0)),
            ))
            .id();
        app.world.spawn((
            CtfFlag {
                team: FlagTeam::Blue,
                home: Vec2::ZERO,
                holder: None,
            },
            Transform::from_translation(Vec3::new(0.0, 0.0, 2.0)),
        ));
        app.world.spawn((
            CtfFlag {
                team: FlagTeam::Red,
                home: Vec2::new(500.0, 0.0),
                holder: Some(player),
            },
            Transform::from_translation(Vec3::new(10.0, 0.0, 2.0)),
        ));

        app.update();

        assert_eq!(
            *app.world.resource::<CaptureScore>(),
            CaptureScore {
                player: 1,
                opponents: 0,
            }
        );
    }
}
