use crate::gameplay::pickup::{NitroBoosts, OpponentScore, Score};
use crate::gameplay::player::Player;
use crate::gameplay::virtual_player::ai::AiTeam;
use crate::gameplay::virtual_player::VirtualPlayer;
use crate::{App, AppState, Plugin};
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use std::cmp::Ordering;

pub const FLAG_TOUCH_RADIUS: f32 = 120.0;
pub const BASE_CAPTURE_RADIUS: f32 = 160.0;
pub const CAPTURES_TO_WIN: u32 = 3;
pub const CAPTURE_CASH_BOUNTY: u32 = 250;
pub const FLAG_STEAL_CASH_BOUNTY: u32 = 50;
pub const FLAG_RETURN_CASH_BOUNTY: u32 = 75;
/// Cash the winning team banks the instant a match resolves in its favour.
///
/// The Death Rally payday that closes the round: every in-match bounty grinds
/// out the cash that funds upgrades, but taking the match is the marquee
/// earner. Priced as the single biggest line item, comfortably above the
/// `3 * CAPTURE_CASH_BOUNTY` a clean three-capture win already banks, so the
/// scoreboard always rewards closing the round over farming it. Banked once,
/// on the frame the winner is decided.
pub const VICTORY_CASH_PURSE: u32 = 1_000;
/// Cash each team banks when a match resolves as a level draw.
///
/// A drawn match earns both sides a participation purse for fighting to a
/// standstill, kept well below [`VICTORY_CASH_PURSE`] so a win is always worth
/// far more than a deadlock.
pub const DRAW_CASH_PURSE: u32 = 250;
/// A win must always out-pay a draw, enforced at compile time.
const _: () = assert!(DRAW_CASH_PURSE < VICTORY_CASH_PURSE);
/// Fixed update frames a CTF round runs before it resolves on time.
///
/// Caps stalemates so a match always ends even if neither team reaches
/// [`CAPTURES_TO_WIN`]. At the game's 60 FPS convention this is three minutes.
pub const MATCH_TIME_LIMIT_FRAMES: u32 = 10_800;
/// Fixed update frames a sudden-death overtime runs before it resolves.
///
/// Entered when regulation expires on a perfectly level scoreline so a tied
/// match gets a dramatic decider instead of a tame draw, while still
/// guaranteeing the round terminates. At 60 FPS this is one minute.
pub const SUDDEN_DEATH_TIME_LIMIT_FRAMES: u32 = 3_600;
/// Speed multiplier a car suffers while hauling the enemy flag home.
///
/// The classic capture-the-flag tax: the heavy flag drags on the car, so the
/// run back to base becomes a tense gauntlet instead of a victory lap. Slow
/// enough that defenders and rammers get a real shot at the carrier, fast
/// enough that a clean break still rewards a daring grab. Pairs with ram
/// damage and integrity wear, so a battered carrier crawls home.
pub const FLAG_CARRIER_SPEED_MULTIPLIER: f32 = 0.8;
/// Fixed-update frames a dropped flag lies loose before it auto-returns home.
///
/// The classic capture-the-flag safeguard: a flag knocked loose (a wrecked or
/// despawned carrier drops it) and then left untouched by both teams resets to
/// its home base instead of stranding the objective in a dead corner. The timer
/// counts only frames the flag is genuinely loose; the instant any car grabs or
/// returns it the count clears. At the game's 60 FPS convention this is ten
/// seconds, long enough that only a truly abandoned flag ever resets.
pub const FLAG_RESET_FRAMES: u32 = 600;

type HumanPlayerOnly = (With<Player>, Without<CtfFlag>);
type VirtualPlayerOnly = (With<VirtualPlayer>, Without<Player>, Without<CtfFlag>);
type CtfMatchResources<'w> = (
    ResMut<'w, CaptureScore>,
    ResMut<'w, FlagStealScore>,
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

/// Per-team countdown tracking how long each side's flag has lain loose.
///
/// Mirrors [`crate::gameplay::combat::WreckStuns`]: a per-team frame counter,
/// here advanced each frame by [`capture_the_flag_system`] and read to
/// auto-return a flag abandoned past [`FLAG_RESET_FRAMES`]. Cleared the moment a
/// flag is held or sitting home, so only a genuinely loose flag ever counts up.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LooseFlagTimers {
    /// Frames the blue flag has lain loose.
    pub blue_frames: u32,
    /// Frames the red flag has lain loose.
    pub red_frames: u32,
}

impl LooseFlagTimers {
    /// Frames the given team's flag has lain loose.
    const fn frames_for(self, team: FlagTeam) -> u32 {
        match team {
            FlagTeam::Blue => self.blue_frames,
            FlagTeam::Red => self.red_frames,
        }
    }

    /// Sets the loose-frame count for the given team's flag.
    const fn set_for(&mut self, team: FlagTeam, frames: u32) {
        match team {
            FlagTeam::Blue => self.blue_frames = frames,
            FlagTeam::Red => self.red_frames = frames,
        }
    }
}

/// What a flag's loose timer dictates for the current frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LooseFlagOutcome {
    /// Held or already home: clear the timer.
    Settled,
    /// Loose and still inside the grace window: keep counting at this value.
    Counting(u32),
    /// Loose past [`FLAG_RESET_FRAMES`]: return the flag to its home base.
    ResetHome,
}

