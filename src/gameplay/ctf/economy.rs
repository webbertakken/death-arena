//! The in-match capture-the-flag reward economy: how a frame's objective events
//! price the cash bounty and nitro momentum each team banks.
//!
//! The pure pricing-policy half of the in-match CTF model, split from the flag,
//! clock and scoring *mechanics* in the parent `ctf` module that produce the
//! capture, steal and return tallies this module reads, mirroring the wreck
//! pricing already carved into the combat `economy` module and the end-of-match
//! pricing in the sibling `purse` module. Every function here is a pure,
//! `const`-evaluable rule keyed on a frame's objective facts (the capture, steal
//! and return tallies before and after the frame) together with the CTF tuning
//! constants that live in the parent module. Nothing here touches the ECS world;
//! the parent's `capture_the_flag_system` feeds these results into the per-team
//! [`Score`] and [`OpponentScore`] tallies and the shared [`NitroBoosts`].

use super::{
    CaptureScore, FlagReturnScore, FlagStealScore, CAPTURES_TO_WIN, CAPTURE_CASH_BOUNTY,
    COMEBACK_CAPTURE_BONUS_PER_DEFICIT, FLAG_RETURN_CASH_BOUNTY, FLAG_STEAL_CASH_BOUNTY,
};
use crate::gameplay::pickup::{NitroBoosts, OpponentScore, Score};

pub(super) const fn award_capture_bounties(
    previous: CaptureScore,
    current: CaptureScore,
    player_economy: &mut Score,
    opponent_economy: &mut OpponentScore,
) {
    player_economy.bank_capture_bonus(
        current.player.saturating_sub(previous.player),
        CAPTURE_CASH_BOUNTY,
    );
    player_economy.bank_comeback_capture_bonus(comeback_capture_bonus(
        previous.player,
        previous.opponents,
        current.player,
    ));
    opponent_economy.bank_capture_bonus(
        current.opponents.saturating_sub(previous.opponents),
        CAPTURE_CASH_BOUNTY,
    );
    opponent_economy.bank_comeback_capture_bonus(comeback_capture_bonus(
        previous.opponents,
        previous.player,
        current.opponents,
    ));
}

/// Cash a team banks for clawing a capture back from behind, given its capture
/// tally `previous_own` and the enemy's `previous_enemy` *before* the claw-back,
/// and its tally `current_own` after.
///
/// Pays [`COMEBACK_CAPTURE_BONUS_PER_DEFICIT`] for every capture the team trailed
/// by before answering, capped at the deepest deficit a live match can hold
/// ([`CAPTURES_TO_WIN`] `- 1`, since an enemy reaching [`CAPTURES_TO_WIN`] ends the
/// round). A team that was level or ahead earns nothing, and a frame in which the
/// team did not capture earns nothing.
const fn comeback_capture_bonus(previous_own: u32, previous_enemy: u32, current_own: u32) -> u32 {
    if current_own <= previous_own {
        return 0;
    }
    let deficit = previous_enemy.saturating_sub(previous_own);
    let max_deficit = CAPTURES_TO_WIN - 1;
    let steps = if deficit < max_deficit {
        deficit
    } else {
        max_deficit
    };
    COMEBACK_CAPTURE_BONUS_PER_DEFICIT * steps
}

pub(super) const fn award_flag_steal_bounties(
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

pub(super) const fn award_flag_return_bounties(
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

pub(super) const fn award_capture_momentum_boosts(
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

pub(super) const fn award_flag_steal_momentum_boosts(
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

pub(super) const fn award_flag_return_momentum_boosts(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_bounty_rewards_only_new_captures() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        // Player is level before answering (1-1), so only the fresh capture pays
        // and the comeback bonus stays out of this fixture (that lever is covered
        // by its own tests).
        award_capture_bounties(
            CaptureScore {
                player: 1,
                opponents: 1,
            },
            CaptureScore {
                player: 2,
                opponents: 1,
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
    fn a_behind_team_banks_a_comeback_bonus_on_top_of_a_capture() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        // Player trails 0-1, then answers to level. The fresh capture pays the
        // flat bounty plus a one-step comeback bonus for clawing back from one
        // capture down.
        award_capture_bounties(
            CaptureScore {
                player: 0,
                opponents: 1,
            },
            CaptureScore {
                player: 1,
                opponents: 1,
            },
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(
            player_economy.cash,
            CAPTURE_CASH_BOUNTY + COMEBACK_CAPTURE_BONUS_PER_DEFICIT
        );
        assert_eq!(player_economy.captures, 1);
        assert_eq!(opponent_economy.cash, 0);
    }

    #[test]
    fn a_level_team_banks_no_comeback_bonus() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        // Player captures from level (1-1): no deficit to claw back, so no bonus.
        award_capture_bounties(
            CaptureScore {
                player: 1,
                opponents: 1,
            },
            CaptureScore {
                player: 2,
                opponents: 1,
            },
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(player_economy.cash, CAPTURE_CASH_BOUNTY);
    }

    #[test]
    fn a_leading_team_banks_no_comeback_bonus() {
        let mut player_economy = Score::default();
        let mut opponent_economy = OpponentScore::default();

        // Player extends a 1-0 lead: ahead, not behind, so no comeback bonus.
        award_capture_bounties(
            CaptureScore {
                player: 1,
                opponents: 0,
            },
            CaptureScore {
                player: 2,
                opponents: 0,
            },
            &mut player_economy,
            &mut opponent_economy,
        );

        assert_eq!(player_economy.cash, CAPTURE_CASH_BOUNTY);
    }

    #[test]
    fn the_comeback_bonus_scales_with_the_deficit() {
        // Clawing back from two down pays more than from one down, so a deeper
        // comeback is worth more.
        let one_down = comeback_capture_bonus(0, 1, 1);
        let two_down = comeback_capture_bonus(0, 2, 1);
        assert!(
            two_down > one_down,
            "a deeper comeback should pay more: two_down={two_down}, one_down={one_down}"
        );
        assert_eq!(one_down, COMEBACK_CAPTURE_BONUS_PER_DEFICIT);
        assert_eq!(two_down, 2 * COMEBACK_CAPTURE_BONUS_PER_DEFICIT);
    }

    #[test]
    fn the_comeback_bonus_is_capped_at_the_deepest_live_deficit() {
        // A deficit beyond the largest a live match can hold is clamped, so the
        // bonus never runs away.
        let capped = comeback_capture_bonus(0, CAPTURES_TO_WIN + 5, 1);
        assert_eq!(
            capped,
            COMEBACK_CAPTURE_BONUS_PER_DEFICIT * (CAPTURES_TO_WIN - 1)
        );
    }

    #[test]
    fn the_comeback_bonus_needs_a_fresh_capture() {
        // No capture this frame (tally unchanged) pays nothing however deep the
        // deficit.
        assert_eq!(comeback_capture_bonus(0, 2, 0), 0);
    }

    #[test]
    fn a_comeback_capture_never_out_earns_taking_a_flag() {
        // Even the deepest comeback bonus stays below a capture's own bounty, so
        // closing the gap never out-earns taking a flag.
        let deepest = comeback_capture_bonus(0, CAPTURES_TO_WIN - 1, 1);
        assert!(
            deepest < CAPTURE_CASH_BOUNTY,
            "comeback bonus {deepest} must stay below the capture bounty {CAPTURE_CASH_BOUNTY}"
        );
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
}
