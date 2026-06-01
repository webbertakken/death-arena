use crate::gameplay::pickup::{NitroBoosts, OpponentScore, Score};
use crate::gameplay::player::Player;
use crate::gameplay::virtual_player::ai::AiTeam;
use crate::gameplay::virtual_player::VirtualPlayer;
use crate::{App, AppState, Plugin};
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;

pub const FLAG_TOUCH_RADIUS: f32 = 120.0;
pub const BASE_CAPTURE_RADIUS: f32 = 160.0;
pub const CAPTURES_TO_WIN: u32 = 3;
pub const CAPTURE_CASH_BOUNTY: u32 = 250;
pub const FLAG_RETURN_CASH_BOUNTY: u32 = 75;

type HumanPlayerOnly = (With<Player>, Without<CtfFlag>);
type VirtualPlayerOnly = (With<VirtualPlayer>, Without<Player>, Without<CtfFlag>);
type CtfMatchResources<'w> = (
    ResMut<'w, CaptureScore>,
    ResMut<'w, FlagReturnScore>,
    ResMut<'w, Score>,
    ResMut<'w, OpponentScore>,
    ResMut<'w, NitroBoosts>,
    ResMut<'w, CtfMatchResult>,
);

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

impl From<AiTeam> for FlagTeam {
    fn from(team: AiTeam) -> Self {
        match team {
            AiTeam::Blue => Self::Blue,
            AiTeam::Red => Self::Red,
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

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagReturnScore {
    pub player: u32,
    pub opponents: u32,
}

impl FlagReturnScore {
    const fn return_for(&mut self, collector: CollectorKind) {
        match collector {
            CollectorKind::Player => self.player += 1,
            CollectorKind::Opponent => self.opponents += 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CtfMatchWinner {
    Player,
    Opponents,
}

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CtfMatchResult {
    pub winner: Option<CtfMatchWinner>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CollectorKind {
    Player,
    Opponent,
}

impl CollectorKind {
    const fn from_team(team: FlagTeam) -> Self {
        match team {
            FlagTeam::Blue => Self::Player,
            FlagTeam::Red => Self::Opponent,
        }
    }
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
    returns: &mut FlagReturnScore,
    result: &mut CtfMatchResult,
) {
    if result.winner.is_some() {
        return;
    }

    drop_flags_with_missing_holders(flags, collectors);
    sync_carried_flags_to_holders(flags, collectors);

    for collector in collectors {
        if result.winner.is_some() {
            break;
        }

        if try_return_stolen_own_flag(flags, collector) {
            returns.return_for(collector.kind);
        }

        try_score_carried_flag(flags, collectors, collector, score, result);
    }

    if result.winner.is_none() {
        claim_touchable_enemy_flags(flags, collectors);
    }
    sync_carried_flags_to_holders(flags, collectors);
}

fn drop_flags_with_missing_holders(flags: &mut [FlagState], collectors: &[CollectorState]) {
    for flag in flags {
        if let Some(holder) = flag.holder {
            let holder_is_present = collectors
                .iter()
                .any(|collector| collector.entity == holder);
            if !holder_is_present {
                flag.holder = None;
            }
        }
    }
}

fn sync_carried_flags_to_holders(flags: &mut [FlagState], collectors: &[CollectorState]) {
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

fn try_return_stolen_own_flag(flags: &mut [FlagState], collector: &CollectorState) -> bool {
    let Some(own_flag) = flags.iter_mut().find(|flag| flag.team == collector.team) else {
        return false;
    };

    let own_flag_is_away = own_flag.holder.is_some()
        || own_flag.position.distance_squared(own_flag.home) > f32::EPSILON;
    if own_flag_is_away
        && own_flag.holder != Some(collector.entity)
        && collector.position.distance_squared(own_flag.position)
            <= FLAG_TOUCH_RADIUS * FLAG_TOUCH_RADIUS
    {
        own_flag.holder = None;
        own_flag.position = own_flag.home;
        return true;
    }

    false
}

fn try_score_carried_flag(
    flags: &mut [FlagState],
    collectors: &[CollectorState],
    collector: &CollectorState,
    score: &mut CaptureScore,
    result: &mut CtfMatchResult,
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
        || home_base_is_contested(own_flag.home, collector.team, collectors)
    {
        return false;
    }

    score.capture_for(collector.kind);
    match collector.kind {
        CollectorKind::Player if score.player >= CAPTURES_TO_WIN => {
            result.winner = Some(CtfMatchWinner::Player);
        }
        CollectorKind::Opponent if score.opponents >= CAPTURES_TO_WIN => {
            result.winner = Some(CtfMatchWinner::Opponents);
        }
        _ => {}
    }
    let carried_flag = &mut flags[carried_flag_index];
    carried_flag.holder = None;
    carried_flag.position = carried_flag.home;
    true
}

fn home_base_is_contested(home: Vec2, home_team: FlagTeam, collectors: &[CollectorState]) -> bool {
    collectors.iter().any(|collector| {
        collector.team == home_team.enemy()
            && collector.position.distance_squared(home)
                <= BASE_CAPTURE_RADIUS * BASE_CAPTURE_RADIUS
    })
}

fn claim_touchable_enemy_flags(flags: &mut [FlagState], collectors: &[CollectorState]) {
    let mut claimed_collectors = Vec::new();

    for flag_index in 0..flags.len() {
        if flags[flag_index].holder.is_some() {
            continue;
        }

        let Some((collector_index, _)) = nearest_enemy_collector_for_flag(
            &flags[flag_index],
            flags,
            collectors,
            &claimed_collectors,
        ) else {
            continue;
        };

        let collector = collectors[collector_index];
        flags[flag_index].holder = Some(collector.entity);
        claimed_collectors.push(collector.entity);
    }
}

fn nearest_enemy_collector_for_flag(
    flag: &FlagState,
    flags: &[FlagState],
    collectors: &[CollectorState],
    claimed_collectors: &[Entity],
) -> Option<(usize, f32)> {
    collectors
        .iter()
        .enumerate()
        .filter(|(_, collector)| {
            collector.team == flag.team.enemy()
                && !claimed_collectors.contains(&collector.entity)
                && !collector_is_carrying_flag(collector.entity, flag.team, flags)
        })
        .filter_map(|(index, collector)| {
            let distance_sq = collector.position.distance_squared(flag.position);
            (distance_sq <= FLAG_TOUCH_RADIUS * FLAG_TOUCH_RADIUS).then_some((index, distance_sq))
        })
        .min_by(|(a_index, a_dist), (b_index, b_dist)| {
            a_dist
                .partial_cmp(b_dist)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a_index.cmp(b_index))
        })
}

fn collector_is_carrying_flag(
    collector_entity: Entity,
    current_flag_team: FlagTeam,
    flags: &[FlagState],
) -> bool {
    flags
        .iter()
        .any(|flag| flag.team != current_flag_team && flag.holder == Some(collector_entity))
}

pub fn capture_the_flag_system(
    resources: CtfMatchResources,
    mut flag_query: Query<(Entity, &mut CtfFlag, &mut Transform)>,
    player_query: Query<(Entity, &Transform), HumanPlayerOnly>,
    virtual_player_query: Query<(Entity, &VirtualPlayer, &Transform), VirtualPlayerOnly>,
) {
    let (
        mut score,
        mut returns,
        mut player_economy,
        mut opponent_economy,
        mut nitro_boosts,
        mut result,
    ) = resources;
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
            .map(|(entity, virtual_player, transform)| CollectorState {
                entity,
                team: virtual_player.team.into(),
                kind: CollectorKind::from_team(virtual_player.team.into()),
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

    let previous_score = *score;
    let previous_returns = *returns;
    advance_capture_the_flag(
        &mut flags,
        &collectors,
        &mut score,
        &mut returns,
        &mut result,
    );
    award_capture_bounties(
        previous_score,
        *score,
        &mut player_economy,
        &mut opponent_economy,
    );
    award_capture_momentum_boosts(previous_score, *score, &mut nitro_boosts);
    award_flag_return_bounties(
        previous_returns,
        *returns,
        &mut player_economy,
        &mut opponent_economy,
    );

    for (entity, mut flag, mut transform) in &mut flag_query {
        if let Some(updated) = flags.iter().find(|updated| updated.entity == entity) {
            flag.holder = updated.holder;
            transform.translation.x = updated.position.x;
            transform.translation.y = updated.position.y;
        }
    }
}

const fn award_capture_bounties(
    previous: CaptureScore,
    current: CaptureScore,
    player_economy: &mut Score,
    opponent_economy: &mut OpponentScore,
) {
    player_economy.bank_capture_bonus(
        current.player.saturating_sub(previous.player),
        CAPTURE_CASH_BOUNTY,
    );
    opponent_economy.bank_capture_bonus(
        current.opponents.saturating_sub(previous.opponents),
        CAPTURE_CASH_BOUNTY,
    );
}

const fn award_flag_return_bounties(
    previous: FlagReturnScore,
    current: FlagReturnScore,
    player_economy: &mut Score,
    opponent_economy: &mut OpponentScore,
) {
    player_economy.bank_flag_return_bonus(
        current.player.saturating_sub(previous.player),
        FLAG_RETURN_CASH_BOUNTY,
    );
    opponent_economy.bank_flag_return_bonus(
        current.opponents.saturating_sub(previous.opponents),
        FLAG_RETURN_CASH_BOUNTY,
    );
}

const fn award_capture_momentum_boosts(
    previous: CaptureScore,
    current: CaptureScore,
    nitro_boosts: &mut NitroBoosts,
) {
    if current.player > previous.player {
        nitro_boosts.trigger_player();
    }
    if current.opponents > previous.opponents {
        nitro_boosts.trigger_opponent();
    }
}

#[derive(Default)]
pub struct CtfPlugin;

impl Plugin for CtfPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CaptureScore>()
            .init_resource::<FlagReturnScore>()
            .init_resource::<NitroBoosts>()
            .init_resource::<CtfMatchResult>()
            .add_system_set(
                SystemSet::on_update(AppState::InGame).with_system(capture_the_flag_system),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::pickup::{OpponentScore, Score};

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

    fn blue_teammate(position: Vec2) -> CollectorState {
        CollectorState {
            entity: entity(3),
            team: FlagTeam::Blue,
            kind: CollectorKind::Player,
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

    fn advance_flags(
        flags: &mut [FlagState],
        collectors: &[CollectorState],
        score: &mut CaptureScore,
    ) {
        let mut result = CtfMatchResult::default();
        let mut returns = FlagReturnScore::default();
        advance_capture_the_flag(flags, collectors, score, &mut returns, &mut result);
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
    fn capture_bounty_rewards_only_new_captures() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        award_capture_bounties(
            CaptureScore {
                player: 1,
                opponents: 2,
            },
            CaptureScore {
                player: 2,
                opponents: 2,
            },
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(player_economy.cash, CAPTURE_CASH_BOUNTY);
        assert_eq!(player_economy.captures, 1);
        assert_eq!(player_economy.collected, 0);
        assert_eq!(opponent_economy.cash, 0);
        assert_eq!(opponent_economy.captures, 0);
        assert_eq!(opponent_economy.collected, 0);
    }

    #[test]
    fn opponent_capture_bounty_goes_to_opponent_economy() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        award_capture_bounties(
            CaptureScore {
                player: 0,
                opponents: 0,
            },
            CaptureScore {
                player: 0,
                opponents: 1,
            },
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(player_economy.cash, 0);
        assert_eq!(player_economy.captures, 0);
        assert_eq!(opponent_economy.cash, CAPTURE_CASH_BOUNTY);
        assert_eq!(opponent_economy.captures, 1);
        assert_eq!(opponent_economy.collected, 0);
    }

    #[test]
    fn flag_return_bounty_rewards_only_new_returns() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        award_flag_return_bounties(
            FlagReturnScore {
                player: 1,
                opponents: 0,
            },
            FlagReturnScore {
                player: 2,
                opponents: 1,
            },
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(player_economy.cash, FLAG_RETURN_CASH_BOUNTY);
        assert_eq!(player_economy.returns, 1);
        assert_eq!(opponent_economy.cash, FLAG_RETURN_CASH_BOUNTY);
        assert_eq!(opponent_economy.returns, 1);
    }

    #[test]
    fn player_capture_triggers_team_nitro_momentum() {
        let mut nitro_boosts = NitroBoosts::default();

        award_capture_momentum_boosts(
            CaptureScore {
                player: 0,
                opponents: 0,
            },
            CaptureScore {
                player: 1,
                opponents: 0,
            },
            &mut nitro_boosts,
        );

        assert_eq!(
            nitro_boosts.player_frames,
            crate::gameplay::pickup::NITRO_BOOST_FRAMES
        );
        assert_eq!(nitro_boosts.opponent_frames, 0);
    }

    #[test]
    fn opponent_capture_triggers_team_nitro_momentum() {
        let mut nitro_boosts = NitroBoosts::default();

        award_capture_momentum_boosts(
            CaptureScore {
                player: 0,
                opponents: 0,
            },
            CaptureScore {
                player: 0,
                opponents: 1,
            },
            &mut nitro_boosts,
        );

        assert_eq!(nitro_boosts.player_frames, 0);
        assert_eq!(
            nitro_boosts.opponent_frames,
            crate::gameplay::pickup::NITRO_BOOST_FRAMES
        );
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

        advance_flags(&mut flags, &[collector], &mut score);

        assert_eq!(flags[1].holder, Some(collector.entity));
        assert_eq!(flags[1].position, collector.position);
        assert_eq!(score, CaptureScore::default());
    }

    #[test]
    fn nearest_teammate_claims_contested_enemy_flag() {
        let mut flags = vec![
            flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0)),
            flag(11, FlagTeam::Red, Vec2::ZERO),
        ];
        let far_collector = blue_collector(Vec2::new(90.0, 0.0));
        let near_collector = blue_teammate(Vec2::new(10.0, 0.0));
        let mut score = CaptureScore::default();

        advance_flags(&mut flags, &[far_collector, near_collector], &mut score);

        assert_eq!(flags[1].holder, Some(near_collector.entity));
        assert_eq!(flags[1].position, near_collector.position);
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

        advance_flags(&mut flags, &[collector], &mut score);

        assert_eq!(score.player, 1);
        assert_eq!(score.opponents, 0);
        assert_eq!(flags[1].holder, None);
        assert_eq!(flags[1].position, flags[1].home);
    }

    #[test]
    fn player_capture_at_limit_wins_the_match() {
        let mut flags = vec![
            flag(10, FlagTeam::Blue, Vec2::ZERO),
            FlagState {
                holder: Some(entity(1)),
                ..flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0))
            },
        ];
        let collector = blue_collector(Vec2::new(10.0, 0.0));
        let mut score = CaptureScore {
            player: CAPTURES_TO_WIN - 1,
            opponents: 0,
        };
        let mut returns = FlagReturnScore::default();
        let mut result = CtfMatchResult::default();

        advance_capture_the_flag(
            &mut flags,
            &[collector],
            &mut score,
            &mut returns,
            &mut result,
        );

        assert_eq!(score.player, CAPTURES_TO_WIN);
        assert_eq!(result.winner, Some(CtfMatchWinner::Player));
    }

    #[test]
    fn finished_match_ignores_later_captures() {
        let mut flags = vec![
            flag(10, FlagTeam::Blue, Vec2::ZERO),
            FlagState {
                holder: Some(entity(1)),
                ..flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0))
            },
        ];
        let collector = blue_collector(Vec2::new(10.0, 0.0));
        let mut score = CaptureScore {
            player: CAPTURES_TO_WIN,
            opponents: 0,
        };
        let mut returns = FlagReturnScore::default();
        let mut result = CtfMatchResult {
            winner: Some(CtfMatchWinner::Player),
        };

        advance_capture_the_flag(
            &mut flags,
            &[collector],
            &mut score,
            &mut returns,
            &mut result,
        );

        assert_eq!(score.player, CAPTURES_TO_WIN);
        assert_eq!(result.winner, Some(CtfMatchWinner::Player));
    }

    #[test]
    fn winning_capture_ends_same_frame_flag_interactions() {
        let mut flags = vec![
            flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0)),
            FlagState {
                holder: Some(entity(1)),
                position: Vec2::new(-500.0, 0.0),
                ..flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0))
            },
        ];
        let blue = blue_collector(Vec2::new(-500.0, 0.0));
        let teammate = blue_teammate(Vec2::new(500.0, 0.0));
        let mut score = CaptureScore {
            player: CAPTURES_TO_WIN - 1,
            opponents: CAPTURES_TO_WIN - 1,
        };
        let mut returns = FlagReturnScore::default();
        let mut result = CtfMatchResult::default();

        advance_capture_the_flag(
            &mut flags,
            &[blue, teammate],
            &mut score,
            &mut returns,
            &mut result,
        );

        assert_eq!(
            score,
            CaptureScore {
                player: CAPTURES_TO_WIN,
                opponents: CAPTURES_TO_WIN - 1,
            }
        );
        assert_eq!(result.winner, Some(CtfMatchWinner::Player));
        assert_eq!(flags[1].holder, None);
        assert_eq!(flags[1].position, flags[1].home);
    }

    #[test]
    fn missing_holder_drops_flag_at_last_position() {
        let dropped_position = Vec2::new(125.0, 50.0);
        let mut flags = vec![FlagState {
            holder: Some(entity(99)),
            position: dropped_position,
            ..flag(10, FlagTeam::Blue, Vec2::ZERO)
        }];
        let mut score = CaptureScore::default();

        advance_flags(&mut flags, &[], &mut score);

        assert_eq!(flags[0].holder, None);
        assert_eq!(flags[0].position, dropped_position);
        assert_eq!(score, CaptureScore::default());
    }

    #[test]
    fn teammate_returns_dropped_home_flag_same_frame() {
        let mut flags = vec![FlagState {
            holder: Some(entity(99)),
            position: Vec2::new(40.0, 0.0),
            ..flag(10, FlagTeam::Blue, Vec2::ZERO)
        }];
        let collector = blue_collector(Vec2::new(45.0, 0.0));
        let mut score = CaptureScore::default();

        advance_flags(&mut flags, &[collector], &mut score);

        assert_eq!(flags[0].holder, None);
        assert_eq!(flags[0].position, flags[0].home);
        assert_eq!(score, CaptureScore::default());
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

        advance_flags(&mut flags, &[collector], &mut score);

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

        advance_flags(&mut flags, &[collector], &mut score);

        assert_eq!(score, CaptureScore::default());
        assert_eq!(flags[1].holder, Some(collector.entity));
    }

    #[test]
    fn cannot_score_while_home_base_is_contested() {
        let mut flags = vec![
            flag(10, FlagTeam::Blue, Vec2::ZERO),
            FlagState {
                holder: Some(entity(1)),
                ..flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0))
            },
        ];
        let blue = blue_collector(Vec2::new(10.0, 0.0));
        let red = red_collector(Vec2::new(150.0, 0.0));
        let mut score = CaptureScore::default();

        advance_flags(&mut flags, &[blue, red], &mut score);

        assert_eq!(score, CaptureScore::default());
        assert_eq!(flags[1].holder, Some(blue.entity));
        assert_eq!(flags[1].position, blue.position);
    }