/// Advances a flag's loose timer by one fixed-update frame.
///
/// A held or home flag clears the timer ([`LooseFlagOutcome::Settled`]); a loose
/// flag counts up until it crosses [`FLAG_RESET_FRAMES`], when it is sent home.
#[must_use]
pub const fn advance_loose_flag(is_held: bool, is_at_home: bool, frames: u32) -> LooseFlagOutcome {
    if is_held || is_at_home {
        return LooseFlagOutcome::Settled;
    }
    let next = frames + 1;
    if next >= FLAG_RESET_FRAMES {
        LooseFlagOutcome::ResetHome
    } else {
        LooseFlagOutcome::Counting(next)
    }
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
pub struct FlagStealScore {
    pub player: u32,
    pub opponents: u32,
}

impl FlagStealScore {
    const fn steal_for(&mut self, collector: CollectorKind) {
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
    /// Neither team led when the match clock expired.
    Draw,
}

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CtfMatchResult {
    pub winner: Option<CtfMatchWinner>,
}

/// Tracks whether the end-of-match purse has been banked for the current round.
///
/// The winner can be settled by a capture, a sudden-death golden goal, or the
/// clock expiring, so the purse is paid by its own system reading this flag
/// rather than diffed at each award site. The flag flips the frame the purse is
/// banked and resets when a fresh match begins, guaranteeing exactly one payout
/// per round however the result is reached.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchPursePaid(pub bool);

/// Which stage of the round the clock is timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatchPhase {
    /// The main timed round.
    #[default]
    Regulation,
    /// Overtime decider entered after a level regulation scoreline.
    SuddenDeath,
}

/// Counts down the current CTF round so a stalemated match always ends.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchClock {
    pub frames_remaining: u32,
    pub phase: MatchPhase,
}

impl Default for MatchClock {
    fn default() -> Self {
        Self {
            frames_remaining: MATCH_TIME_LIMIT_FRAMES,
            phase: MatchPhase::Regulation,
        }
    }
}

impl MatchClock {
    /// Advances the clock by one fixed frame, saturating at zero.
    pub const fn tick(&mut self) {
        self.frames_remaining = self.frames_remaining.saturating_sub(1);
    }

    /// Whether the round time limit has been reached.
    pub const fn is_expired(self) -> bool {
        self.frames_remaining == 0
    }

    /// Whether the clock is timing a sudden-death overtime.
    pub const fn is_sudden_death(self) -> bool {
        matches!(self.phase, MatchPhase::SuddenDeath)
    }

    /// Refills the overtime budget and switches the clock to sudden death.
    pub const fn enter_sudden_death(&mut self) {
        self.frames_remaining = SUDDEN_DEATH_TIME_LIMIT_FRAMES;
        self.phase = MatchPhase::SuddenDeath;
    }
}

/// Resolves a timed-out match by captures, then steals, then returns.
///
/// A perfectly level scoreline across all three is a [`CtfMatchWinner::Draw`].
#[must_use]
fn time_limit_winner(
    captures: CaptureScore,
    steals: FlagStealScore,
    returns: FlagReturnScore,
) -> CtfMatchWinner {
    let player = (captures.player, steals.player, returns.player);
    let opponents = (captures.opponents, steals.opponents, returns.opponents);
    match player.cmp(&opponents) {
        Ordering::Greater => CtfMatchWinner::Player,
        Ordering::Less => CtfMatchWinner::Opponents,
        Ordering::Equal => CtfMatchWinner::Draw,
    }
}

/// Breaks an overtime level on every objective by the team that did more damage.
///
/// When sudden death expires with captures, steals, and returns all dead even,
/// the round goes to whichever side wrecked more enemy cars: the classic Death
/// Rally decider where raw aggression settles a standstill the objective could
/// not. Only a match also level on wrecks stays a true [`CtfMatchWinner::Draw`].
#[must_use]
const fn break_level_overtime_by_wrecks(
    player_wrecks: u32,
    opponent_wrecks: u32,
) -> CtfMatchWinner {
    if player_wrecks > opponent_wrecks {
        CtfMatchWinner::Player
    } else if player_wrecks < opponent_wrecks {
        CtfMatchWinner::Opponents
    } else {
        CtfMatchWinner::Draw
    }
}

