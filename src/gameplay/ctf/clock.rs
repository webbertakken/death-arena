//! The CTF round clock: the regulation and sudden-death timing that guarantees a
//! match always ends, and the rules that resolve a round the clock runs out on.
//!
//! Split from the flag mechanics and cash economy in the parent `ctf` module: the
//! self-contained match-timing concern, the [`MatchClock`] every per-frame system
//! reads to know how much of the round is left, the [`MatchPhase`] it advances
//! through, and the pure resolvers ([`time_limit_winner`],
//! [`break_level_overtime_by_wrecks`]) that decide a round when the clock expires.
//! Nothing here drives the ECS world beyond holding the clock resource; the CTF
//! systems tick it and feed its verdict into [`super::CtfMatchResult`].

use super::{CaptureScore, CtfMatchWinner, FlagReturnScore, FlagStealScore};
use bevy::prelude::*;
use std::cmp::Ordering;

/// Fixed update frames a CTF round runs before it resolves on time.
///
/// Caps stalemates so a match always ends even if neither team reaches
/// [`crate::gameplay::ctf::CAPTURES_TO_WIN`]. At the game's 60 FPS convention this
/// is three minutes.
pub const MATCH_TIME_LIMIT_FRAMES: u32 = 10_800;
/// Fixed update frames a sudden-death overtime runs before it resolves.
///
/// Entered when regulation expires on a perfectly level scoreline so a tied
/// match gets a dramatic decider instead of a tame draw, while still
/// guaranteeing the round terminates. At 60 FPS this is one minute.
pub const SUDDEN_DEATH_TIME_LIMIT_FRAMES: u32 = 3_600;
/// Fixed update frames from the end of regulation within which the round counts
/// as closing time, the final stretch where a team not ahead on captures stops
/// gambling on opportunistic pickup detours and commits to the objective.
///
/// At the game's 60 FPS convention this is the last thirty seconds of a
/// regulation round. Sudden death is always closing time regardless of this
/// window, since in golden-goal overtime every frame off the objective is wasted.
pub const CLOSING_TIME_FRAMES: u32 = 1_800;
/// Closing time must be a real slice of regulation, not the whole round, enforced
/// at compile time, so a round only tightens into clutch play near the end.
const _: () = assert!(CLOSING_TIME_FRAMES < MATCH_TIME_LIMIT_FRAMES);

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

    /// Whether the round is in its closing stretch: the final
    /// [`CLOSING_TIME_FRAMES`] of regulation, or any moment of sudden death.
    ///
    /// Read by the virtual players to switch a team that is not ahead on captures
    /// into objective-commitment play, the classic clutch-time push.
    pub const fn is_closing_time(self) -> bool {
        self.is_sudden_death() || self.frames_remaining <= CLOSING_TIME_FRAMES
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
pub fn time_limit_winner(
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
/// not. A match also level on wrecks falls through to
/// [`break_level_overtime_by_cash`], the final money-talks arbiter.
#[must_use]
pub const fn break_level_overtime_by_wrecks(
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

/// Breaks an overtime still level on objectives *and* wrecks by the richer team.
///
/// The last word when sudden death expires with captures, steals, returns, and
/// wrecks all dead even: the round goes to whichever side banked more cash. In
/// Death Rally money is the whole point, so the team that ran the more profitable
/// campaign, snapping up more pickups and grinding out more bounties, edges a
/// standstill that neither the objective nor raw aggression could settle. Chained
/// after [`break_level_overtime_by_wrecks`], so it is consulted only once a match
/// is level on every objective and on damage; only a side level on cash too stays
/// a true [`CtfMatchWinner::Draw`], the genuine mirror match.
#[must_use]
pub const fn break_level_overtime_by_cash(player_cash: u32, opponent_cash: u32) -> CtfMatchWinner {
    if player_cash > opponent_cash {
        CtfMatchWinner::Player
    } else if player_cash < opponent_cash {
        CtfMatchWinner::Opponents
    } else {
        CtfMatchWinner::Draw
    }
}

/// Whether a timed-out sudden-death overtime was settled by the wreck tiebreak: the
/// "demolition decider".
///
/// True exactly when the overtime ran level on every objective (so
/// [`time_limit_winner`] returns [`CtfMatchWinner::Draw`] and the round falls through
/// to the damage decider) and the wreck counts were not themselves level (so
/// [`break_level_overtime_by_wrecks`] produces a decisive winner rather than chaining
/// on to the cash decider). The caller pairs this with the clock state (sudden death,
/// expired) to tell a wreck-settled overtime apart from a golden goal, which clinches
/// before the overtime clock runs out. Prices the
/// [`crate::gameplay::ctf::DEMOLITION_DECIDER_CASH_BONUS`] win-quality bonus.
#[must_use]
pub fn overtime_decided_by_wrecks(
    captures: CaptureScore,
    steals: FlagStealScore,
    returns: FlagReturnScore,
    player_wrecks: u32,
    opponent_wrecks: u32,
) -> bool {
    matches!(
        time_limit_winner(captures, steals, returns),
        CtfMatchWinner::Draw
    ) && !matches!(
        break_level_overtime_by_wrecks(player_wrecks, opponent_wrecks),
        CtfMatchWinner::Draw
    )
}

/// Whether a timed-out sudden-death overtime was settled by the cash tiebreak: the
/// "treasury decider".
///
/// True exactly when the overtime ran level on every objective (so
/// [`time_limit_winner`] returns [`CtfMatchWinner::Draw`]) *and* level on wrecks too
/// (so [`break_level_overtime_by_wrecks`] also returns [`CtfMatchWinner::Draw`] and
/// the round falls past the demolition decider), yet the banked cash was not itself
/// level (so [`break_level_overtime_by_cash`] produces a decisive winner rather than
/// a true draw). The caller pairs this with the clock state (sudden death, expired)
/// to tell a cash-settled overtime apart from a golden goal, which clinches before
/// the overtime clock runs out. Its mutual exclusivity with the demolition decider
/// is structural: that fires only when the wrecks are *not* level, this only when
/// they are. Prices the [`crate::gameplay::ctf::TREASURY_DECIDER_CASH_BONUS`]
/// win-quality bonus, the money-talks counterpart to the
/// [`crate::gameplay::ctf::DEMOLITION_DECIDER_CASH_BONUS`].
#[must_use]
pub fn overtime_decided_by_cash(
    captures: CaptureScore,
    steals: FlagStealScore,
    returns: FlagReturnScore,
    player_wrecks: u32,
    opponent_wrecks: u32,
    player_cash: u32,
    opponent_cash: u32,
) -> bool {
    matches!(
        time_limit_winner(captures, steals, returns),
        CtfMatchWinner::Draw
    ) && matches!(
        break_level_overtime_by_wrecks(player_wrecks, opponent_wrecks),
        CtfMatchWinner::Draw
    ) && !matches!(
        break_level_overtime_by_cash(player_cash, opponent_cash),
        CtfMatchWinner::Draw
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn regulation_is_closing_time_only_in_its_final_stretch() {
        let early = MatchClock {
            frames_remaining: CLOSING_TIME_FRAMES + 1,
            phase: MatchPhase::Regulation,
        };
        assert!(
            !early.is_closing_time(),
            "a round with time to spare must not force clutch play"
        );

        let boundary = MatchClock {
            frames_remaining: CLOSING_TIME_FRAMES,
            phase: MatchPhase::Regulation,
        };
        assert!(
            boundary.is_closing_time(),
            "the closing window opens exactly at CLOSING_TIME_FRAMES"
        );

        let expiring = MatchClock {
            frames_remaining: 0,
            phase: MatchPhase::Regulation,
        };
        assert!(expiring.is_closing_time());
    }

    #[test]
    fn sudden_death_is_always_closing_time() {
        let clock = MatchClock {
            // More frames than the regulation closing window, yet still closing
            // because every second of golden-goal overtime is decisive.
            frames_remaining: SUDDEN_DEATH_TIME_LIMIT_FRAMES,
            phase: MatchPhase::SuddenDeath,
        };
        assert!(clock.is_closing_time());
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

    #[test]
    fn level_overtime_goes_to_the_richer_team() {
        assert_eq!(
            break_level_overtime_by_cash(900, 400),
            CtfMatchWinner::Player,
            "the team that banked the richer campaign takes a deadlock level on damage"
        );
        assert_eq!(
            break_level_overtime_by_cash(250, 1_000),
            CtfMatchWinner::Opponents,
            "the wealthier opponents take a deadlock level on damage"
        );
    }

    #[test]
    fn level_overtime_stays_a_draw_when_cash_is_also_level() {
        assert_eq!(
            break_level_overtime_by_cash(750, 750),
            CtfMatchWinner::Draw,
            "a match level on objectives, damage, and cash is a true mirror-match draw"
        );
        assert_eq!(
            break_level_overtime_by_cash(0, 0),
            CtfMatchWinner::Draw,
            "a penniless deadlock stays a draw"
        );
    }

    #[test]
    fn an_objective_deadlock_broken_on_wrecks_is_a_demolition_decider() {
        // Captures, steals and returns all level, but one side wrecked more: the
        // round fell to the wreck tiebreak, the demolition decider.
        assert!(overtime_decided_by_wrecks(
            CaptureScore {
                player: 2,
                opponents: 2,
            },
            FlagStealScore {
                player: 1,
                opponents: 1,
            },
            FlagReturnScore {
                player: 3,
                opponents: 3,
            },
            5,
            2,
        ));
    }

    #[test]
    fn an_overtime_won_on_objectives_is_no_demolition_decider() {
        // A side led on an objective (here steals) when the overtime clock expired,
        // so the round was settled by the objective decider, not the wreck tiebreak.
        assert!(
            !overtime_decided_by_wrecks(
                CaptureScore {
                    player: 1,
                    opponents: 1,
                },
                FlagStealScore {
                    player: 3,
                    opponents: 0,
                },
                FlagReturnScore::default(),
                1,
                4,
            ),
            "an objective-decided overtime is not a demolition decider"
        );
    }

    #[test]
    fn an_overtime_level_on_wrecks_too_is_no_demolition_decider() {
        // Objectives level and wrecks level: the round falls past the wreck tiebreak
        // to the cash decider, so it is no demolition decider.
        assert!(
            !overtime_decided_by_wrecks(
                CaptureScore {
                    player: 1,
                    opponents: 1,
                },
                FlagStealScore::default(),
                FlagReturnScore::default(),
                3,
                3,
            ),
            "an overtime level on wrecks is settled on cash, not by demolition"
        );
    }

    #[test]
    fn an_objective_and_wreck_deadlock_broken_on_cash_is_a_treasury_decider() {
        // Captures, steals, returns and wrecks all level, but one side banked more
        // cash: the round fell past the wreck tiebreak to the money arbiter, the
        // treasury decider.
        assert!(overtime_decided_by_cash(
            CaptureScore {
                player: 2,
                opponents: 2,
            },
            FlagStealScore {
                player: 1,
                opponents: 1,
            },
            FlagReturnScore {
                player: 3,
                opponents: 3,
            },
            4,
            4,
            900,
            350,
        ));
    }

    #[test]
    fn an_overtime_won_on_wrecks_is_no_treasury_decider() {
        // One side wrecked more when the overtime clock expired, so the round was
        // settled by the wreck tiebreak (a demolition decider), never reaching the
        // cash arbiter, however richer the other side ran.
        assert!(
            !overtime_decided_by_cash(
                CaptureScore {
                    player: 1,
                    opponents: 1,
                },
                FlagStealScore::default(),
                FlagReturnScore::default(),
                5,
                2,
                100,
                5_000,
            ),
            "a wreck-decided overtime is settled by demolition, not the cash arbiter"
        );
    }

    #[test]
    fn an_overtime_won_on_objectives_is_no_treasury_decider() {
        // A side led on an objective (here returns) when the overtime clock expired,
        // so the round was settled by the objective decider, never reaching the cash
        // arbiter.
        assert!(
            !overtime_decided_by_cash(
                CaptureScore {
                    player: 1,
                    opponents: 1,
                },
                FlagStealScore::default(),
                FlagReturnScore {
                    player: 4,
                    opponents: 1,
                },
                2,
                2,
                900,
                100,
            ),
            "an objective-decided overtime is not a treasury decider"
        );
    }

    #[test]
    fn an_overtime_level_on_cash_too_is_no_treasury_decider() {
        // Objectives level, wrecks level and cash level: the round stays a true
        // mirror-match draw, so no side won the cash tiebreak and it is no treasury
        // decider.
        assert!(
            !overtime_decided_by_cash(
                CaptureScore {
                    player: 1,
                    opponents: 1,
                },
                FlagStealScore::default(),
                FlagReturnScore::default(),
                3,
                3,
                750,
                750,
            ),
            "a deadlock level on cash too stays a draw, never a treasury decider"
        );
    }
}
