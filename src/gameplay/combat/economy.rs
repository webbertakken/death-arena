//! The wreck cash economy: how a frame's wreck events price the bounty each team
//! banks.
//!
//! The pure pricing-policy half of the combat model, split from the ram, wreck,
//! stun and surge *mechanics* in the parent `combat` module that produce the
//! [`WreckEvents`] this module reads. Every function here is a pure,
//! `const`-evaluable rule keyed on a frame's wreck facts (who fell, who was
//! carrying, the capture standing, the payback windows, the clock) together with
//! the combat tuning constants that live in the parent module. Nothing here
//! touches the ECS world; the combat systems feed these results into the per-team
//! [`crate::gameplay::pickup::Score`] tallies.

use super::{
    PaybackWindows, WreckCarriers, WreckEvents, CARRIER_TAKEDOWN_WRECK_BONUS,
    CLUTCH_WRECK_CASH_BONUS, FIRST_BLOOD_CASH_BONUS, MOST_WANTED_BOUNTY_PER_CAPTURE_LEAD,
    MOST_WANTED_MAX_CAPTURE_LEAD, PAYBACK_CASH_BONUS, SHUTDOWN_BOUNTY_PER_STREAK_STEP,
    SHUTDOWN_MAX_STREAK_STEPS, SHUTDOWN_MIN_STREAK, WRECK_CASH_BOUNTY, WRECK_STREAK_BONUS,
    WRECK_STREAK_BONUS_CAP,
};
use crate::gameplay::ctf::CaptureScore;
use bevy::prelude::*;

/// Consecutive wrecks each team has dealt without being wrecked itself.
///
/// A team's streak climbs each time it grinds an enemy car to a full wreck and
/// resets the instant the team is wrecked in turn, so only a sustained rampage
/// earns the escalating [`wreck_bounty_for_streak`] payday.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WreckStreaks {
    /// Wrecks the player team has dealt in its current rampage.
    pub player: u32,
    /// Wrecks the opponent team has dealt in its current rampage.
    pub opponent: u32,
}

/// The streaks and per-team bounties that result from a frame's wreck events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WreckStreakPayout {
    /// The teams' rampage streaks after this frame is resolved.
    pub streaks: WreckStreaks,
    /// Cash banked for the player team this frame (`0` when it dealt no wreck).
    pub player_bounty: u32,
    /// Cash banked for the opponent team this frame.
    pub opponent_bounty: u32,
}

/// Cash a team banks for the `streak`-th consecutive wreck in a rampage.
///
/// The first wreck pays the base [`WRECK_CASH_BOUNTY`]; each further wreck adds
/// [`WRECK_STREAK_BONUS`], capped at [`WRECK_STREAK_BONUS_CAP`] steps so a
/// runaway team cannot snowball its economy forever.
#[must_use]
pub const fn wreck_bounty_for_streak(streak: u32) -> u32 {
    let steps = streak.saturating_sub(1);
    let capped = if steps > WRECK_STREAK_BONUS_CAP {
        WRECK_STREAK_BONUS_CAP
    } else {
        steps
    };
    WRECK_CASH_BOUNTY + capped * WRECK_STREAK_BONUS
}

/// Advances each team's rampage streak for a frame's wreck events and prices the
/// bounty each dealt wreck pays.
///
/// The player team deals a wreck when the opponents fall, and vice versa.
/// Dealing a wreck extends the dealer's streak and banks
/// [`wreck_bounty_for_streak`]; being wrecked breaks the victim's streak first.
/// When both teams fall in the same frame each mutually breaks the other's
/// rampage, so both restart at a single wreck and bank the base bounty.
#[must_use]
pub const fn resolve_wreck_streaks(before: WreckStreaks, wrecks: WreckEvents) -> WreckStreakPayout {
    let mut streaks = before;
    let mut player_bounty = 0;
    let mut opponent_bounty = 0;

    // Being wrecked breaks your own rampage before this frame's wreck counts.
    if wrecks.player {
        streaks.player = 0;
    }
    if wrecks.opponent {
        streaks.opponent = 0;
    }
    // The player team deals a wreck when the opponents are the ones wrecked.
    if wrecks.opponent {
        streaks.player += 1;
        player_bounty = wreck_bounty_for_streak(streaks.player);
    }
    if wrecks.player {
        streaks.opponent += 1;
        opponent_bounty = wreck_bounty_for_streak(streaks.opponent);
    }

    WreckStreakPayout {
        streaks,
        player_bounty,
        opponent_bounty,
    }
}

/// Cash bonus a team banks for wrecking a car belonging to the capture leader.
///
/// `victim_captures` is the capture tally of the team that was wrecked,
/// `dealer_captures` that of the team that dealt the wreck. The bonus is paid
/// only when the wrecked team leads on captures, scaling with the lead up to
/// [`MOST_WANTED_MAX_CAPTURE_LEAD`]; wrecking a level or trailing team pays
/// nothing, so only taking down the side that is ahead bankrolls a comeback.
#[must_use]
pub const fn most_wanted_wreck_bonus(victim_captures: u32, dealer_captures: u32) -> u32 {
    let lead = victim_captures.saturating_sub(dealer_captures);
    let capped = if lead > MOST_WANTED_MAX_CAPTURE_LEAD {
        MOST_WANTED_MAX_CAPTURE_LEAD
    } else {
        lead
    };
    capped * MOST_WANTED_BOUNTY_PER_CAPTURE_LEAD
}