/// Speed multiplier for a car given whether it is carrying the enemy flag.
///
/// A car hauling a flag drives at [`FLAG_CARRIER_SPEED_MULTIPLIER`]; an
/// empty-handed car is unaffected. Composed alongside the nitro and integrity
/// multipliers in both the player and virtual-player movement systems.
#[must_use]
pub const fn flag_carrier_speed_multiplier(is_carrying_flag: bool) -> f32 {
    if is_carrying_flag {
        FLAG_CARRIER_SPEED_MULTIPLIER
    } else {
        1.0
    }
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

/// Auto-returns any flag left loose past [`FLAG_RESET_FRAMES`] to its base.
///
/// Advances every flag's per-team loose timer: a held or home flag clears it, a
/// loose flag counts up, and one abandoned past the limit is sent home (holder
/// cleared, position reset) with its timer wiped. Runs after the steal/return
/// pass so a flag a car grabbed this frame is never yanked out from under it.
fn auto_return_loose_flags(flags: &mut [FlagState], timers: &mut LooseFlagTimers) {
    for flag in flags {
        let is_held = flag.holder.is_some();
        let is_at_home = flag.position.distance_squared(flag.home) <= f32::EPSILON;
        match advance_loose_flag(is_held, is_at_home, timers.frames_for(flag.team)) {
            LooseFlagOutcome::Settled => timers.set_for(flag.team, 0),
            LooseFlagOutcome::Counting(next) => timers.set_for(flag.team, next),
            LooseFlagOutcome::ResetHome => {
                flag.holder = None;
                flag.position = flag.home;
                timers.set_for(flag.team, 0);
            }
        }
    }
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

pub fn capture_the_flag_system(
    resources: CtfMatchResources,
    clock: Res<MatchClock>,
    mut flag_query: Query<(Entity, &mut CtfFlag, &mut Transform)>,
    player_query: Query<(Entity, &Transform), HumanPlayerOnly>,
    virtual_player_query: Query<(Entity, &VirtualPlayer, &Transform), VirtualPlayerOnly>,
    mut loose_timers: Option<ResMut<LooseFlagTimers>>,
) {
    let (
        mut score,
        mut steals,
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
    let previous_steals = *steals;
    let previous_returns = *returns;
    advance_capture_the_flag(
        &mut flags,
        &collectors,
        &mut score,
        &mut steals,
        &mut returns,
        &mut result,
    );
    if result.winner.is_none() {
        if let Some(timers) = loose_timers.as_deref_mut() {
            auto_return_loose_flags(&mut flags, timers);
        }
    }
    award_golden_goal(clock.is_sudden_death(), previous_score, *score, &mut result);
    award_capture_bounties(
        previous_score,
        *score,
        &mut player_economy,
        &mut opponent_economy,
    );
    award_flag_steal_bounties(
        previous_steals,
        *steals,
        &mut player_economy,
        &mut opponent_economy,
    );
    award_flag_steal_momentum_boosts(previous_steals, *steals, &mut nitro_boosts);
    award_capture_momentum_boosts(previous_score, *score, &mut nitro_boosts);
    award_flag_return_bounties(
        previous_returns,
        *returns,
        &mut player_economy,
        &mut opponent_economy,
    );
    award_flag_return_momentum_boosts(previous_returns, *returns, &mut nitro_boosts);

    for (entity, mut flag, mut transform) in &mut flag_query {
        if let Some(updated) = flags.iter().find(|updated| updated.entity == entity) {
            flag.holder = updated.holder;
            transform.translation.x = updated.position.x;
            transform.translation.y = updated.position.y;
        }
    }
}

/// Ends the match when the round clock runs out, resolving the leader on score.
///
/// Runs after [`capture_the_flag_system`] so a capture landed on the final frame
/// still counts before the time limit decides the result. A level regulation
/// scoreline opens a sudden-death overtime instead of a draw; an overtime still
/// level on objectives is then settled by
/// [`break_level_overtime_by_wrecks`], so only a match dead even on damage too
/// falls back to [`CtfMatchWinner::Draw`].
fn expire_match_on_time_limit(
    mut clock: ResMut<MatchClock>,
    mut result: ResMut<CtfMatchResult>,
    captures: Res<CaptureScore>,
    steals: Res<FlagStealScore>,
    returns: Res<FlagReturnScore>,
    score: Res<Score>,
    opponent_score: Res<OpponentScore>,
) {
    if result.winner.is_some() {
        return;
    }

    clock.tick();
    if !clock.is_expired() {
        return;
    }

    let leader = time_limit_winner(*captures, *steals, *returns);
    match clock.phase {
        MatchPhase::Regulation if matches!(leader, CtfMatchWinner::Draw) => {
            clock.enter_sudden_death();
            info!("CTF regulation level; entering sudden death");
        }
        MatchPhase::Regulation => {
            info!("CTF match time limit reached; resolved as {leader:?}");
            result.winner = Some(leader);
        }
        MatchPhase::SuddenDeath => {
            let resolved = match leader {
                CtfMatchWinner::Draw => {
                    break_level_overtime_by_wrecks(score.wrecks, opponent_score.wrecks)
                }
                decided => decided,
            };
            info!("CTF sudden death expired; resolved as {resolved:?}");
            result.winner = Some(resolved);
        }
    }
}

/// Ends a sudden-death overtime the instant either team lands a capture.
///
/// In regulation a lone capture is harmless; in overtime it is the golden goal
/// that decides the match, so the team whose capture tally just rose wins
/// outright regardless of [`CAPTURES_TO_WIN`].
const fn award_golden_goal(
    sudden_death: bool,
    previous: CaptureScore,
    current: CaptureScore,
    result: &mut CtfMatchResult,
) {
    if !sudden_death || result.winner.is_some() {
        return;
    }
    if current.player > previous.player {
        result.winner = Some(CtfMatchWinner::Player);
    } else if current.opponents > previous.opponents {
        result.winner = Some(CtfMatchWinner::Opponents);
    }
}

/// Banks the end-of-match purse to whichever side the result favours.
///
/// A win pays the victor [`VICTORY_CASH_PURSE`]; a draw pays both teams the
/// smaller [`DRAW_CASH_PURSE`] for fighting to a standstill. Pure cash, banked
/// on top of every in-match bounty.
const fn award_match_purse(
    winner: CtfMatchWinner,
    player_economy: &mut Score,
    opponent_economy: &mut OpponentScore,
) {
    match winner {
        CtfMatchWinner::Player => player_economy.bank_match_purse(VICTORY_CASH_PURSE),
        CtfMatchWinner::Opponents => opponent_economy.bank_match_purse(VICTORY_CASH_PURSE),
        CtfMatchWinner::Draw => {
            player_economy.bank_match_purse(DRAW_CASH_PURSE);
            opponent_economy.bank_match_purse(DRAW_CASH_PURSE);
        }
    }
}

/// Banks the match purse exactly once the round resolves, however it is decided.
///
/// Runs after [`expire_match_on_time_limit`] so a winner settled by a capture,
/// a golden goal, or the clock has all landed before the purse is paid. The
/// [`MatchPursePaid`] latch keeps the payout to a single frame even though the
/// result lingers for the rest of the frozen round.
fn award_match_purse_on_resolution(
    result: Res<CtfMatchResult>,
    mut paid: ResMut<MatchPursePaid>,
    mut player_economy: ResMut<Score>,
    mut opponent_economy: ResMut<OpponentScore>,
) {
    if paid.0 {
        return;
    }
    let Some(winner) = result.winner else {
        return;
    };

    award_match_purse(winner, &mut player_economy, &mut opponent_economy);
    paid.0 = true;
    info!("Match purse banked for {winner:?}");
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

const fn award_flag_steal_bounties(
    previous: FlagStealScore,
    current: FlagStealScore,
    player_economy: &mut Score,
    opponent_economy: &mut OpponentScore,
) {
    player_economy.bank_flag_steal_bonus(
        current.player.saturating_sub(previous.player),
        FLAG_STEAL_CASH_BOUNTY,
    );
    opponent_economy.bank_flag_steal_bonus(
        current.opponents.saturating_sub(previous.opponents),
        FLAG_STEAL_CASH_BOUNTY,
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

const fn award_flag_steal_momentum_boosts(
    previous: FlagStealScore,
    current: FlagStealScore,
    nitro_boosts: &mut NitroBoosts,
) {
    if current.player > previous.player {
        nitro_boosts.trigger_player();
    }
    if current.opponents > previous.opponents {
        nitro_boosts.trigger_opponent();
    }
}

const fn award_flag_return_momentum_boosts(
    previous: FlagReturnScore,
    current: FlagReturnScore,
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
            .init_resource::<FlagStealScore>()
            .init_resource::<FlagReturnScore>()
            .init_resource::<NitroBoosts>()
            .init_resource::<Score>()
            .init_resource::<OpponentScore>()
            .init_resource::<CtfMatchResult>()
            .init_resource::<MatchPursePaid>()
            .init_resource::<MatchClock>()
            .init_resource::<LooseFlagTimers>()
            .add_system_set(
                SystemSet::on_enter(AppState::InGame).with_system(reset_ctf_match_resources),
            )
            .add_system_set(
                SystemSet::on_update(AppState::InGame)
                    .with_system(capture_the_flag_system)
                    .with_system(expire_match_on_time_limit.after(capture_the_flag_system))
                    .with_system(award_match_purse_on_resolution.after(expire_match_on_time_limit)),
            );
    }
}

fn reset_ctf_match_resources(
    mut captures: ResMut<CaptureScore>,
    mut steals: ResMut<FlagStealScore>,
    mut returns: ResMut<FlagReturnScore>,
    mut result: ResMut<CtfMatchResult>,
    mut purse_paid: ResMut<MatchPursePaid>,
    mut clock: ResMut<MatchClock>,
    mut loose_timers: ResMut<LooseFlagTimers>,
) {
    *captures = CaptureScore::default();
    *steals = FlagStealScore::default();
    *returns = FlagReturnScore::default();
    *result = CtfMatchResult::default();
    *purse_paid = MatchPursePaid::default();
    *clock = MatchClock::default();
    *loose_timers = LooseFlagTimers::default();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::pickup::{OpponentScore, Score};

    fn entity(id: u32) -> Entity {
        Entity::from_raw(id)
    }

    fn assert_near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= f32::EPSILON,
            "actual={actual}, expected={expected}"
        );
    }

    #[test]
    fn empty_handed_car_keeps_full_speed() {
        assert_near(flag_carrier_speed_multiplier(false), 1.0);
    }

    #[test]
    fn flag_carrier_is_slowed_by_the_heavy_flag() {
        let carrying = flag_carrier_speed_multiplier(true);
        let empty_handed = flag_carrier_speed_multiplier(false);
        assert_near(carrying, FLAG_CARRIER_SPEED_MULTIPLIER);
        assert!(
            carrying < empty_handed,
            "carrying the flag must cost speed: carrying={carrying}, empty_handed={empty_handed}"
        );
        assert!(
            carrying > 0.0,
            "a carrier must still be able to move, multiplier={carrying}"
        );
    }

    #[test]
    fn entering_match_resets_ctf_scores_and_result() {
        let mut app = App::new();
        app.insert_resource(CaptureScore {
            player: 2,
            opponents: 1,
        });
        app.insert_resource(FlagStealScore {
            player: 3,
            opponents: 4,
        });
        app.insert_resource(FlagReturnScore {
            player: 5,
            opponents: 6,
        });
        app.insert_resource(CtfMatchResult {
            winner: Some(CtfMatchWinner::Opponents),
        });
        app.insert_resource(MatchPursePaid(true));
        app.insert_resource(MatchClock {
            frames_remaining: 7,
            phase: MatchPhase::SuddenDeath,
        });
        app.insert_resource(LooseFlagTimers {
            blue_frames: 120,
            red_frames: 240,
        });
        app.add_system(reset_ctf_match_resources);

        app.update();

        assert_eq!(
            *app.world.resource::<CaptureScore>(),
            CaptureScore::default()
        );
        assert_eq!(
            *app.world.resource::<FlagStealScore>(),
            FlagStealScore::default()
        );
        assert_eq!(
            *app.world.resource::<FlagReturnScore>(),
            FlagReturnScore::default()
        );
        assert_eq!(
            *app.world.resource::<CtfMatchResult>(),
            CtfMatchResult::default()
        );
        assert_eq!(
            *app.world.resource::<MatchPursePaid>(),
            MatchPursePaid::default(),
            "a fresh match must clear the purse latch so the next win pays out"
        );
        assert_eq!(*app.world.resource::<MatchClock>(), MatchClock::default());
        assert_eq!(
            *app.world.resource::<LooseFlagTimers>(),
            LooseFlagTimers::default(),
            "a fresh match must clear loose-flag timers so a stale count never resets a flag"
        );
    }

    #[test]
    fn a_held_flag_clears_its_loose_timer() {
        assert_eq!(
            advance_loose_flag(true, false, 123),
            LooseFlagOutcome::Settled,
            "a carried flag is not loose, so its timer must reset"
        );
    }

    #[test]
    fn a_home_flag_clears_its_loose_timer() {
        assert_eq!(
            advance_loose_flag(false, true, 123),
            LooseFlagOutcome::Settled,
            "a flag sitting at base is not loose, so its timer must reset"
        );
    }

    #[test]
    fn a_loose_flag_counts_up_inside_the_grace_window() {
        assert_eq!(
            advance_loose_flag(false, false, 0),
            LooseFlagOutcome::Counting(1)
        );
        assert_eq!(
            advance_loose_flag(false, false, FLAG_RESET_FRAMES - 2),
            LooseFlagOutcome::Counting(FLAG_RESET_FRAMES - 1)
        );
    }

    #[test]
    fn a_loose_flag_returns_home_at_the_reset_limit() {
        assert_eq!(
            advance_loose_flag(false, false, FLAG_RESET_FRAMES - 1),
            LooseFlagOutcome::ResetHome,
            "a flag loose for the full grace window must auto-return"
        );
    }

    #[test]
    fn auto_return_sends_an_abandoned_flag_home() {
        let home = Vec2::new(500.0, 0.0);
        let mut red = flag(1, FlagTeam::Red, home);
        red.position = Vec2::new(120.0, -40.0);
        let mut flags = [red];
        let mut timers = LooseFlagTimers {
            red_frames: FLAG_RESET_FRAMES - 1,
            ..Default::default()
        };

        auto_return_loose_flags(&mut flags, &mut timers);

        assert_eq!(
            flags[0].position, home,
            "an abandoned flag must reset to base"
        );
        assert_eq!(flags[0].holder, None);
        assert_eq!(timers.red_frames, 0, "the reset must clear the loose timer");
    }

    #[test]
    fn auto_return_keeps_counting_a_still_loose_flag() {
        let mut blue = flag(2, FlagTeam::Blue, Vec2::ZERO);
        blue.position = Vec2::new(80.0, 80.0);
        let mut flags = [blue];
        let mut timers = LooseFlagTimers::default();

        auto_return_loose_flags(&mut flags, &mut timers);

        assert_eq!(timers.blue_frames, 1, "a loose flag must keep counting");
        assert_eq!(
            flags[0].position,
            Vec2::new(80.0, 80.0),
            "a flag inside the grace window must stay put"
        );
    }

    #[test]
    fn auto_return_clears_the_timer_when_a_flag_is_recovered() {
        let mut red = flag(3, FlagTeam::Red, Vec2::new(500.0, 0.0));
        red.position = Vec2::new(200.0, 0.0);
        red.holder = Some(entity(9));
        let mut flags = [red];
        let mut timers = LooseFlagTimers {
            red_frames: 300,
            ..Default::default()
        };

        auto_return_loose_flags(&mut flags, &mut timers);

        assert_eq!(
            timers.red_frames, 0,
            "grabbing a loose flag must reset its abandonment timer"
        );
        assert_eq!(
            flags[0].position,
            Vec2::new(200.0, 0.0),
            "a recovered flag stays where its carrier holds it"
        );
    }

    #[test]
    fn system_auto_returns_a_flag_left_loose_too_long() {
        let mut app = App::new();
        app.init_resource::<CaptureScore>();
        app.init_resource::<FlagStealScore>();
        app.init_resource::<FlagReturnScore>();
        app.init_resource::<CtfMatchResult>();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<NitroBoosts>();
        app.init_resource::<MatchClock>();
        app.insert_resource(LooseFlagTimers {
            red_frames: FLAG_RESET_FRAMES - 1,
            ..Default::default()
        });
        app.add_system(capture_the_flag_system);
        // Player far from every flag so nobody touches the loose red flag.
        app.world.spawn((
            test_player(),
            Transform::from_translation(Vec3::new(-2000.0, 0.0, 5.0)),
        ));
        app.world.spawn((
            CtfFlag {
                team: FlagTeam::Blue,
                home: Vec2::new(-1000.0, 0.0),
                holder: None,
            },
            Transform::from_translation(Vec3::new(-1000.0, 0.0, 2.0)),
        ));
        let red_home = Vec2::new(500.0, 0.0);
        let red_flag = app
            .world
            .spawn((
                CtfFlag {
                    team: FlagTeam::Red,
                    home: red_home,
                    holder: None,
                },
                Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
            ))
            .id();

        app.update();

        let transform = app.world.get::<Transform>(red_flag).unwrap();
        assert_eq!(
            transform.translation.xy(),
            red_home,
            "a flag abandoned past the reset limit must auto-return to base"
        );
        assert_eq!(
            app.world.resource::<LooseFlagTimers>().red_frames,
            0,
            "the auto-return must clear the loose timer"
        );
    }

    #[test]
    fn match_clock_defaults_to_the_round_time_limit() {
        assert_eq!(
            MatchClock::default().frames_remaining,
            MATCH_TIME_LIMIT_FRAMES
        );
    }

    #[test]
    fn match_clock_ticks_down_and_saturates_at_zero() {
        let mut clock = MatchClock {
            frames_remaining: 2,
            phase: MatchPhase::Regulation,
        };

        clock.tick();
        assert_eq!(clock.frames_remaining, 1);
        assert!(!clock.is_expired());

        clock.tick();
        assert_eq!(clock.frames_remaining, 0);
        assert!(clock.is_expired());

        clock.tick();
        assert_eq!(clock.frames_remaining, 0, "tick must not underflow");
    }

    #[test]
    fn match_clock_starts_in_regulation() {
        let clock = MatchClock::default();

        assert_eq!(clock.phase, MatchPhase::Regulation);
        assert!(!clock.is_sudden_death());
    }

    #[test]
    fn entering_sudden_death_refills_the_overtime_budget() {
        let mut clock = MatchClock {
            frames_remaining: 0,
            phase: MatchPhase::Regulation,
        };

        clock.enter_sudden_death();

        assert!(clock.is_sudden_death());
        assert_eq!(clock.frames_remaining, SUDDEN_DEATH_TIME_LIMIT_FRAMES);
        assert!(!clock.is_expired());
    }

    #[test]
    fn time_limit_winner_is_the_capture_leader() {
        let winner = time_limit_winner(
            CaptureScore {
                player: 2,
                opponents: 1,
            },
            FlagStealScore::default(),
            FlagReturnScore::default(),
        );

        assert_eq!(winner, CtfMatchWinner::Player);
    }

    #[test]
    fn time_limit_winner_breaks_capture_ties_on_steals_then_returns() {
        let steal_leader = time_limit_winner(
            CaptureScore {
                player: 1,
                opponents: 1,
            },
            FlagStealScore {
                player: 0,
                opponents: 3,
            },
            FlagReturnScore::default(),
        );
        assert_eq!(steal_leader, CtfMatchWinner::Opponents);

        let return_leader = time_limit_winner(
            CaptureScore {
                player: 1,
                opponents: 1,
            },
            FlagStealScore {
                player: 2,
                opponents: 2,
            },
            FlagReturnScore {
                player: 4,
                opponents: 1,
            },
        );
        assert_eq!(return_leader, CtfMatchWinner::Player);
    }

    #[test]
    fn time_limit_winner_is_a_draw_when_every_tally_is_level() {
        let winner = time_limit_winner(
            CaptureScore {
                player: 1,
                opponents: 1,
            },
            FlagStealScore {
                player: 2,
                opponents: 2,
            },
            FlagReturnScore {
                player: 3,
                opponents: 3,
            },
        );

        assert_eq!(winner, CtfMatchWinner::Draw);
    }

    #[test]
    fn level_overtime_goes_to_the_team_that_wrecked_more() {
        assert_eq!(
            break_level_overtime_by_wrecks(4, 2),
            CtfMatchWinner::Player,
            "the team that did more damage takes the deadlock"
        );
        assert_eq!(
            break_level_overtime_by_wrecks(1, 5),
            CtfMatchWinner::Opponents,
            "the more aggressive opponents take the deadlock"
        );
    }

    #[test]
    fn level_overtime_stays_a_draw_when_wrecks_are_also_level() {
        assert_eq!(
            break_level_overtime_by_wrecks(3, 3),
            CtfMatchWinner::Draw,
            "a match level on objectives and damage is a true draw"
        );
        assert_eq!(
            break_level_overtime_by_wrecks(0, 0),
            CtfMatchWinner::Draw,
            "a passive deadlock with no wrecks stays a draw"
        );
    }

    fn app_with_clock(frames_remaining: u32) -> App {
        app_with_phased_clock(frames_remaining, MatchPhase::Regulation)
    }

    fn app_with_phased_clock(frames_remaining: u32, phase: MatchPhase) -> App {
        let mut app = App::new();
        app.init_resource::<CaptureScore>();
        app.init_resource::<FlagStealScore>();
        app.init_resource::<FlagReturnScore>();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<CtfMatchResult>();
        app.insert_resource(MatchClock {
            frames_remaining,
            phase,
        });
        app.add_system(expire_match_on_time_limit);
        app
    }

    #[test]
    fn expiring_clock_ends_match_for_the_capture_leader() {
        let mut app = app_with_clock(1);
        app.insert_resource(CaptureScore {
            player: 2,
            opponents: 0,
        });

        app.update();

        assert!(app.world.resource::<MatchClock>().is_expired());
        assert_eq!(
            app.world.resource::<CtfMatchResult>().winner,
            Some(CtfMatchWinner::Player)
        );
    }

    #[test]
    fn expiring_regulation_clock_with_level_scores_enters_sudden_death() {
        let mut app = app_with_clock(1);

        app.update();

        let clock = *app.world.resource::<MatchClock>();
        assert!(clock.is_sudden_death(), "a level round must go to overtime");
        assert_eq!(clock.frames_remaining, SUDDEN_DEATH_TIME_LIMIT_FRAMES);
        assert_eq!(
            app.world.resource::<CtfMatchResult>().winner,
            None,
            "overtime must not resolve immediately"
        );
    }

    #[test]
    fn sudden_death_does_not_re_enter_itself() {
        let mut app = app_with_phased_clock(1, MatchPhase::SuddenDeath);

        app.update();

        let clock = *app.world.resource::<MatchClock>();
        assert!(clock.is_expired(), "overtime must run down, not refill");
        assert_eq!(
            app.world.resource::<CtfMatchResult>().winner,
            Some(CtfMatchWinner::Draw),
            "an overtime level on objectives and damage is the final fallback to a draw"
        );
    }

    #[test]
    fn level_sudden_death_is_decided_by_the_heavier_wrecker() {
        let mut app = app_with_phased_clock(1, MatchPhase::SuddenDeath);
        app.insert_resource(Score {
            wrecks: 3,
            ..Score::default()
        });
        app.insert_resource(OpponentScore {
            wrecks: 1,
            ..OpponentScore::default()
        });

        app.update();

        assert_eq!(
            app.world.resource::<CtfMatchResult>().winner,
            Some(CtfMatchWinner::Player),
            "a deadlocked overtime goes to the team that wrecked more enemies"
        );
    }

    #[test]
    fn objective_lead_still_wins_overtime_regardless_of_wrecks() {
        let mut app = app_with_phased_clock(1, MatchPhase::SuddenDeath);
        app.insert_resource(CaptureScore {
            player: 1,
            opponents: 0,
        });
        app.insert_resource(Score {
            wrecks: 0,
            ..Score::default()
        });
        app.insert_resource(OpponentScore {
            wrecks: 9,
            ..OpponentScore::default()
        });

        app.update();

        assert_eq!(
            app.world.resource::<CtfMatchResult>().winner,
            Some(CtfMatchWinner::Player),
            "the wreck tie-break must never override a genuine objective lead"
        );
    }

    #[test]
    fn expiring_sudden_death_clock_resolves_on_running_tallies() {
        let mut app = app_with_phased_clock(1, MatchPhase::SuddenDeath);
        app.insert_resource(FlagStealScore {
            player: 0,
            opponents: 1,
        });

        app.update();

        assert_eq!(
            app.world.resource::<CtfMatchResult>().winner,
            Some(CtfMatchWinner::Opponents),
            "a steal earned in overtime breaks the tie"
        );
    }

    #[test]
    fn running_clock_keeps_the_match_open() {
        let mut app = app_with_clock(5);

        app.update();

        assert_eq!(app.world.resource::<MatchClock>().frames_remaining, 4);
        assert_eq!(app.world.resource::<CtfMatchResult>().winner, None);
    }

    #[test]
    fn expired_clock_never_overrides_a_decided_winner() {
        let mut app = app_with_clock(1);
        app.insert_resource(CtfMatchResult {
            winner: Some(CtfMatchWinner::Player),
        });
        app.insert_resource(CaptureScore {
            player: 0,
            opponents: 3,
        });

        app.update();

        assert_eq!(
            app.world.resource::<CtfMatchResult>().winner,
            Some(CtfMatchWinner::Player),
            "a clinched win must not be rewritten by the timer"
        );
        assert_eq!(
            app.world.resource::<MatchClock>().frames_remaining,
            1,
            "a finished match must not keep burning clock"
        );
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
        let mut steals = FlagStealScore::default();
        let mut returns = FlagReturnScore::default();
        advance_capture_the_flag(
            flags,
            collectors,
            score,
            &mut steals,
            &mut returns,
            &mut result,
        );
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
    fn flag_steal_bounty_rewards_only_new_steals() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        award_flag_steal_bounties(
            FlagStealScore {
                player: 1,
                opponents: 0,
            },
            FlagStealScore {
                player: 2,
                opponents: 1,
            },
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(player_economy.cash, FLAG_STEAL_CASH_BOUNTY);
        assert_eq!(player_economy.steals, 1);
        assert_eq!(opponent_economy.cash, FLAG_STEAL_CASH_BOUNTY);
        assert_eq!(opponent_economy.steals, 1);
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
    fn player_flag_steal_triggers_team_nitro_momentum() {
        let mut nitro_boosts = NitroBoosts::default();

        award_flag_steal_momentum_boosts(
            FlagStealScore {
                player: 0,
                opponents: 0,
            },
            FlagStealScore {
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
    fn opponent_flag_steal_triggers_team_nitro_momentum() {
        let mut nitro_boosts = NitroBoosts::default();

        award_flag_steal_momentum_boosts(
            FlagStealScore {
                player: 0,
                opponents: 0,
            },
            FlagStealScore {
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
    fn player_flag_return_triggers_team_nitro_momentum() {
        let mut nitro_boosts = NitroBoosts::default();

        award_flag_return_momentum_boosts(
            FlagReturnScore {
                player: 0,
                opponents: 0,
            },
            FlagReturnScore {
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
    fn opponent_flag_return_triggers_team_nitro_momentum() {
        let mut nitro_boosts = NitroBoosts::default();

        award_flag_return_momentum_boosts(
            FlagReturnScore {
                player: 0,
                opponents: 0,
            },
            FlagReturnScore {
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
    fn player_steals_red_flag_for_steal_score() {
        let mut flags = vec![
            flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0)),
            flag(11, FlagTeam::Red, Vec2::new(50.0, 0.0)),
        ];
        let collector = blue_collector(Vec2::ZERO);
        let mut score = CaptureScore::default();
        let mut steals = FlagStealScore::default();
        let mut returns = FlagReturnScore::default();
        let mut result = CtfMatchResult::default();

        advance_capture_the_flag(
            &mut flags,
            &[collector],
            &mut score,
            &mut steals,
            &mut returns,
            &mut result,
        );

        assert_eq!(
            steals,
            FlagStealScore {
                player: 1,
                opponents: 0,
            }
        );
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
        let mut steals = FlagStealScore::default();
        let mut returns = FlagReturnScore::default();
        let mut result = CtfMatchResult::default();

        advance_capture_the_flag(
            &mut flags,
            &[collector],
            &mut score,
            &mut steals,
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
        let mut steals = FlagStealScore::default();
        let mut returns = FlagReturnScore::default();
        let mut result = CtfMatchResult {
            winner: Some(CtfMatchWinner::Player),
        };

        advance_capture_the_flag(
            &mut flags,
            &[collector],
            &mut score,
            &mut steals,
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
        let mut steals = FlagStealScore::default();
        let mut returns = FlagReturnScore::default();
        let mut result = CtfMatchResult::default();

        advance_capture_the_flag(
            &mut flags,
            &[blue, teammate],
            &mut score,
            &mut steals,
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
        app.init_resource::<FlagStealScore>();
        app.init_resource::<FlagReturnScore>();
        app.init_resource::<CtfMatchResult>();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<NitroBoosts>();
        app.init_resource::<MatchClock>();
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
        app.init_resource::<FlagStealScore>();
        app.init_resource::<FlagReturnScore>();
        app.init_resource::<CtfMatchResult>();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<NitroBoosts>();
        app.init_resource::<MatchClock>();
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
        assert_eq!(
            app.world.resource::<NitroBoosts>().player_frames,
            crate::gameplay::pickup::NITRO_BOOST_FRAMES
        );
        assert_eq!(app.world.resource::<NitroBoosts>().opponent_frames, 0);
    }

    #[test]
    fn system_uses_virtual_player_team_for_enemy_flags() {
        let mut app = App::new();
        app.init_resource::<CaptureScore>();
        app.init_resource::<FlagStealScore>();
        app.init_resource::<FlagReturnScore>();
        app.init_resource::<CtfMatchResult>();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<NitroBoosts>();
        app.init_resource::<MatchClock>();
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
                Transform::from_translation(Vec3::new(-500.0, 0.0, 2.0)),
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
        assert_eq!(
            *app.world.resource::<FlagStealScore>(),
            FlagStealScore {
                player: 1,
                opponents: 0,
            }
        );
        assert_eq!(app.world.resource::<Score>().cash, FLAG_STEAL_CASH_BOUNTY);
        assert_eq!(
            app.world.resource::<NitroBoosts>().player_frames,
            crate::gameplay::pickup::NITRO_BOOST_FRAMES
        );
        assert_eq!(app.world.resource::<NitroBoosts>().opponent_frames, 0);
    }

    #[test]
    fn blue_virtual_player_capture_scores_for_player_team() {
        let mut app = App::new();
        app.init_resource::<CaptureScore>();
        app.init_resource::<FlagStealScore>();
        app.init_resource::<FlagReturnScore>();
        app.init_resource::<CtfMatchResult>();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<NitroBoosts>();
        app.init_resource::<MatchClock>();
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
        app.init_resource::<FlagStealScore>();
        app.init_resource::<FlagReturnScore>();
        app.init_resource::<CtfMatchResult>();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<NitroBoosts>();
        app.init_resource::<MatchClock>();
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

    #[test]
    fn golden_goal_ends_overtime_on_a_player_capture() {
        let mut result = CtfMatchResult::default();

        award_golden_goal(
            true,
            CaptureScore {
                player: 1,
                opponents: 1,
            },
            CaptureScore {
                player: 2,
                opponents: 1,
            },
            &mut result,
        );

        assert_eq!(result.winner, Some(CtfMatchWinner::Player));
    }

    #[test]
    fn golden_goal_ends_overtime_on_an_opponent_capture() {
        let mut result = CtfMatchResult::default();

        award_golden_goal(
            true,
            CaptureScore::default(),
            CaptureScore {
                player: 0,
                opponents: 1,
            },
            &mut result,
        );

        assert_eq!(result.winner, Some(CtfMatchWinner::Opponents));
    }

    #[test]
    fn golden_goal_wins_below_the_regulation_capture_threshold() {
        // A single overtime capture decides it though it is far from
        // CAPTURES_TO_WIN, which a lone regulation capture never would be.
        let mut result = CtfMatchResult::default();

        award_golden_goal(
            true,
            CaptureScore::default(),
            CaptureScore {
                player: 1,
                opponents: 0,
            },
            &mut result,
        );

        assert_eq!(result.winner, Some(CtfMatchWinner::Player));
    }

    #[test]
    fn golden_goal_is_inert_during_regulation() {
        let mut result = CtfMatchResult::default();

        award_golden_goal(
            false,
            CaptureScore::default(),
            CaptureScore {
                player: 1,
                opponents: 0,
            },
            &mut result,
        );

        assert_eq!(
            result.winner, None,
            "in regulation a lone capture only banks a point"
        );
    }

    #[test]
    fn golden_goal_ignores_frames_without_a_new_capture() {
        let mut result = CtfMatchResult::default();

        award_golden_goal(
            true,
            CaptureScore {
                player: 2,
                opponents: 2,
            },
            CaptureScore {
                player: 2,
                opponents: 2,
            },
            &mut result,
        );

        assert_eq!(result.winner, None);
    }

    #[test]
    fn golden_goal_never_overrides_a_decided_winner() {
        let mut result = CtfMatchResult {
            winner: Some(CtfMatchWinner::Opponents),
        };

        award_golden_goal(
            true,
            CaptureScore::default(),
            CaptureScore {
                player: 1,
                opponents: 0,
            },
            &mut result,
        );

        assert_eq!(result.winner, Some(CtfMatchWinner::Opponents));
    }

    #[test]
    fn system_awards_golden_goal_capture_in_sudden_death() {
        let mut app = App::new();
        app.insert_resource(CaptureScore {
            player: 1,
            opponents: 1,
        });
        app.init_resource::<FlagStealScore>();
        app.init_resource::<FlagReturnScore>();
        app.init_resource::<CtfMatchResult>();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<NitroBoosts>();
        app.insert_resource(MatchClock {
            frames_remaining: SUDDEN_DEATH_TIME_LIMIT_FRAMES,
            phase: MatchPhase::SuddenDeath,
        });
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
            app.world.resource::<CaptureScore>().player,
            2,
            "the overtime capture still tallies"
        );
        assert!(
            app.world.resource::<CaptureScore>().player < CAPTURES_TO_WIN,
            "the golden goal wins short of a regulation clinch"
        );
        assert_eq!(
            app.world.resource::<CtfMatchResult>().winner,
            Some(CtfMatchWinner::Player),
            "the first overtime capture wins outright"
        );
    }

    #[test]
    fn player_win_banks_the_victory_purse_for_the_player_economy() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        award_match_purse(
            CtfMatchWinner::Player,
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(player_economy.cash, VICTORY_CASH_PURSE);
        assert_eq!(opponent_economy.cash, 0);
    }

    #[test]
    fn opponents_win_banks_the_victory_purse_for_the_opponent_economy() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        award_match_purse(
            CtfMatchWinner::Opponents,
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(opponent_economy.cash, VICTORY_CASH_PURSE);
        assert_eq!(player_economy.cash, 0);
    }

    #[test]
    fn a_draw_splits_a_smaller_purse_to_both_teams() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        award_match_purse(
            CtfMatchWinner::Draw,
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(player_economy.cash, DRAW_CASH_PURSE);
        assert_eq!(opponent_economy.cash, DRAW_CASH_PURSE);
    }

    fn purse_app() -> App {
        let mut app = App::new();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<CtfMatchResult>();
        app.init_resource::<MatchPursePaid>();
        app.add_system(award_match_purse_on_resolution);
        app
    }

    #[test]
    fn unresolved_match_banks_no_purse() {
        let mut app = purse_app();

        app.update();

        assert_eq!(app.world.resource::<Score>().cash, 0);
        assert_eq!(app.world.resource::<OpponentScore>().cash, 0);
        assert!(!app.world.resource::<MatchPursePaid>().0);
    }

    #[test]
    fn resolved_match_banks_the_purse_and_latches_it() {
        let mut app = purse_app();
        app.insert_resource(CtfMatchResult {
            winner: Some(CtfMatchWinner::Player),
        });

        app.update();

        assert_eq!(app.world.resource::<Score>().cash, VICTORY_CASH_PURSE);
        assert!(
            app.world.resource::<MatchPursePaid>().0,
            "banking the purse must latch the flag"
        );
    }

    #[test]
    fn a_resolved_match_pays_the_purse_only_once() {
        let mut app = purse_app();
        app.insert_resource(CtfMatchResult {
            winner: Some(CtfMatchWinner::Opponents),
        });

        app.update();
        app.update();
        app.update();

        assert_eq!(
            app.world.resource::<OpponentScore>().cash,
            VICTORY_CASH_PURSE,
            "the frozen post-match frames must not keep re-banking the purse"
        );
    }
}
