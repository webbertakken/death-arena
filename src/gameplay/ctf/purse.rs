//! The match-purse cash economy: how a resolved round prices the end-of-match
//! purse and the win-quality bonuses that top it.
//!
//! The pure pricing-policy half of the capture-the-flag model, split from the
//! flag, clock and scoring *mechanics* in the parent `ctf` module that drive it,
//! mirroring the wreck pricing already carved into the combat `economy` module.
//! Every function here is a pure,
//! `const`-evaluable rule keyed on a resolved round's facts (who won, the final
//! capture tally, whether it was clinched in sudden-death overtime) together with
//! the purse tuning constants that live in the parent module. Nothing here touches
//! the ECS world; the parent's `award_match_purse_on_resolution` system feeds these
//! results into the per-team [`Score`] and [`OpponentScore`] tallies.

use super::{
    CaptureScore, CtfMatchWinner, CAPTURES_TO_WIN, CLEAN_SHEET_CASH_BONUS, DRAW_CASH_PURSE,
    GOLDEN_GOAL_CASH_BONUS, NAIL_BITER_CASH_BONUS, VICTORY_CASH_PURSE,
};
use crate::gameplay::pickup::{OpponentScore, Score};

/// Cash a decisive winner banks for a clean sheet given the final capture
/// tally: [`CLEAN_SHEET_CASH_BONUS`] if the beaten team never captured, else 0.
///
/// A level [`CtfMatchWinner::Draw`] never earns the bonus: a drawn result is no
/// clean-sheet win however few captures changed hands.
#[must_use]
const fn clean_sheet_bonus(winner: CtfMatchWinner, captures: CaptureScore) -> u32 {
    let clean_sheet = match winner {
        CtfMatchWinner::Player => captures.opponents == 0,
        CtfMatchWinner::Opponents => captures.player == 0,
        CtfMatchWinner::Draw => false,
    };
    if clean_sheet {
        CLEAN_SHEET_CASH_BONUS
    } else {
        0
    }
}

/// Cash a decisive winner banks for a nail-biter given the final capture tally:
/// [`NAIL_BITER_CASH_BONUS`] if the beaten team finished on match point
/// ([`CAPTURES_TO_WIN`] `- 1`), else 0.
///
/// A capture ends the round, so a beaten team left a single capture short
/// genuinely had its decider denied. A level [`CtfMatchWinner::Draw`] never
/// earns the bonus, and the condition is disjoint from [`clean_sheet_bonus`]
/// (beaten team on zero), so a win banks at most one of the two win-quality
/// bonuses.
#[must_use]
const fn nail_biter_bonus(winner: CtfMatchWinner, captures: CaptureScore) -> u32 {
    let nail_biter = match winner {
        CtfMatchWinner::Player => captures.opponents == CAPTURES_TO_WIN - 1,
        CtfMatchWinner::Opponents => captures.player == CAPTURES_TO_WIN - 1,
        CtfMatchWinner::Draw => false,
    };
    if nail_biter {
        NAIL_BITER_CASH_BONUS
    } else {
        0
    }
}

/// Cash a decisive winner banks for a golden goal given whether the round was
/// clinched in sudden-death overtime: [`GOLDEN_GOAL_CASH_BONUS`] if it was, else
/// 0.
///
/// `clinched_in_overtime` is true only when a capture decided a golden-goal
/// decider; an overtime that instead ran its own clock down is settled by the
/// timeout path, not a golden goal, and earns nothing here. A level
/// [`CtfMatchWinner::Draw`] never earns it, since a golden goal always produces a
/// decisive winner. Unlike [`clean_sheet_bonus`] and [`nail_biter_bonus`] this
/// keys on the finish mode rather than the final tally, so it can stack on top
/// of either scoreline bonus.
#[must_use]
const fn golden_goal_bonus(winner: CtfMatchWinner, clinched_in_overtime: bool) -> u32 {
    let golden_goal = clinched_in_overtime
        && matches!(winner, CtfMatchWinner::Player | CtfMatchWinner::Opponents);
    if golden_goal {
        GOLDEN_GOAL_CASH_BONUS
    } else {
        0
    }
}