/// Cash bonus a team banks for wrecking an enemy car that was carrying a flag.
///
/// `victim_was_carrying` is whether the wrecked team had a car hauling the enemy
/// flag on the frame it fell. A carrier takedown both denies the capture and
/// forces a turnover, so it pays the [`CARRIER_TAKEDOWN_WRECK_BONUS`] on top of
/// every other wreck reward; wrecking an empty-handed car adds nothing.
#[must_use]
pub const fn carrier_takedown_wreck_bonus(victim_was_carrying: bool) -> u32 {
    if victim_was_carrying {
        CARRIER_TAKEDOWN_WRECK_BONUS
    } else {
        0
    }
}

/// Cash bonus a team banks for ending an enemy rampage by wrecking one of its
/// cars.
///
/// `victim_streak` is the number of consecutive wrecks the wrecked team had racked
/// up on the frame it fell. The bonus scales with how deep that rampage was, paid
/// per step above [`SHUTDOWN_MIN_STREAK`] up to [`SHUTDOWN_MAX_STREAK_STEPS`];
/// wrecking a car of a team that was not on a rampage pays nothing. The combat
/// mirror of [`most_wanted_wreck_bonus`]: where that prices the capture leader's
/// head, this prices the wreck leader's.
#[must_use]
pub const fn shutdown_wreck_bonus(victim_streak: u32) -> u32 {
    let rampage = victim_streak.saturating_sub(SHUTDOWN_MIN_STREAK);
    let capped = if rampage > SHUTDOWN_MAX_STREAK_STEPS {
        SHUTDOWN_MAX_STREAK_STEPS
    } else {
        rampage
    };
    capped * SHUTDOWN_BOUNTY_PER_STREAK_STEP
}

/// Cash bonus a team banks for drawing first blood by wrecking an enemy car.
///
/// `available` is whether the round's first blood is still unclaimed, `dealt_wreck`
/// whether this team ground an enemy car down to a full wreck this frame. The
/// [`FIRST_BLOOD_CASH_BONUS`] is paid only on the opening wreck of the round; once
/// drawn it is spent, so every later wreck pays nothing extra here.
#[must_use]
pub const fn first_blood_wreck_bonus(available: bool, dealt_wreck: bool) -> u32 {
    if available && dealt_wreck {
        FIRST_BLOOD_CASH_BONUS
    } else {
        0
    }
}

/// Cash bonus a team banks for a payback wreck: hitting straight back while still
/// smarting from a recent wreck of its own.
///
/// `window_live` is whether this team was wrecked recently enough to still be owed
/// a riposte, `dealt_wreck` whether it ground an enemy car down to a full wreck
/// this frame. The [`PAYBACK_CASH_BONUS`] is paid only when a team that was itself
/// knocked out wrecks an enemy back inside the [`super::PAYBACK_WINDOW_FRAMES`] window;
/// a kill landed by a team that has not been wrecked recently pays nothing extra.
#[must_use]
pub const fn payback_wreck_bonus(window_live: bool, dealt_wreck: bool) -> u32 {
    if window_live && dealt_wreck {
        PAYBACK_CASH_BONUS
    } else {
        0
    }
}

/// Cash bonus a team banks for a clutch wreck: a kill landed in closing time.
///
/// `closing_time` is whether the round is in its closing stretch (the final
/// frames of regulation or any moment of sudden death), `dealt_wreck` whether this
/// team ground an enemy car down to a full wreck this frame. The
/// [`CLUTCH_WRECK_CASH_BONUS`] is paid only when a wreck lands while the clock is
/// running down the match; a kill earlier in the round pays nothing extra here.
#[must_use]
pub const fn clutch_wreck_bonus(closing_time: bool, dealt_wreck: bool) -> u32 {
    if closing_time && dealt_wreck {
        CLUTCH_WRECK_CASH_BONUS
    } else {
        0
    }
}

