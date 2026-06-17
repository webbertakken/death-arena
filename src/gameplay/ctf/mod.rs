use crate::gameplay::pickup::{NitroBoosts, OpponentScore, Score};
use crate::gameplay::player::Player;
use crate::gameplay::virtual_player::ai::AiTeam;
use crate::gameplay::virtual_player::VirtualPlayer;
use crate::{App, AppState, Plugin};
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;

mod clock;
pub use clock::*;

mod timers;
pub use timers::*;

mod economy;
use economy::{
    award_capture_bounties, award_capture_momentum_boosts, award_flag_return_bounties,
    award_flag_return_momentum_boosts, award_flag_steal_bounties, award_flag_steal_momentum_boosts,
};

mod purse;

mod flag;
use flag::{advance_capture_the_flag, CollectorKind, CollectorState, FlagState};

pub const FLAG_TOUCH_RADIUS: f32 = 120.0;
pub const BASE_CAPTURE_RADIUS: f32 = 160.0;
pub const CAPTURES_TO_WIN: u32 = 3;
pub const CAPTURE_CASH_BOUNTY: u32 = 250;
pub const FLAG_STEAL_CASH_BOUNTY: u32 = 50;
pub const FLAG_RETURN_CASH_BOUNTY: u32 = 75;
/// Cash a team behind on captures banks per capture of deficit it claws back
/// from, on top of the flat [`CAPTURE_CASH_BOUNTY`].
///
/// The capture-the-flag mirror of the combat most-wanted leader bounty
/// ([`crate::gameplay::combat::most_wanted_wreck_bonus`]): where that pays the
/// trailing side a comeback bonus for wrecking a capture leader's car, this pays
/// it for answering on the objective itself. Both are anti-snowball levers keyed
/// on the capture standing that point the opposite way to a runaway lead, and the
/// speed-side catch-up ([`crate::gameplay::comeback`]) is the third. Priced per
/// capture of deficit so a bigger comeback pays more, and pinned (see the
/// compile-assert below) so even the largest comeback never out-earns taking a
/// flag.
pub const COMEBACK_CAPTURE_BONUS_PER_DEFICIT: u32 = 100;
/// A comeback bonus must be a real payday, not a token, enforced at compile time.
const _: () = assert!(COMEBACK_CAPTURE_BONUS_PER_DEFICIT > 0);
/// Clawing the gap shut must never out-earn taking a flag: the largest comeback
/// bonus (from the deepest live deficit, [`CAPTURES_TO_WIN`] `- 1`) stays below a
/// capture's own bounty, enforced at compile time. Holds while a round needs more
/// than one capture to win.
const _: () = assert!(CAPTURES_TO_WIN > 1);
const _: () =
    assert!(COMEBACK_CAPTURE_BONUS_PER_DEFICIT * (CAPTURES_TO_WIN - 1) < CAPTURE_CASH_BOUNTY);