/// Banks the end-of-match purse to whichever side the result favours.
///
/// A win pays the victor [`VICTORY_CASH_PURSE`], topped by a win-quality bonus:
/// a [`CLEAN_SHEET_CASH_BONUS`] when the beaten team never captured (see
/// [`clean_sheet_bonus`]), or a [`NAIL_BITER_CASH_BONUS`] when it finished on
/// match point (see [`nail_biter_bonus`]). Those two scoreline bonuses are
/// disjoint, so a win banks at most one, and a [`GOLDEN_GOAL_CASH_BONUS`] stacks
/// on top of either when the round was clinched in sudden-death overtime (see
/// [`golden_goal_bonus`]). A draw pays both teams the smaller [`DRAW_CASH_PURSE`]
/// for fighting to a standstill. Pure cash, banked on top of every in-match
/// bounty.
pub(super) const fn award_match_purse(
    winner: CtfMatchWinner,
    captures: CaptureScore,
    clinched_in_overtime: bool,
    player_economy: &mut Score,
    opponent_economy: &mut OpponentScore,
) {
    let win_quality_bonus = clean_sheet_bonus(winner, captures)
        + nail_biter_bonus(winner, captures)
        + golden_goal_bonus(winner, clinched_in_overtime);
    match winner {
        CtfMatchWinner::Player => {
            player_economy.bank_match_purse(VICTORY_CASH_PURSE + win_quality_bonus);
        }
        CtfMatchWinner::Opponents => {
            opponent_economy.bank_match_purse(VICTORY_CASH_PURSE + win_quality_bonus);
        }
        CtfMatchWinner::Draw => {
            player_economy.bank_match_purse(DRAW_CASH_PURSE);
            opponent_economy.bank_match_purse(DRAW_CASH_PURSE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_win_banks_the_victory_purse_for_the_player_economy() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        // A conceded capture means no clean sheet, so just the bare purse.
        award_match_purse(
            CtfMatchWinner::Player,
            CaptureScore {
                player: 3,
                opponents: 1,
            },
            false,
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

        // A conceded capture means no clean sheet, so just the bare purse.
        award_match_purse(
            CtfMatchWinner::Opponents,
            CaptureScore {
                player: 1,
                opponents: 3,
            },
            false,
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
            CaptureScore::default(),
            false,
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(player_economy.cash, DRAW_CASH_PURSE);
        assert_eq!(opponent_economy.cash, DRAW_CASH_PURSE);
    }

    #[test]
    fn a_clean_sheet_win_tops_the_purse_with_the_bonus_for_the_player() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        // The beaten opponents never captured: an airtight clean-sheet win.
        award_match_purse(
            CtfMatchWinner::Player,
            CaptureScore {
                player: 3,
                opponents: 0,
            },
            false,
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(
            player_economy.cash,
            VICTORY_CASH_PURSE + CLEAN_SHEET_CASH_BONUS,
            "holding the enemy to zero captures must top the purse with the clean-sheet bonus"
        );
        assert_eq!(opponent_economy.cash, 0);
    }

    #[test]
    fn a_clean_sheet_win_tops_the_purse_with_the_bonus_for_the_opponents() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        // The beaten player team never captured: an airtight clean-sheet win.
        award_match_purse(
            CtfMatchWinner::Opponents,
            CaptureScore {
                player: 0,
                opponents: 3,
            },
            false,
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(
            opponent_economy.cash,
            VICTORY_CASH_PURSE + CLEAN_SHEET_CASH_BONUS
        );
        assert_eq!(player_economy.cash, 0);
    }

    #[test]
    fn conceding_a_single_capture_forfeits_the_clean_sheet_bonus() {
        assert_eq!(
            clean_sheet_bonus(
                CtfMatchWinner::Player,
                CaptureScore {
                    player: 3,
                    opponents: 1,
                },
            ),
            0,
            "one conceded capture is enough to lose the clean sheet"
        );
    }

    #[test]
    fn a_zero_zero_capture_win_still_counts_as_a_clean_sheet() {
        // A win decided on steals or returns with no capture conceded is still a
        // watertight defensive round on the capture objective.
        assert_eq!(
            clean_sheet_bonus(CtfMatchWinner::Player, CaptureScore::default()),
            CLEAN_SHEET_CASH_BONUS
        );
    }

    #[test]
    fn a_draw_never_earns_a_clean_sheet_bonus() {
        // Even a draw with both teams held to zero captures pays no bonus: a
        // level result is no clean-sheet win.
        assert_eq!(
            clean_sheet_bonus(CtfMatchWinner::Draw, CaptureScore::default()),
            0
        );
    }

    #[test]
    fn a_nail_biter_win_tops_the_purse_with_the_bonus_for_the_player() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        // The beaten opponents finished on match point: a nail-biter win.
        award_match_purse(
            CtfMatchWinner::Player,
            CaptureScore {
                player: CAPTURES_TO_WIN,
                opponents: CAPTURES_TO_WIN - 1,
            },
            false,
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(
            player_economy.cash,
            VICTORY_CASH_PURSE + NAIL_BITER_CASH_BONUS,
            "denying the enemy at match point must top the purse with the nail-biter bonus"
        );
        assert_eq!(opponent_economy.cash, 0);
    }

    #[test]
    fn a_nail_biter_win_tops_the_purse_with_the_bonus_for_the_opponents() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        // The beaten player team finished on match point: a nail-biter win.
        award_match_purse(
            CtfMatchWinner::Opponents,
            CaptureScore {
                player: CAPTURES_TO_WIN - 1,
                opponents: CAPTURES_TO_WIN,
            },
            false,
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(
            opponent_economy.cash,
            VICTORY_CASH_PURSE + NAIL_BITER_CASH_BONUS
        );
        assert_eq!(player_economy.cash, 0);
    }

    #[test]
    fn a_golden_goal_win_tops_the_purse_with_the_bonus_for_the_player() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        // Clinched 2-1 in overtime: the loser sat clear of zero (no clean sheet)
        // and clear of match point (no nail-biter), isolating the golden-goal
        // bonus the overtime finish earns.
        award_match_purse(
            CtfMatchWinner::Player,
            CaptureScore {
                player: 2,
                opponents: 1,
            },
            true,
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(
            player_economy.cash,
            VICTORY_CASH_PURSE + GOLDEN_GOAL_CASH_BONUS,
            "clinching in overtime must top the purse with the golden-goal bonus"
        );
        assert_eq!(opponent_economy.cash, 0);
    }

    #[test]
    fn a_golden_goal_win_tops_the_purse_with_the_bonus_for_the_opponents() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        award_match_purse(
            CtfMatchWinner::Opponents,
            CaptureScore {
                player: 1,
                opponents: 2,
            },
            true,
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(
            opponent_economy.cash,
            VICTORY_CASH_PURSE + GOLDEN_GOAL_CASH_BONUS
        );
        assert_eq!(player_economy.cash, 0);
    }

    #[test]
    fn a_clean_sheet_golden_goal_stacks_both_win_quality_bonuses() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        // A 0-0 deadlock won 1-0 in overtime is both an airtight clean sheet and
        // a golden goal, so the two orthogonal bonuses stack on the purse.
        award_match_purse(
            CtfMatchWinner::Player,
            CaptureScore {
                player: 1,
                opponents: 0,
            },
            true,
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(
            player_economy.cash,
            VICTORY_CASH_PURSE + CLEAN_SHEET_CASH_BONUS + GOLDEN_GOAL_CASH_BONUS,
            "a clean-sheet golden goal must bank both win-quality bonuses"
        );
    }

    #[test]
    fn a_nail_biter_golden_goal_stacks_both_win_quality_bonuses() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        // A 2-2 deadlock won 3-2 in overtime leaves the loser on match point (a
        // nail-biter) and was clinched by the golden goal, so both bonuses stack.
        award_match_purse(
            CtfMatchWinner::Player,
            CaptureScore {
                player: CAPTURES_TO_WIN,
                opponents: CAPTURES_TO_WIN - 1,
            },
            true,
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(
            player_economy.cash,
            VICTORY_CASH_PURSE + NAIL_BITER_CASH_BONUS + GOLDEN_GOAL_CASH_BONUS,
            "a nail-biter golden goal must bank both win-quality bonuses"
        );
    }

    #[test]
    fn finishing_a_single_capture_short_earns_the_nail_biter_bonus() {
        assert_eq!(
            nail_biter_bonus(
                CtfMatchWinner::Player,
                CaptureScore {
                    player: CAPTURES_TO_WIN,
                    opponents: CAPTURES_TO_WIN - 1,
                },
            ),
            NAIL_BITER_CASH_BONUS,
            "leaving the loser on match point is a nail-biter"
        );
    }

    #[test]
    fn winning_with_two_captures_to_spare_forfeits_the_nail_biter_bonus() {
        // The loser finished two captures short, not on match point, so the win
        // is comfortable rather than a nail-biter.
        assert_eq!(
            nail_biter_bonus(
                CtfMatchWinner::Player,
                CaptureScore {
                    player: CAPTURES_TO_WIN,
                    opponents: CAPTURES_TO_WIN - 2,
                },
            ),
            0,
            "a two-capture cushion is no nail-biter"
        );
    }

    #[test]
    fn a_clean_sheet_win_never_also_earns_the_nail_biter_bonus() {
        // The two win-quality bonuses sit at opposite ends of the spectrum: a
        // beaten team on zero captures is the airtight clean sheet, never the
        // match-point nail-biter, so a single win never double-dips.
        let captures = CaptureScore {
            player: CAPTURES_TO_WIN,
            opponents: 0,
        };
        assert_eq!(nail_biter_bonus(CtfMatchWinner::Player, captures), 0);
        assert_eq!(
            clean_sheet_bonus(CtfMatchWinner::Player, captures),
            CLEAN_SHEET_CASH_BONUS
        );
    }

    #[test]
    fn a_draw_never_earns_a_nail_biter_bonus() {
        // A level result is no win, however close the scoreline ran.
        assert_eq!(
            nail_biter_bonus(
                CtfMatchWinner::Draw,
                CaptureScore {
                    player: CAPTURES_TO_WIN - 1,
                    opponents: CAPTURES_TO_WIN - 1,
                },
            ),
            0
        );
    }

    #[test]
    fn an_overtime_clincher_earns_the_golden_goal_bonus() {
        assert_eq!(
            golden_goal_bonus(CtfMatchWinner::Player, true),
            GOLDEN_GOAL_CASH_BONUS,
            "clinching the decider with an overtime capture is a golden goal"
        );
        assert_eq!(
            golden_goal_bonus(CtfMatchWinner::Opponents, true),
            GOLDEN_GOAL_CASH_BONUS
        );
    }

    #[test]
    fn a_win_not_clinched_in_overtime_earns_no_golden_goal_bonus() {
        // A regulation win, or an overtime that ran its own clock down, is no
        // golden goal: the bonus is for the capture that decides the decider.
        assert_eq!(
            golden_goal_bonus(CtfMatchWinner::Player, false),
            0,
            "a win not clinched by an overtime capture is no golden goal"
        );
    }

    #[test]
    fn a_draw_never_earns_a_golden_goal_bonus() {
        // A golden goal always produces a decisive winner, so a level result
        // earns nothing even were the overtime flag somehow set.
        assert_eq!(golden_goal_bonus(CtfMatchWinner::Draw, true), 0);
    }
}