/// Every cash reward a frame's wrecks pay each team, with the bonus breakdown
/// preserved for logging.
///
/// `player`/`opponent` are the totals each team banks; the `_most_wanted` and
/// `_carrier_takedown` fields are the bonuses already folded into those totals,
/// kept separate only so the wreck log can attribute the payout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WreckBounties {
    /// Each team's rampage streak after this frame's wrecks.
    pub streaks: WreckStreaks,
    /// Total cash the player team banks for wrecks it dealt this frame.
    pub player: u32,
    /// Total cash the opponent team banks for wrecks it dealt this frame.
    pub opponent: u32,
    /// Most-wanted leader bonus folded into `player`.
    pub player_most_wanted: u32,
    /// Most-wanted leader bonus folded into `opponent`.
    pub opponent_most_wanted: u32,
    /// Carrier-takedown bonus folded into `player`.
    pub player_carrier_takedown: u32,
    /// Carrier-takedown bonus folded into `opponent`.
    pub opponent_carrier_takedown: u32,
    /// Shutdown bonus (for ending the opponents' rampage) folded into `player`.
    pub player_shutdown: u32,
    /// Shutdown bonus (for ending the player team's rampage) folded into
    /// `opponent`.
    pub opponent_shutdown: u32,
    /// First-blood bonus (for the round's opening wreck) folded into `player`.
    pub player_first_blood: u32,
    /// First-blood bonus (for the round's opening wreck) folded into `opponent`.
    pub opponent_first_blood: u32,
    /// Payback bonus (for a retaliation wreck) folded into `player`.
    pub player_payback: u32,
    /// Payback bonus (for a retaliation wreck) folded into `opponent`.
    pub opponent_payback: u32,
    /// Clutch bonus (for a closing-time wreck) folded into `player`.
    pub player_clutch: u32,
    /// Clutch bonus (for a closing-time wreck) folded into `opponent`.
    pub opponent_clutch: u32,
}

