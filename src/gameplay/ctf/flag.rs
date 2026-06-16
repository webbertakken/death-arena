//! The capture-the-flag flag-state mechanics: how a frame's flags and collectors
//! resolve into steals, returns and captures.
//!
//! The pure flag-collection half of the CTF model, split from the round clock
//! ([`super::clock`]), the per-team flag timers ([`super::timers`]) and the cash
//! economy ([`super::economy`]) already carved out of the parent `ctf` module.
//! [`FlagState`] and [`CollectorState`] are the per-frame snapshots the parent's
//! [`super::capture_the_flag_system`] builds from the live flag and car transforms;
//! [`advance_capture_the_flag`] is the pure rule that drops orphaned flags, syncs
//! carried flags to their holders, returns a stolen own flag, scores a carried
//! flag and claims touchable enemy flags, folding the steal/return/capture tallies
//! and settling the match winner. Nothing here drives the ECS world; the parent
//! system feeds these results back into the flag transforms and the CTF score
//! resources.

use super::{
    CaptureScore, CtfMatchResult, CtfMatchWinner, FlagReturnScore, FlagStealScore, FlagTeam,
    BASE_CAPTURE_RADIUS, CAPTURES_TO_WIN, FLAG_TOUCH_RADIUS,
};
use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CollectorKind {
    Player,
    Opponent,
}

impl CollectorKind {
    pub(super) const fn from_team(team: FlagTeam) -> Self {
        match team {
            FlagTeam::Blue => Self::Player,
            FlagTeam::Red => Self::Opponent,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct CollectorState {
    pub(super) entity: Entity,
    pub(super) team: FlagTeam,
    pub(super) kind: CollectorKind,
    pub(super) position: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct FlagState {
    pub(super) entity: Entity,
    pub(super) team: FlagTeam,
    pub(super) home: Vec2,
    pub(super) position: Vec2,
    pub(super) holder: Option<Entity>,
}

pub(super) fn advance_capture_the_flag(
    flags: &mut [FlagState],
    collectors: &[CollectorState],
    score: &mut CaptureScore,
    steals: &mut FlagStealScore,
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
        claim_touchable_enemy_flags(flags, collectors, steals);
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

fn claim_touchable_enemy_flags(
    flags: &mut [FlagState],
    collectors: &[CollectorState],
    steals: &mut FlagStealScore,
) {
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
        steals.steal_for(collector.kind);
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