/// Cash a team behind on captures banks per capture of deficit for a flag steal or
/// return it lands while trailing, on top of the flat [`FLAG_STEAL_CASH_BOUNTY`] or
/// [`FLAG_RETURN_CASH_BOUNTY`].
///
/// The intermediate-objective companion to [`COMEBACK_CAPTURE_BONUS_PER_DEFICIT`]:
/// where that pays a trailing side for answering on the capture itself, this pays it
/// for the steps that lead there, lifting the enemy flag off its base and clawing its
/// own flag back, so a behind team's fightback is funded even on the frames it makes
/// ground without yet completing a capture. Like the capture comeback it is an
/// anti-snowball lever keyed on the capture standing, the same axis the most-wanted
/// wreck bounty and the speed-side catch-up ([`crate::gameplay::comeback`]) answer.
/// Priced per capture of deficit so a deeper hole pays more, and pinned (see the
/// compile asserts below) below the capture comeback per deficit and far enough
/// below a capture's own bounty that a comeback steal or return can never out-earn
/// taking a flag.
pub const COMEBACK_FLAG_EVENT_BONUS_PER_DEFICIT: u32 = 50;
/// A comeback steal/return bonus must be a real payday, not a token, enforced at
/// compile time.
const _: () = assert!(COMEBACK_FLAG_EVENT_BONUS_PER_DEFICIT > 0);
/// Answering on the marquee capture must out-reward an intermediate steal or return
/// per capture of deficit, enforced at compile time, so the comeback economy still
/// points hardest at the objective that takes the round.
const _: () = assert!(COMEBACK_FLAG_EVENT_BONUS_PER_DEFICIT < COMEBACK_CAPTURE_BONUS_PER_DEFICIT);
/// A comeback steal, even from the deepest live deficit, must never out-earn taking
/// a flag, enforced at compile time, so the intermediate reward never eclipses the
/// objective it builds toward.
const _: () = assert!(
    FLAG_STEAL_CASH_BOUNTY + COMEBACK_FLAG_EVENT_BONUS_PER_DEFICIT * (CAPTURES_TO_WIN - 1)
        < CAPTURE_CASH_BOUNTY
);
/// A comeback return, even from the deepest live deficit, must never out-earn taking
/// a flag, enforced at compile time.
const _: () = assert!(
    FLAG_RETURN_CASH_BOUNTY + COMEBACK_FLAG_EVENT_BONUS_PER_DEFICIT * (CAPTURES_TO_WIN - 1)
        < CAPTURE_CASH_BOUNTY
);
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
/// Cash a decisive winner banks on top of [`VICTORY_CASH_PURSE`] for a clean
/// sheet: taking the round without conceding a single capture.
///
/// The Death Rally reward for an airtight round. A flat bonus paid only on a
/// decisive win where the beaten team never captured, so it prizes watertight
/// CTF defence on top of taking the round. Pitched well below the victory purse
/// so the clean sheet enriches a win without ever out-paying taking the round
/// itself, and a level draw never earns it however stingy the defence.
pub const CLEAN_SHEET_CASH_BONUS: u32 = 500;
/// A clean sheet must enrich a win, never out-pay taking the round itself,
/// enforced at compile time.
const _: () = assert!(CLEAN_SHEET_CASH_BONUS < VICTORY_CASH_PURSE);
/// A clean-sheet bonus must be a real payday, not a token, enforced at compile
/// time.
const _: () = assert!(CLEAN_SHEET_CASH_BONUS > 0);
/// Cash a decisive winner banks on top of [`VICTORY_CASH_PURSE`] for a
/// nail-biter: taking the round while the beaten team sat on match point, one
/// capture short of [`CAPTURES_TO_WIN`].
///
/// The clutch counterpart to [`CLEAN_SHEET_CASH_BONUS`]: where the clean sheet
/// prizes airtight dominance (the loser never captured), the nail-biter prizes
/// surviving a round that ran down to the wire (the loser was a single capture
/// from the title when the winner clinched). Because a capture ends the round,
/// a beaten team left on [`CAPTURES_TO_WIN`] `- 1` genuinely had its decider
/// denied, so the bonus always marks a real cliff-hanger. Pitched below the
/// clean sheet so watertight defence still out-earns a last-gasp scrape, and
/// like it kept well below the victory purse so the bonus enriches a win
/// without ever out-paying taking the round; a level draw never earns it.
pub const NAIL_BITER_CASH_BONUS: u32 = 250;
/// A nail-biter bonus must be a real payday, not a token, enforced at compile
/// time.
const _: () = assert!(NAIL_BITER_CASH_BONUS > 0);
/// A nail-biter must enrich a win, never out-pay taking the round itself,
/// enforced at compile time.
const _: () = assert!(NAIL_BITER_CASH_BONUS < VICTORY_CASH_PURSE);
/// Airtight dominance must out-earn a last-gasp scrape, enforced at compile
/// time.
const _: () = assert!(NAIL_BITER_CASH_BONUS < CLEAN_SHEET_CASH_BONUS);
/// The clean sheet (beaten team on zero) and the nail-biter (beaten team on
/// [`CAPTURES_TO_WIN`] `- 1`) are disjoint conditions, so a single win never
/// banks both. Holds as long as a round needs more than one capture to win,
/// enforced at compile time.
const _: () = assert!(CAPTURES_TO_WIN > 1);
/// Cash a decisive winner banks on top of [`VICTORY_CASH_PURSE`] for a golden
/// goal: clinching the round with a capture in sudden-death overtime.
///
/// The marquee Death Rally finish. Where [`CLEAN_SHEET_CASH_BONUS`] and
/// [`NAIL_BITER_CASH_BONUS`] price the *scoreline* a win was taken on, this
/// prices the *way it was clinched*: a regulation deadlock carried into a
/// golden-goal decider and settled by the next capture. The finish mode is an
/// axis orthogonal to the scoreline, so a golden goal stacks on top of either
/// scoreline bonus (a 0-0 overtime won 1-0 is both a clean sheet and a golden
/// goal). Pitched above the nail-biter (winning the decider outright is more
/// than denying the enemy theirs) yet below the clean sheet so airtight
/// dominance stays the top single win-quality bonus.
pub const GOLDEN_GOAL_CASH_BONUS: u32 = 350;
/// A golden-goal bonus must be a real payday, not a token, enforced at compile
/// time.
const _: () = assert!(GOLDEN_GOAL_CASH_BONUS > 0);
/// Winning the decider outright must edge out merely denying the enemy theirs,
/// enforced at compile time.
const _: () = assert!(GOLDEN_GOAL_CASH_BONUS > NAIL_BITER_CASH_BONUS);
/// Airtight dominance must stay the top single win-quality bonus, enforced at
/// compile time.
const _: () = assert!(GOLDEN_GOAL_CASH_BONUS < CLEAN_SHEET_CASH_BONUS);
/// A golden goal stacks on a scoreline bonus, so even its largest stack (with
/// the clean sheet, the dearer of the two disjoint scoreline bonuses) must still
/// enrich a win without ever out-paying taking the round itself, enforced at
/// compile time.
const _: () = assert!(GOLDEN_GOAL_CASH_BONUS + CLEAN_SHEET_CASH_BONUS < VICTORY_CASH_PURSE);
/// Cash a decisive winner banks on top of [`VICTORY_CASH_PURSE`] for a demolition
/// decider: settling a sudden-death overtime that ran level on every objective by
/// the wreck tiebreak.
///
/// The combat-flavoured counterpart to [`GOLDEN_GOAL_CASH_BONUS`]. Where the golden
/// goal prices the overtime *capture* that wins the decider outright, this prices
/// the other dramatic overtime finish: a regulation deadlock carried into overtime,
/// still level on captures, steals and returns when the overtime clock expires, and
/// then settled by [`break_level_overtime_by_wrecks`] in favour of the heavier
/// wrecker. The quintessential Death Rally finish in a car-combat game: a side that
/// could not out-score the enemy out-wrecks it instead.
///
/// Like the golden goal it keys on the finish mode, not the final tally, so it
/// stacks on whichever scoreline bonus the level overtime leaves: a 0-0 demolition
/// decider is also a clean sheet, a 2-2 one a nail-biter. It is mutually exclusive
/// with the golden goal (a golden goal clinches *before* the overtime clock expires,
/// a demolition decider only *once* it has) and with the cash tiebreak (that settles
/// only an overtime level on wrecks too, which a demolition decider never is).
/// Pitched above the nail-biter (out-wrecking the decider is a more active feat than
/// merely denying the enemy match point) yet below the golden goal (a scored decider
/// outranks a wreck-tiebreak one), and kept clear below the victory purse so it
/// enriches a win without ever out-paying taking the round; a level draw never earns
/// it.
pub const DEMOLITION_DECIDER_CASH_BONUS: u32 = 300;
/// A demolition-decider bonus must be a real payday, not a token, enforced at
/// compile time.
const _: () = assert!(DEMOLITION_DECIDER_CASH_BONUS > 0);
/// Out-wrecking a deadlocked decider must edge out merely denying the enemy match
/// point, enforced at compile time.
const _: () = assert!(DEMOLITION_DECIDER_CASH_BONUS > NAIL_BITER_CASH_BONUS);
/// A scored golden-goal decider must outrank a wreck-tiebreak one, enforced at
/// compile time.
const _: () = assert!(DEMOLITION_DECIDER_CASH_BONUS < GOLDEN_GOAL_CASH_BONUS);
/// A demolition decider stacks on a scoreline bonus, so even its largest stack
/// (with the clean sheet, the dearer of the two disjoint scoreline bonuses) must
/// still enrich a win without ever out-paying taking the round itself, enforced at
/// compile time.
const _: () = assert!(DEMOLITION_DECIDER_CASH_BONUS + CLEAN_SHEET_CASH_BONUS < VICTORY_CASH_PURSE);
/// Cash a decisive winner banks on top of [`VICTORY_CASH_PURSE`] for a treasury
/// decider: settling a sudden-death overtime that ran level on captures, steals,
/// returns *and* wrecks by the cash tiebreak, money the final Death Rally arbiter.
///
/// The money-talks counterpart to [`GOLDEN_GOAL_CASH_BONUS`] and
/// [`DEMOLITION_DECIDER_CASH_BONUS`], the third and last of the overtime finish
/// bonuses. Where the golden goal prices the overtime *capture* and the demolition
/// decider the *wreck* tiebreak, this prices the final resort: a deadlock neither the
/// objective nor raw aggression could break, carried down to whichever side ran the
/// richer campaign and settled by [`break_level_overtime_by_cash`]. In Death Rally
/// money is the whole point, so the wealthier side edging a standstill nothing else
/// could is a finish worth a payday of its own.
///
/// Like the other two it keys on the finish mode, not the final tally, so it stacks
/// on whichever scoreline bonus the level overtime leaves: a 0-0 treasury decider is
/// also a clean sheet, a 2-2 one a nail-biter. It is mutually exclusive with both
/// other finish bonuses (a golden goal clinches before the overtime clock expires, a
/// demolition decider settles an overtime the wreck tiebreak could break, which a
/// treasury decider by definition could not), so a single win still banks at most one
/// finish-mode bonus. Pitched below the demolition decider (winning on the bankroll is
/// a more passive feat than out-wrecking the decider) and below the nail-biter too,
/// the smallest win-quality bonus of all, yet kept clear below the victory purse so it
/// enriches a win without ever out-paying taking the round; a level draw never earns
/// it.
pub const TREASURY_DECIDER_CASH_BONUS: u32 = 200;
/// A treasury-decider bonus must be a real payday, not a token, enforced at compile
/// time.
const _: () = assert!(TREASURY_DECIDER_CASH_BONUS > 0);
/// Out-wrecking a deadlocked decider must edge out winning it on the bankroll,
/// enforced at compile time, so a more active overtime finish always out-earns the
/// most passive one.
const _: () = assert!(TREASURY_DECIDER_CASH_BONUS < DEMOLITION_DECIDER_CASH_BONUS);
/// Winning purely on the bankroll, the most passive finish, must be the smallest
/// win-quality bonus, below even denying the enemy match point, enforced at compile
/// time.
const _: () = assert!(TREASURY_DECIDER_CASH_BONUS < NAIL_BITER_CASH_BONUS);
/// A treasury decider stacks on a scoreline bonus, so even its largest stack (with
/// the clean sheet, the dearer of the two disjoint scoreline bonuses) must still
/// enrich a win without ever out-paying taking the round itself, enforced at compile
/// time.
const _: () = assert!(TREASURY_DECIDER_CASH_BONUS + CLEAN_SHEET_CASH_BONUS < VICTORY_CASH_PURSE);
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
/// The three objective tallies the purse system reads to spot a wreck-settled
/// overtime, bundled into one system parameter to keep the resolution system's
/// signature within the argument-count lint (mirrors [`CtfMatchResources`]).
type MatchObjectiveStandings<'w> = (
    Res<'w, CaptureScore>,
    Res<'w, FlagStealScore>,
    Res<'w, FlagReturnScore>,
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

    /// Captures from `team`'s own point of view, paired as `(own, enemy)`.
    ///
    /// The blue flag belongs to the human's side, so [`FlagTeam::Blue`] reads the
    /// player tally as its own; the red flag belongs to the opponents. Lets the
    /// catch-up boost ([`crate::gameplay::comeback`]) read a team's deficit without
    /// knowing which colour the human is.
    #[must_use]
    pub const fn standings(self, team: FlagTeam) -> (u32, u32) {
        match team {
            FlagTeam::Blue => (self.player, self.opponents),
            FlagTeam::Red => (self.opponents, self.player),
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

/// Advances every flag's continuous-carry timer by one fixed-update frame.
///
/// A held flag's carry timer counts up; a flag sitting loose or home clears to
/// zero. Read by both movement systems through
/// [`crate::gameplay::carry_fatigue::carry_fatigue_speed_multiplier`] to tire a
/// carrier that clings to a flag rather than committing it home. Runs after the
/// loose-flag auto-return so a flag sent home this frame clears its carry clock too.
fn advance_flag_carry_timers(flags: &[FlagState], timers: &mut FlagCarryTimers) {
    for flag in flags {
        if flag.holder.is_some() {
            timers.set_for(flag.team, timers.frames_for(flag.team) + 1);
        } else {
            timers.set_for(flag.team, 0);
        }
    }
}

pub fn capture_the_flag_system(
    resources: CtfMatchResources,
    clock: Res<MatchClock>,
    mut flag_query: Query<(Entity, &mut CtfFlag, &mut Transform)>,
    player_query: Query<(Entity, &Transform), HumanPlayerOnly>,
    virtual_player_query: Query<(Entity, &VirtualPlayer, &Transform), VirtualPlayerOnly>,
    mut loose_timers: Option<ResMut<LooseFlagTimers>>,
    mut carry_timers: Option<ResMut<FlagCarryTimers>>,
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
    if let Some(timers) = carry_timers.as_deref_mut() {
        advance_flag_carry_timers(&flags, timers);
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
        *score,
        &mut player_economy,
        &mut opponent_economy,
    );
    award_flag_steal_momentum_boosts(previous_steals, *steals, &mut nitro_boosts);
    award_capture_momentum_boosts(previous_score, *score, &mut nitro_boosts);
    award_flag_return_bounties(
        previous_returns,
        *returns,
        *score,
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
/// level on objectives is then settled by [`break_level_overtime_by_wrecks`], and
/// a match dead even on damage too by [`break_level_overtime_by_cash`], so only a
/// side level on cash as well falls back to [`CtfMatchWinner::Draw`].
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
                    // Level on objectives: the heavier wrecker takes it, and a
                    // match level on damage too falls through to the richer team,
                    // money being the final Death Rally arbiter.
                    match break_level_overtime_by_wrecks(score.wrecks, opponent_score.wrecks) {
                        CtfMatchWinner::Draw => {
                            break_level_overtime_by_cash(score.cash, opponent_score.cash)
                        }
                        decided => decided,
                    }
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

/// Banks the match purse exactly once the round resolves, however it is decided.
///
/// Runs after [`expire_match_on_time_limit`] so a winner settled by a capture,
/// a golden goal, or the clock has all landed before the purse is paid. The
/// [`MatchPursePaid`] latch keeps the payout to a single frame even though the
/// result lingers for the rest of the frozen round.
fn award_match_purse_on_resolution(
    result: Res<CtfMatchResult>,
    standings: MatchObjectiveStandings,
    clock: Res<MatchClock>,
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
    let (captures, steals, returns) = standings;

    // A decisive winner settled while overtime is still running (not yet expired)
    // can only have come from a golden-goal capture; an overtime that ran its
    // clock down is resolved by the timeout path with the clock already expired.
    let clinched_in_overtime = clock.is_sudden_death() && !clock.is_expired();
    // The timeout counterpart to the golden goal: an overtime that ran level on
    // every objective down to its expiring clock and was then settled by the wreck
    // tiebreak (not the cash decider). The expired-clock guard tells it apart from a
    // golden goal, which clinches before the clock runs out.
    let overtime_wreck_decider = clock.is_sudden_death()
        && clock.is_expired()
        && overtime_decided_by_wrecks(
            *captures,
            *steals,
            *returns,
            player_economy.wrecks,
            opponent_economy.wrecks,
        );
    // The last of the overtime finish bonuses: an overtime level on every objective
    // and on wrecks too, run down to its expiring clock and settled by the cash
    // tiebreak. Read from the in-match cash before the purse is banked below, so the
    // tally that decided the overtime is the one priced. The expired-clock guard
    // tells it apart from a golden goal; the wreck-level requirement inside
    // [`overtime_decided_by_cash`] keeps it disjoint from the demolition decider.
    let overtime_cash_decider = clock.is_sudden_death()
        && clock.is_expired()
        && overtime_decided_by_cash(
            *captures,
            *steals,
            *returns,
            player_economy.wrecks,
            opponent_economy.wrecks,
            player_economy.cash,
            opponent_economy.cash,
        );
    purse::award_match_purse(
        winner,
        *captures,
        clinched_in_overtime,
        overtime_wreck_decider,
        overtime_cash_decider,
        &mut player_economy,
        &mut opponent_economy,
    );
    paid.0 = true;
    info!("Match purse banked for {winner:?}");
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
            .init_resource::<FlagCarryTimers>()
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
    timers: (ResMut<LooseFlagTimers>, ResMut<FlagCarryTimers>),
) {
    let (mut loose_timers, mut carry_timers) = timers;
    *captures = CaptureScore::default();
    *steals = FlagStealScore::default();
    *returns = FlagReturnScore::default();
    *result = CtfMatchResult::default();
    *purse_paid = MatchPursePaid::default();
    *clock = MatchClock::default();
    *loose_timers = LooseFlagTimers::default();
    *carry_timers = FlagCarryTimers::default();
}

#[cfg(test)]
mod tests;