/// Resolves every cash reward a frame's wrecks pay: the rampage streak payout,
/// the most-wanted leader bonus, the carrier-takedown bonus, the shutdown bonus
/// for ending an enemy rampage, the first-blood bonus for the opening wreck, the
/// payback bonus for a retaliation wreck, and the clutch bonus for a closing-time
/// wreck.
///
/// The player team deals a wreck when the opponents fall (and vice versa), so it
/// collects on the opponents' capture lead, on a wrecked opponent carrier, and on
/// ending the opponents' rampage.
/// `carriers` says whether each team had a car hauling the enemy flag the frame
/// it fell. `first_blood_available` is whether the round's opening wreck is still
/// up for grabs; when it is, whichever side(s) deal a wreck this frame draw first
/// blood.
/// `payback` is each team's payback window as it stood entering the frame; a team
/// that deals a wreck while its window is still live banks the payback.
/// `closing_time` is whether the round is in its closing stretch; a wreck landed
/// while it is banks the clutch bonus for whichever side(s) dealt it. Bonuses
/// are folded into the per-team totals and also returned individually for the
/// wreck log.
#[must_use]
pub const fn resolve_wreck_bounties(
    before_streaks: WreckStreaks,
    wrecks: WreckEvents,
    captures: CaptureScore,
    carriers: WreckCarriers,
    first_blood_available: bool,
    payback: PaybackWindows,
    closing_time: bool,
) -> WreckBounties {
    let payout = resolve_wreck_streaks(before_streaks, wrecks);

    let player_most_wanted = if wrecks.opponent {
        most_wanted_wreck_bonus(captures.opponents, captures.player)
    } else {
        0
    };
    let opponent_most_wanted = if wrecks.player {
        most_wanted_wreck_bonus(captures.player, captures.opponents)
    } else {
        0
    };

    let player_carrier_takedown = if wrecks.opponent {
        carrier_takedown_wreck_bonus(carriers.opponent)
    } else {
        0
    };
    let opponent_carrier_takedown = if wrecks.player {
        carrier_takedown_wreck_bonus(carriers.player)
    } else {
        0
    };

    // The wrecked team's *pre-frame* streak is the rampage this wreck just ended,
    // so the shutdown bonus reads `before_streaks` rather than the post-reset
    // `payout.streaks`.
    let player_shutdown = if wrecks.opponent {
        shutdown_wreck_bonus(before_streaks.opponent)
    } else {
        0
    };
    let opponent_shutdown = if wrecks.player {
        shutdown_wreck_bonus(before_streaks.player)
    } else {
        0
    };

    // The opening wreck of the round draws first blood for whichever side dealt
    // it: a wrecked opponent means the player team landed the kill, and vice
    // versa. A simultaneous double wreck pays both, since each dealt a wreck.
    let player_first_blood = first_blood_wreck_bonus(first_blood_available, wrecks.opponent);
    let opponent_first_blood = first_blood_wreck_bonus(first_blood_available, wrecks.player);

    // A payback is owed to whichever side was wrecked recently and now wrecks an
    // enemy back: a wrecked opponent means the player team landed the riposte, and
    // vice versa. The window is read from before this frame's wreck, so trading a
    // wreck back on the very same frame is a double wreck, not a retaliation.
    let player_payback = payback_wreck_bonus(payback.is_player_live(), wrecks.opponent);
    let opponent_payback = payback_wreck_bonus(payback.is_opponent_live(), wrecks.player);

    // A clutch wreck is any kill landed while the clock is running the match down:
    // a wrecked opponent means the player team landed it, and vice versa. The clock
    // is shared, so a double wreck in closing time pays both, mirroring first blood.
    let player_clutch = clutch_wreck_bonus(closing_time, wrecks.opponent);
    let opponent_clutch = clutch_wreck_bonus(closing_time, wrecks.player);

    WreckBounties {
        streaks: payout.streaks,
        player: payout.player_bounty
            + player_most_wanted
            + player_carrier_takedown
            + player_shutdown
            + player_first_blood
            + player_payback
            + player_clutch,
        opponent: payout.opponent_bounty
            + opponent_most_wanted
            + opponent_carrier_takedown
            + opponent_shutdown
            + opponent_first_blood
            + opponent_payback
            + opponent_clutch,
        player_most_wanted,
        opponent_most_wanted,
        player_carrier_takedown,
        opponent_carrier_takedown,
        player_shutdown,
        opponent_shutdown,
        player_first_blood,
        opponent_first_blood,
        player_payback,
        opponent_payback,
        player_clutch,
        opponent_clutch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::combat::PAYBACK_WINDOW_FRAMES;
    use crate::gameplay::ctf::{CAPTURE_CASH_BOUNTY, FLAG_RETURN_CASH_BOUNTY};

    #[test]
    fn a_lone_wreck_pays_the_base_bounty() {
        assert_eq!(wreck_bounty_for_streak(0), WRECK_CASH_BOUNTY);
        assert_eq!(wreck_bounty_for_streak(1), WRECK_CASH_BOUNTY);
    }

    #[test]
    fn each_consecutive_wreck_raises_the_bounty() {
        let bounties: Vec<u32> = (1..=WRECK_STREAK_BONUS_CAP + 1)
            .map(wreck_bounty_for_streak)
            .collect();
        for pair in bounties.windows(2) {
            assert!(
                pair[1] > pair[0],
                "a longer rampage must pay more: {bounties:?}"
            );
        }
        assert_eq!(
            wreck_bounty_for_streak(2),
            WRECK_CASH_BOUNTY + WRECK_STREAK_BONUS
        );
    }

    #[test]
    fn the_rampage_bounty_is_capped() {
        let capped = WRECK_CASH_BOUNTY + WRECK_STREAK_BONUS_CAP * WRECK_STREAK_BONUS;
        assert_eq!(wreck_bounty_for_streak(WRECK_STREAK_BONUS_CAP + 1), capped);
        assert_eq!(wreck_bounty_for_streak(99), capped);
    }

    #[test]
    fn most_wanted_pays_nothing_for_wrecking_a_level_or_trailing_team() {
        assert_eq!(
            most_wanted_wreck_bonus(2, 2),
            0,
            "a level victim has no price on its head"
        );
        assert_eq!(
            most_wanted_wreck_bonus(1, 2),
            0,
            "wrecking the team that is behind earns no comeback bonus"
        );
    }

    #[test]
    fn most_wanted_bonus_scales_with_the_leader_capture_lead() {
        assert_eq!(
            most_wanted_wreck_bonus(1, 0),
            MOST_WANTED_BOUNTY_PER_CAPTURE_LEAD,
            "a one-capture lead is worth a single step"
        );
        assert_eq!(
            most_wanted_wreck_bonus(2, 0),
            2 * MOST_WANTED_BOUNTY_PER_CAPTURE_LEAD,
            "a wider lead is worth proportionally more"
        );
    }

    #[test]
    fn most_wanted_bonus_is_capped_at_the_max_lead() {
        let capped = MOST_WANTED_MAX_CAPTURE_LEAD * MOST_WANTED_BOUNTY_PER_CAPTURE_LEAD;
        assert_eq!(
            most_wanted_wreck_bonus(MOST_WANTED_MAX_CAPTURE_LEAD + 5, 0),
            capped
        );
        assert_eq!(most_wanted_wreck_bonus(u32::MAX, 0), capped);
    }

    #[test]
    fn taking_the_leader_down_never_out_earns_a_capture() {
        assert!(
            most_wanted_wreck_bonus(u32::MAX, 0) < CAPTURE_CASH_BOUNTY,
            "the comeback lever must stay below the value of a capture"
        );
    }

    #[test]
    fn carrier_takedown_pays_nothing_for_wrecking_an_empty_handed_car() {
        assert_eq!(
            carrier_takedown_wreck_bonus(false),
            0,
            "wrecking a car that was not running a flag earns no takedown bonus"
        );
    }

    #[test]
    fn carrier_takedown_pays_the_bonus_for_wrecking_a_flag_carrier() {
        assert_eq!(
            carrier_takedown_wreck_bonus(true),
            CARRIER_TAKEDOWN_WRECK_BONUS,
            "cutting down the enemy flag carrier must pay the takedown bonus"
        );
    }

    #[test]
    fn taking_a_carrier_down_out_earns_a_return_but_not_a_capture() {
        let takedown = carrier_takedown_wreck_bonus(true);
        assert!(
            takedown > FLAG_RETURN_CASH_BOUNTY,
            "cutting the carrier down must beat merely returning the flag it drops: {takedown}"
        );
        assert!(
            takedown < CAPTURE_CASH_BOUNTY,
            "denying a capture must never out-earn scoring one: {takedown}"
        );
    }

    #[test]
    fn shutdown_pays_nothing_for_wrecking_a_team_not_on_a_rampage() {
        assert_eq!(
            shutdown_wreck_bonus(0),
            0,
            "a team with no kills has no price on its head"
        );
        assert_eq!(
            shutdown_wreck_bonus(SHUTDOWN_MIN_STREAK),
            0,
            "a lone single wreck is no rampage, so ending it pays nothing"
        );
    }

    #[test]
    fn shutdown_bonus_scales_with_the_rampage_depth() {
        assert_eq!(
            shutdown_wreck_bonus(SHUTDOWN_MIN_STREAK + 1),
            SHUTDOWN_BOUNTY_PER_STREAK_STEP,
            "ending a two-wreck rampage is worth a single step"
        );
        assert_eq!(
            shutdown_wreck_bonus(SHUTDOWN_MIN_STREAK + 2),
            2 * SHUTDOWN_BOUNTY_PER_STREAK_STEP,
            "a deeper rampage is worth proportionally more to end"
        );
    }

    #[test]
    fn shutdown_bonus_is_capped_at_the_max_rampage() {
        let capped = SHUTDOWN_MAX_STREAK_STEPS * SHUTDOWN_BOUNTY_PER_STREAK_STEP;
        assert_eq!(
            shutdown_wreck_bonus(SHUTDOWN_MIN_STREAK + SHUTDOWN_MAX_STREAK_STEPS + 5),
            capped,
            "the shutdown reward tops out at the capped rampage depth"
        );
        assert_eq!(shutdown_wreck_bonus(u32::MAX), capped);
    }

    #[test]
    fn the_shutdown_caps_where_the_rampage_earnings_do() {
        assert_eq!(
            SHUTDOWN_MAX_STREAK_STEPS, WRECK_STREAK_BONUS_CAP,
            "ending a rampage must scale exactly as deep as the rampage's own payday"
        );
    }

    #[test]
    fn ending_a_rampage_never_out_earns_a_capture() {
        assert!(
            shutdown_wreck_bonus(u32::MAX) < CAPTURE_CASH_BOUNTY,
            "the comeback lever must stay below the value of a capture"
        );
    }

    #[test]
    fn resolve_wreck_bounties_pays_a_shutdown_for_ending_an_enemy_rampage() {
        // The opponents were three wrecks deep into a rampage when the player team
        // finally cut one of their cars down: ending the run pays the shutdown
        // bounty on top of the base wreck bounty.
        let before = WreckStreaks {
            player: 0,
            opponent: 3,
        };
        let bounties = resolve_wreck_bounties(
            before,
            WreckEvents {
                player: false,
                opponent: true,
            },
            CaptureScore::default(),
            WreckCarriers {
                player: false,
                opponent: false,
            },
            false,
            PaybackWindows::default(),
            false,
        );

        assert_eq!(bounties.player_shutdown, shutdown_wreck_bonus(3));
        assert_eq!(
            bounties.player,
            WRECK_CASH_BOUNTY + shutdown_wreck_bonus(3),
            "ending the opponents' rampage must fold its shutdown bonus into the player total"
        );
        assert_eq!(
            bounties.opponent_shutdown, 0,
            "the side that fell ended no run"
        );
    }

    #[test]
    fn resolve_wreck_bounties_pays_no_shutdown_for_wrecking_a_calm_team() {
        let bounties = resolve_wreck_bounties(
            WreckStreaks::default(),
            WreckEvents {
                player: false,
                opponent: true,
            },
            CaptureScore::default(),
            WreckCarriers {
                player: false,
                opponent: false,
            },
            false,
            PaybackWindows::default(),
            false,
        );

        assert_eq!(bounties.player_shutdown, 0);
        assert_eq!(bounties.player, WRECK_CASH_BOUNTY);
    }

    #[test]
    fn resolve_wreck_bounties_stacks_streak_leader_and_carrier_rewards() {
        // The player team wrecks the opponents, who lead by two captures and were
        // hauling a flag: the base bounty, the most-wanted comeback bonus, and the
        // carrier-takedown bonus all ride the same wreck.
        let bounties = resolve_wreck_bounties(
            WreckStreaks::default(),
            WreckEvents {
                player: false,
                opponent: true,
            },
            CaptureScore {
                player: 0,
                opponents: 2,
            },
            WreckCarriers {
                player: false,
                opponent: true,
            },
            false,
            PaybackWindows::default(),
            false,
        );

        assert_eq!(bounties.player_most_wanted, most_wanted_wreck_bonus(2, 0));
        assert_eq!(
            bounties.player_carrier_takedown,
            CARRIER_TAKEDOWN_WRECK_BONUS
        );
        assert_eq!(
            bounties.player,
            WRECK_CASH_BOUNTY + most_wanted_wreck_bonus(2, 0) + CARRIER_TAKEDOWN_WRECK_BONUS,
            "every reward the player team earns this frame must fold into its total"
        );
        assert_eq!(bounties.opponent, 0, "the side that fell banks nothing");
        assert_eq!(
            bounties.streaks.player, 1,
            "dealing the wreck extends the player team's rampage"
        );
    }

    #[test]
    fn resolve_wreck_bounties_pays_an_empty_handed_wreck_only_the_base_bounty() {
        let bounties = resolve_wreck_bounties(
            WreckStreaks::default(),
            WreckEvents {
                player: false,
                opponent: true,
            },
            CaptureScore::default(),
            WreckCarriers {
                player: false,
                opponent: false,
            },
            false,
            PaybackWindows::default(),
            false,
        );

        assert_eq!(bounties.player, WRECK_CASH_BOUNTY);
        assert_eq!(bounties.player_most_wanted, 0);
        assert_eq!(bounties.player_carrier_takedown, 0);
    }

    #[test]
    fn first_blood_pays_the_opening_wreck() {
        assert_eq!(
            first_blood_wreck_bonus(true, true),
            FIRST_BLOOD_CASH_BONUS,
            "dealing the round's first wreck while first blood is up draws it"
        );
    }

    #[test]
    fn first_blood_pays_nothing_once_spent() {
        assert_eq!(
            first_blood_wreck_bonus(false, true),
            0,
            "first blood is spent once drawn, so a later wreck pays nothing extra"
        );
    }

    #[test]
    fn first_blood_pays_nothing_without_a_wreck() {
        assert_eq!(
            first_blood_wreck_bonus(true, false),
            0,
            "first blood needs an actual wreck to be drawn"
        );
    }

    #[test]
    fn first_blood_pays_nothing_when_spent_and_no_wreck() {
        assert_eq!(first_blood_wreck_bonus(false, false), 0);
    }

    #[test]
    fn payback_pays_a_retaliation_wreck() {
        assert_eq!(
            payback_wreck_bonus(true, true),
            PAYBACK_CASH_BONUS,
            "wrecking an enemy while still owed a riposte banks the payback bonus"
        );
    }

    #[test]
    fn payback_pays_nothing_without_a_live_window() {
        assert_eq!(
            payback_wreck_bonus(false, true),
            0,
            "a kill by a team not recently wrecked is no riposte"
        );
    }

    #[test]
    fn payback_pays_nothing_without_a_wreck() {
        assert_eq!(
            payback_wreck_bonus(true, false),
            0,
            "being owed a riposte pays nothing until an enemy is actually wrecked"
        );
    }

    #[test]
    fn payback_pays_nothing_when_idle_and_no_wreck() {
        assert_eq!(payback_wreck_bonus(false, false), 0);
    }

    #[test]
    fn clutch_pays_a_closing_time_wreck() {
        assert_eq!(
            clutch_wreck_bonus(true, true),
            CLUTCH_WRECK_CASH_BONUS,
            "wrecking an enemy while the clock is closing out the round banks the clutch bonus"
        );
    }

    #[test]
    fn clutch_pays_nothing_outside_closing_time() {
        assert_eq!(
            clutch_wreck_bonus(false, true),
            0,
            "a wreck landed before closing time pays nothing extra"
        );
    }

    #[test]
    fn clutch_pays_nothing_without_a_wreck() {
        assert_eq!(
            clutch_wreck_bonus(true, false),
            0,
            "closing time pays nothing until an enemy is actually wrecked"
        );
    }

    #[test]
    fn clutch_pays_nothing_when_calm_and_no_wreck() {
        assert_eq!(clutch_wreck_bonus(false, false), 0);
    }

    #[test]
    fn landing_a_clutch_wreck_is_a_real_payday_below_a_capture() {
        let bonus = clutch_wreck_bonus(true, true);
        assert!(
            bonus > 0,
            "a clutch wreck must be a real dying-seconds payday"
        );
        assert!(
            bonus < CAPTURE_CASH_BOUNTY,
            "the closing-time reward must never eclipse scoring a capture"
        );
    }

    #[test]
    fn paying_back_a_wreck_is_a_real_payday_below_a_capture() {
        let bonus = payback_wreck_bonus(true, true);
        assert!(bonus > 0, "a payback must be a real retaliation payday");
        assert!(
            bonus < CAPTURE_CASH_BOUNTY,
            "the riposte reward must never eclipse scoring a capture"
        );
        assert!(
            bonus < WRECK_CASH_BOUNTY,
            "a payback tops up the kill rather than being worth a second wreck"
        );
    }

    #[test]
    fn drawing_first_blood_is_a_real_payday_below_a_capture() {
        let bonus = first_blood_wreck_bonus(true, true);
        assert!(bonus > 0, "first blood must be a real opening-kill payday");
        assert!(
            bonus < CAPTURE_CASH_BOUNTY,
            "the opening-kill reward must never eclipse scoring a capture"
        );
    }

    #[test]
    fn resolve_wreck_bounties_draws_first_blood_on_the_opening_wreck() {
        // The player team lands the round's opening wreck while first blood is up.
        let bounties = resolve_wreck_bounties(
            WreckStreaks::default(),
            WreckEvents {
                player: false,
                opponent: true,
            },
            CaptureScore::default(),
            WreckCarriers {
                player: false,
                opponent: false,
            },
            true,
            PaybackWindows::default(),
            false,
        );

        assert_eq!(bounties.player_first_blood, FIRST_BLOOD_CASH_BONUS);
        assert_eq!(
            bounties.player,
            WRECK_CASH_BOUNTY + FIRST_BLOOD_CASH_BONUS,
            "the opening wreck folds first blood into the player total"
        );
        assert_eq!(
            bounties.opponent_first_blood, 0,
            "the side that fell drew no first blood"
        );
    }

    #[test]
    fn resolve_wreck_bounties_draws_no_first_blood_once_spent() {
        let bounties = resolve_wreck_bounties(
            WreckStreaks::default(),
            WreckEvents {
                player: false,
                opponent: true,
            },
            CaptureScore::default(),
            WreckCarriers {
                player: false,
                opponent: false,
            },
            false,
            PaybackWindows::default(),
            false,
        );

        assert_eq!(bounties.player_first_blood, 0);
        assert_eq!(bounties.player, WRECK_CASH_BOUNTY);
    }

    #[test]
    fn resolve_wreck_bounties_draws_first_blood_for_both_on_a_double_opening_wreck() {
        // Both teams are ground out on the same opening frame: each dealt a wreck,
        // so each draws first blood, mirroring how the base bounty restarts for both.
        let bounties = resolve_wreck_bounties(
            WreckStreaks::default(),
            WreckEvents {
                player: true,
                opponent: true,
            },
            CaptureScore::default(),
            WreckCarriers {
                player: false,
                opponent: false,
            },
            true,
            PaybackWindows::default(),
            false,
        );

        assert_eq!(bounties.player_first_blood, FIRST_BLOOD_CASH_BONUS);
        assert_eq!(bounties.opponent_first_blood, FIRST_BLOOD_CASH_BONUS);
        assert_eq!(bounties.player, WRECK_CASH_BOUNTY + FIRST_BLOOD_CASH_BONUS);
        assert_eq!(
            bounties.opponent,
            WRECK_CASH_BOUNTY + FIRST_BLOOD_CASH_BONUS
        );
    }

    #[test]
    fn resolve_wreck_bounties_pays_a_payback_for_a_retaliation_wreck() {
        // The player team was wrecked recently (its window is still live) and now
        // grinds an opponent back down: the riposte banks the payback bonus on top
        // of the base wreck bounty.
        let bounties = resolve_wreck_bounties(
            WreckStreaks::default(),
            WreckEvents {
                player: false,
                opponent: true,
            },
            CaptureScore::default(),
            WreckCarriers {
                player: false,
                opponent: false,
            },
            false,
            PaybackWindows {
                player_frames: PAYBACK_WINDOW_FRAMES,
                opponent_frames: 0,
            },
            false,
        );

        assert_eq!(bounties.player_payback, PAYBACK_CASH_BONUS);
        assert_eq!(
            bounties.player,
            WRECK_CASH_BOUNTY + PAYBACK_CASH_BONUS,
            "a retaliation wreck folds the payback into the player total"
        );
        assert_eq!(
            bounties.opponent_payback, 0,
            "the side that fell landed no riposte"
        );
    }

    #[test]
    fn resolve_wreck_bounties_pays_no_payback_without_a_live_window() {
        let bounties = resolve_wreck_bounties(
            WreckStreaks::default(),
            WreckEvents {
                player: false,
                opponent: true,
            },
            CaptureScore::default(),
            WreckCarriers {
                player: false,
                opponent: false,
            },
            false,
            PaybackWindows::default(),
            false,
        );

        assert_eq!(bounties.player_payback, 0);
        assert_eq!(bounties.player, WRECK_CASH_BOUNTY);
    }

    #[test]
    fn resolve_wreck_bounties_pays_payback_to_both_on_a_mutual_revenge_wreck() {
        // Both teams were owed a riposte and both wreck the other this frame: each
        // collects its payback, mirroring how the base bounty restarts for both.
        let bounties = resolve_wreck_bounties(
            WreckStreaks::default(),
            WreckEvents {
                player: true,
                opponent: true,
            },
            CaptureScore::default(),
            WreckCarriers {
                player: false,
                opponent: false,
            },
            false,
            PaybackWindows {
                player_frames: PAYBACK_WINDOW_FRAMES,
                opponent_frames: PAYBACK_WINDOW_FRAMES,
            },
            false,
        );

        assert_eq!(bounties.player_payback, PAYBACK_CASH_BONUS);
        assert_eq!(bounties.opponent_payback, PAYBACK_CASH_BONUS);
        assert_eq!(bounties.player, WRECK_CASH_BOUNTY + PAYBACK_CASH_BONUS);
        assert_eq!(bounties.opponent, WRECK_CASH_BOUNTY + PAYBACK_CASH_BONUS);
    }

    #[test]
    fn resolve_wreck_bounties_pays_no_payback_to_a_side_that_only_fell() {
        // The player team is owed a riposte but it is the one wrecked this frame,
        // not the one dealing the wreck: no payback rides a kill it did not land.
        let bounties = resolve_wreck_bounties(
            WreckStreaks::default(),
            WreckEvents {
                player: true,
                opponent: false,
            },
            CaptureScore::default(),
            WreckCarriers {
                player: false,
                opponent: false,
            },
            false,
            PaybackWindows {
                player_frames: PAYBACK_WINDOW_FRAMES,
                opponent_frames: 0,
            },
            false,
        );

        assert_eq!(
            bounties.player_payback, 0,
            "a payback rides the riposte, not being wrecked again"
        );
        assert_eq!(bounties.opponent_payback, 0);
    }

    #[test]
    fn resolve_wreck_bounties_pays_a_clutch_for_a_closing_time_wreck() {
        // The player team grinds an opponent down while the clock is closing out
        // the round: the kill banks the clutch bonus on top of the base bounty.
        let bounties = resolve_wreck_bounties(
            WreckStreaks::default(),
            WreckEvents {
                player: false,
                opponent: true,
            },
            CaptureScore::default(),
            WreckCarriers {
                player: false,
                opponent: false,
            },
            false,
            PaybackWindows::default(),
            true,
        );

        assert_eq!(bounties.player_clutch, CLUTCH_WRECK_CASH_BONUS);
        assert_eq!(
            bounties.player,
            WRECK_CASH_BOUNTY + CLUTCH_WRECK_CASH_BONUS,
            "a closing-time wreck folds the clutch bonus into the player total"
        );
        assert_eq!(
            bounties.opponent_clutch, 0,
            "the side that fell landed no clutch wreck"
        );
    }

    #[test]
    fn resolve_wreck_bounties_pays_no_clutch_outside_closing_time() {
        let bounties = resolve_wreck_bounties(
            WreckStreaks::default(),
            WreckEvents {
                player: false,
                opponent: true,
            },
            CaptureScore::default(),
            WreckCarriers {
                player: false,
                opponent: false,
            },
            false,
            PaybackWindows::default(),
            false,
        );

        assert_eq!(bounties.player_clutch, 0);
        assert_eq!(bounties.player, WRECK_CASH_BOUNTY);
    }

    #[test]
    fn resolve_wreck_bounties_pays_clutch_to_both_on_a_double_closing_time_wreck() {
        // Both teams are ground out in closing time: the clock is shared, so each
        // dealt wreck banks its clutch bonus, mirroring how first blood pays both.
        let bounties = resolve_wreck_bounties(
            WreckStreaks::default(),
            WreckEvents {
                player: true,
                opponent: true,
            },
            CaptureScore::default(),
            WreckCarriers {
                player: false,
                opponent: false,
            },
            false,
            PaybackWindows::default(),
            true,
        );

        assert_eq!(bounties.player_clutch, CLUTCH_WRECK_CASH_BONUS);
        assert_eq!(bounties.opponent_clutch, CLUTCH_WRECK_CASH_BONUS);
        assert_eq!(bounties.player, WRECK_CASH_BOUNTY + CLUTCH_WRECK_CASH_BONUS);
        assert_eq!(
            bounties.opponent,
            WRECK_CASH_BOUNTY + CLUTCH_WRECK_CASH_BONUS
        );
    }

    #[test]
    fn a_quiet_frame_leaves_streaks_and_pays_nothing() {
        let before = WreckStreaks {
            player: 2,
            opponent: 1,
        };
        let payout = resolve_wreck_streaks(before, WreckEvents::default());
        assert_eq!(payout.streaks, before);
        assert_eq!(payout.player_bounty, 0);
        assert_eq!(payout.opponent_bounty, 0);
    }

    #[test]
    fn dealing_a_wreck_extends_the_dealer_and_resets_the_victim() {
        let before = WreckStreaks {
            player: 1,
            opponent: 2,
        };
        // The opponent team is wrecked, so the player team dealt the wreck.
        let payout = resolve_wreck_streaks(
            before,
            WreckEvents {
                player: false,
                opponent: true,
            },
        );
        assert_eq!(payout.streaks.player, 2);
        assert_eq!(
            payout.streaks.opponent, 0,
            "a wrecked team loses its rampage"
        );
        assert_eq!(payout.player_bounty, wreck_bounty_for_streak(2));
        assert_eq!(payout.opponent_bounty, 0);
    }

    #[test]
    fn an_opponent_rampage_extends_them_and_resets_the_player() {
        let before = WreckStreaks {
            player: 3,
            opponent: 1,
        };
        let payout = resolve_wreck_streaks(
            before,
            WreckEvents {
                player: true,
                opponent: false,
            },
        );
        assert_eq!(payout.streaks.opponent, 2);
        assert_eq!(payout.streaks.player, 0);
        assert_eq!(payout.opponent_bounty, wreck_bounty_for_streak(2));
        assert_eq!(payout.player_bounty, 0);
    }

    #[test]
    fn mutual_wrecks_restart_both_streaks_at_the_base_bounty() {
        let before = WreckStreaks {
            player: 3,
            opponent: 3,
        };
        let payout = resolve_wreck_streaks(
            before,
            WreckEvents {
                player: true,
                opponent: true,
            },
        );
        assert_eq!(payout.streaks.player, 1);
        assert_eq!(payout.streaks.opponent, 1);
        assert_eq!(payout.player_bounty, WRECK_CASH_BOUNTY);
        assert_eq!(payout.opponent_bounty, WRECK_CASH_BOUNTY);
    }
}