    #[test]
    fn opponent_returns_stolen_red_flag_by_tagging_player() {
        let mut flags = vec![
            flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0)),
            FlagState {
                holder: Some(entity(1)),
                position: Vec2::new(20.0, 0.0),
                ..flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0))
            },
        ];
        let collector = red_collector(Vec2::ZERO);
        let mut score = CaptureScore::default();

        advance_flags(&mut flags, &[collector], &mut score);

        assert_eq!(score, CaptureScore::default());
        assert_eq!(flags[1].holder, None);
        assert_eq!(flags[1].position, flags[1].home);
    }

    #[test]
    fn player_returns_stolen_blue_flag_by_tagging_opponent() {
        let mut flags = vec![
            FlagState {
                holder: Some(entity(2)),
                position: Vec2::new(-20.0, 0.0),
                ..flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0))
            },
            flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0)),
        ];
        let collector = blue_collector(Vec2::ZERO);
        let mut score = CaptureScore::default();

        advance_flags(&mut flags, &[collector], &mut score);

        assert_eq!(score, CaptureScore::default());
        assert_eq!(flags[0].holder, None);
        assert_eq!(flags[0].position, flags[0].home);
    }

    #[test]
    fn player_returns_dropped_blue_flag_by_touching_it() {
        let mut flags = vec![
            FlagState {
                position: Vec2::new(-20.0, 0.0),
                ..flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0))
            },
            flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0)),
        ];
        let collector = blue_collector(Vec2::ZERO);
        let mut score = CaptureScore::default();

        advance_flags(&mut flags, &[collector], &mut score);

        assert_eq!(score, CaptureScore::default());
        assert_eq!(flags[0].holder, None);
        assert_eq!(flags[0].position, flags[0].home);
    }

    #[test]
    fn player_returns_stolen_blue_flag_using_current_carrier_position() {
        let mut flags = vec![
            FlagState {
                holder: Some(entity(2)),
                position: Vec2::new(800.0, 0.0),
                ..flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0))
            },
            flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0)),
        ];
        let blue = blue_collector(Vec2::ZERO);
        let red = red_collector(Vec2::new(20.0, 0.0));
        let mut score = CaptureScore::default();

        advance_flags(&mut flags, &[blue, red], &mut score);

        assert_eq!(score, CaptureScore::default());
        assert_eq!(flags[0].holder, None);
        assert_eq!(flags[0].position, flags[0].home);
    }

    #[test]
    fn system_tracks_player_capture_without_query_conflicts() {
        let mut app = App::new();
        app.init_resource::<CaptureScore>();
        app.init_resource::<FlagReturnScore>();
        app.init_resource::<CtfMatchResult>();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<NitroBoosts>();
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
        assert_eq!(app.world.resource::<Score>().cash, CAPTURE_CASH_BOUNTY);
        assert_eq!(app.world.resource::<OpponentScore>().cash, 0);
        assert_eq!(
            app.world.resource::<NitroBoosts>().player_frames,
            crate::gameplay::pickup::NITRO_BOOST_FRAMES
        );
        assert_eq!(app.world.resource::<NitroBoosts>().opponent_frames, 0);
    }

    #[test]
    fn system_rewards_player_for_returning_home_flag() {
        let mut app = App::new();
        app.init_resource::<CaptureScore>();
        app.init_resource::<FlagReturnScore>();
        app.init_resource::<CtfMatchResult>();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<NitroBoosts>();
        app.add_system(capture_the_flag_system);
        app.world.spawn((
            test_player(),
            Transform::from_translation(Vec3::new(-20.0, 0.0, 5.0)),
        ));
        app.world.spawn((
            CtfFlag {
                team: FlagTeam::Blue,
                home: Vec2::new(-500.0, 0.0),
                holder: None,
            },
            Transform::from_translation(Vec3::new(-20.0, 0.0, 2.0)),
        ));
        app.world.spawn((
            CtfFlag {
                team: FlagTeam::Red,
                home: Vec2::new(500.0, 0.0),
                holder: None,
            },
            Transform::from_translation(Vec3::new(500.0, 0.0, 2.0)),
        ));

        app.update();

        assert_eq!(
            *app.world.resource::<FlagReturnScore>(),
            FlagReturnScore {
                player: 1,
                opponents: 0,
            }
        );
        assert_eq!(app.world.resource::<Score>().cash, FLAG_RETURN_CASH_BOUNTY);
        assert_eq!(app.world.resource::<Score>().returns, 1);
        assert_eq!(app.world.resource::<OpponentScore>().cash, 0);
    }

    #[test]
    fn system_uses_virtual_player_team_for_enemy_flags() {
        let mut app = App::new();
        app.init_resource::<CaptureScore>();
        app.init_resource::<FlagReturnScore>();
        app.init_resource::<CtfMatchResult>();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<NitroBoosts>();
        app.add_system(capture_the_flag_system);
        let virtual_player = app
            .world
            .spawn((
                VirtualPlayer {
                    team: AiTeam::Blue,
                    movement_speed: 0.0,
                    rotation_speed: 0.0,
                    waypoints: vec![],
                    current_waypoint: 0,
                },
                Transform::from_translation(Vec3::ZERO),
            ))
            .id();
        let blue_flag = app
            .world
            .spawn((
                CtfFlag {
                    team: FlagTeam::Blue,
                    home: Vec2::new(-500.0, 0.0),
                    holder: None,
                },
                Transform::from_translation(Vec3::new(20.0, 0.0, 2.0)),
            ))
            .id();
        let red_flag = app
            .world
            .spawn((
                CtfFlag {
                    team: FlagTeam::Red,
                    home: Vec2::new(500.0, 0.0),
                    holder: None,
                },
                Transform::from_translation(Vec3::new(10.0, 0.0, 2.0)),
            ))
            .id();

        app.update();

        assert_eq!(app.world.get::<CtfFlag>(blue_flag).unwrap().holder, None);
        assert_eq!(
            app.world.get::<CtfFlag>(red_flag).unwrap().holder,
            Some(virtual_player)
        );
    }

    #[test]
    fn blue_virtual_player_capture_scores_for_player_team() {
        let mut app = App::new();
        app.init_resource::<CaptureScore>();
        app.init_resource::<FlagReturnScore>();
        app.init_resource::<CtfMatchResult>();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<NitroBoosts>();
        app.add_system(capture_the_flag_system);
        let virtual_player = app
            .world
            .spawn((
                VirtualPlayer {
                    team: AiTeam::Blue,
                    movement_speed: 0.0,
                    rotation_speed: 0.0,
                    waypoints: vec![],
                    current_waypoint: 0,
                },
                Transform::from_translation(Vec3::new(-500.0, 0.0, 4.0)),
            ))
            .id();
        app.world.spawn((
            CtfFlag {
                team: FlagTeam::Blue,
                home: Vec2::new(-500.0, 0.0),
                holder: None,
            },
            Transform::from_translation(Vec3::new(-500.0, 0.0, 2.0)),
        ));
        app.world.spawn((
            CtfFlag {
                team: FlagTeam::Red,
                home: Vec2::new(500.0, 0.0),
                holder: Some(virtual_player),
            },
            Transform::from_translation(Vec3::new(-500.0, 0.0, 2.0)),
        ));

        app.update();

        assert_eq!(
            *app.world.resource::<CaptureScore>(),
            CaptureScore {
                player: 1,
                opponents: 0,
            }
        );
        assert_eq!(app.world.resource::<Score>().cash, CAPTURE_CASH_BOUNTY);
        assert_eq!(app.world.resource::<OpponentScore>().cash, 0);
        assert_eq!(
            app.world.resource::<NitroBoosts>().player_frames,
            crate::gameplay::pickup::NITRO_BOOST_FRAMES
        );
        assert_eq!(app.world.resource::<NitroBoosts>().opponent_frames, 0);
    }

    #[test]
    fn system_records_match_winner_at_capture_limit() {
        let mut app = App::new();
        app.insert_resource(CaptureScore {
            player: CAPTURES_TO_WIN - 1,
            opponents: 0,
        });
        app.init_resource::<FlagReturnScore>();
        app.init_resource::<CtfMatchResult>();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<NitroBoosts>();
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
            app.world.resource::<CtfMatchResult>().winner,
            Some(CtfMatchWinner::Player)
        );
    }
}
