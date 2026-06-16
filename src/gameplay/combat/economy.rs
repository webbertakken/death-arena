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
