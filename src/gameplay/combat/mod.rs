use crate::gameplay::ctf::{
    CaptureScore, CtfFlag, CtfMatchResult, FlagTeam, MatchClock, CAPTURES_TO_WIN,
    CAPTURE_CASH_BOUNTY, FLAG_RETURN_CASH_BOUNTY, FLAG_STEAL_CASH_BOUNTY,
};
use crate::gameplay::main::BOUNDS;
use crate::gameplay::pickup::{ArmourBoosts, NitroBoosts, OpponentScore, Score};
use crate::gameplay::player::Player;
use crate::gameplay::virtual_player::ai::AiTeam;
use crate::gameplay::virtual_player::VirtualPlayer;
use crate::{App, AppState, Plugin};
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;

mod economy;
pub use economy::*;

mod timers;
pub use timers::*;

mod ram;
pub use ram::*;

mod integrity;
pub use integrity::*;

/// Maximum durability a team's cars carry into a match.
pub const MAX_INTEGRITY: f32 = 100.0;
/// Durability restored when a car collects a repair pickup.
pub const REPAIR_INTEGRITY: f32 = 35.0;
/// World-space distance two cars must be within to count as ramming.
///
/// Cars use a `ball(350)` collider scaled to `0.2`, so two of them touch when
/// their centres are roughly 140 units apart.
pub const RAM_RADIUS: f32 = 140.0;
/// Durability a team loses per car caught ramming, each fixed frame.
pub const RAM_DAMAGE_PER_FRAME: f32 = 0.25;
/// Speed multiplier applied to a fully wrecked (zero integrity) team.
pub const MIN_INTEGRITY_SPEED_MULTIPLIER: f32 = 0.65;
/// Extra durability the enemy of a nitro-boosted car loses each frame the two
/// are trading paint.
///
/// A boosted car is charging, so slamming it into an opponent while nitro burns
/// wears the enemy down twice as fast as the base scrape, the classic Death
/// Rally "boost into them to wreck them" play. It also closes the combat loop:
/// nitro ram -> battered enemy -> enemy breaks off for a repair.
pub const NITRO_RAM_DAMAGE_PER_FRAME: f32 = 0.5;
/// Cash a team banks for grinding an enemy car down to a full wreck.
///
/// The classic Death Rally payday: ramming an opponent until their integrity
/// hits zero is worth real money, closing the combat loop the ramming systems
/// open. Priced between a flag steal (50) and a capture (250) so wrecking is a
/// meaningful earner without eclipsing the CTF objective, and it bankrolls the
/// upgrades a battered driver needs to stay in the fight. The bounty pays once
/// per wreck, on the frame integrity crosses to zero, so a team only cashes in
/// again after its victim limps to a repair and is wrecked anew.
pub const WRECK_CASH_BOUNTY: u32 = 150;
/// Wrecking must out-earn a flag steal, enforced at compile time, so grinding an
/// enemy car down to zero stays the meatier payday the pricing intends rather than
/// dropping to a mere steal.
const _: () = assert!(WRECK_CASH_BOUNTY > FLAG_STEAL_CASH_BOUNTY);
/// A wreck must never out-earn scoring a capture, enforced at compile time, so the
/// combat payday stays a meaningful earner without eclipsing the CTF objective, the
/// same ceiling every derived wreck bonus already respects but the base bounty they
/// build on did not yet pin.
const _: () = assert!(WRECK_CASH_BOUNTY < CAPTURE_CASH_BOUNTY);
/// Extra cash each consecutive wreck adds on top of [`WRECK_CASH_BOUNTY`].
///
/// A team that keeps grinding enemies down without being wrecked itself is on a
/// rampage, and a rampage should pay. Each wreck in the streak banks this much
/// more than the last, so chaining wrecks bankrolls upgrades faster than picking
/// off the odd lone car.
pub const WRECK_STREAK_BONUS: u32 = 75;
/// Most consecutive wrecks that still raise the bounty.
///
/// Caps the rampage payday so a dominant team cannot snowball its economy out of
/// reach; wrecks beyond this point still pay, just at the capped top rate. With
/// the base bounty this tops a rampage out at `150 + 3 * 75 = 375` per wreck.
pub const WRECK_STREAK_BONUS_CAP: u32 = 3;
/// Extra cash a team banks per capture the *leader* it just wrecked is ahead by.
///
/// The classic Death Rally "most wanted" bounty: the team winning the round has
/// a price on its head, so taking one of its cars down pays the trailing team
/// extra on top of the base [`WRECK_CASH_BOUNTY`] and any rampage streak. This
/// is the economy's missing anti-snowball lever pointing the other way: the
/// [`WRECK_STREAK_BONUS_CAP`] keeps a *dominant* team from snowballing its cash
/// out of reach, while this lets the *trailing* team bankroll a comeback by
/// hunting the leader. Paid only to the side that is behind on captures, so a
/// leader wrecking the team chasing it earns nothing extra.
pub const MOST_WANTED_BOUNTY_PER_CAPTURE_LEAD: u32 = 100;
/// Largest capture lead the most-wanted bounty still scales with.
///
/// A team reaching [`CAPTURES_TO_WIN`] ends the round, so the widest lead that
/// can stand mid-match is one short of the win. Capping here keeps the bounty
/// bounded even if a future rule ever let the tally climb higher.
pub const MOST_WANTED_MAX_CAPTURE_LEAD: u32 = CAPTURES_TO_WIN - 1;
/// Taking the leader down must never out-earn actually scoring a capture,
/// enforced at compile time, so the comeback lever rewards the chase without
/// eclipsing the objective it is chasing.
const _: () = assert!(
    MOST_WANTED_MAX_CAPTURE_LEAD * MOST_WANTED_BOUNTY_PER_CAPTURE_LEAD < CAPTURE_CASH_BOUNTY
);
/// Extra cash a team banks for wrecking an enemy car that was hauling a flag.
///
/// The marquee defensive play in capture-the-flag: grinding down the car running
/// a stolen flag home is the single most valuable wreck on the board, because it
/// does double duty, it denies the imminent capture *and* knocks the flag loose
/// for a turnover (the carrier already drops it on a wreck). The base
/// [`WRECK_CASH_BOUNTY`] pays for any kill; this rewards aiming that kill at the
/// runner who actually matters, so defending the run home is worth real money
/// rather than a thankless chore. Paid on top of the base bounty, any rampage
/// [`WRECK_STREAK_BONUS`], and the [`most_wanted_wreck_bonus`] leader bonus, and
/// only when the wrecked car was carrying a flag the frame it fell. Priced above
/// a [`FLAG_RETURN_CASH_BOUNTY`] (the next-best way to undo a steal) so cutting
/// the carrier down out-earns mopping up the loose flag afterwards, yet below a
/// [`CAPTURE_CASH_BOUNTY`] so denying a capture never out-pays scoring one.
pub const CARRIER_TAKEDOWN_WRECK_BONUS: u32 = 100;
/// A carrier takedown must out-earn a flag return, enforced at compile time, so
/// cutting the runner down beats merely tidying up the flag it drops.
const _: () = assert!(CARRIER_TAKEDOWN_WRECK_BONUS > FLAG_RETURN_CASH_BOUNTY);
/// Denying a capture must never out-earn scoring one, enforced at compile time,
/// so the takedown rewards defence without eclipsing the objective.
const _: () = assert!(CARRIER_TAKEDOWN_WRECK_BONUS < CAPTURE_CASH_BOUNTY);
/// Extra cash a team banks per rampage step it ends by wrecking a car of a team
/// on a wreck streak: the bounty on a dangerous driver's head.
///
/// The third anti-snowball lever, completing the set. The capped
/// [`WRECK_STREAK_BONUS`] stops a rampaging team *earning* its way out of reach;
/// the [`most_wanted_wreck_bonus`] bankrolls the side trailing on *captures*; this
/// rewards the side that actually *ends* a rampage. Where most-wanted prices the
/// capture leader's head, this prices the wreck leader's: cutting down a car that
/// has been racking up kills pays extra, so a team being ground down in the scrum
/// can still buy its comeback by stopping the run. Paid on top of the base
/// [`WRECK_CASH_BOUNTY`], any rampage [`WRECK_STREAK_BONUS`], the
/// [`most_wanted_wreck_bonus`] and the [`carrier_takedown_wreck_bonus`], and only
/// when the wrecked team was genuinely on a rampage the frame it fell.
pub const SHUTDOWN_BOUNTY_PER_STREAK_STEP: u32 = 50;
/// Smallest rampage (consecutive wrecks) that still puts a bounty on a team's
/// head.
///
/// A lone single wreck is no rampage, so a shutdown pays from the *second*
/// consecutive wreck on, exactly where a rampage's own escalating
/// [`WRECK_STREAK_BONUS`] snowball begins, so the lever counters precisely the
/// run it is meant to.
pub const SHUTDOWN_MIN_STREAK: u32 = 1;
/// Deepest rampage the shutdown bounty still scales with, matched to
/// [`WRECK_STREAK_BONUS_CAP`] so the reward for ending a rampage tops out exactly
/// where the rampage's own earnings do.
pub const SHUTDOWN_MAX_STREAK_STEPS: u32 = WRECK_STREAK_BONUS_CAP;
/// Each rampage step ended must pay a real bounty, enforced at compile time.
const _: () = assert!(SHUTDOWN_BOUNTY_PER_STREAK_STEP > 0);
/// Ending a rampage must never out-earn scoring a capture, enforced at compile
/// time, so the comeback lever rewards the shutdown without eclipsing the
/// objective, mirroring the same ceiling the most-wanted and carrier-takedown
/// bonuses respect.
const _: () =
    assert!(SHUTDOWN_MAX_STREAK_STEPS * SHUTDOWN_BOUNTY_PER_STREAK_STEP < CAPTURE_CASH_BOUNTY);
/// Cash a team banks for drawing first blood: the round's opening wreck.
///
/// The classic arcade "first blood" reward, the opening-kill payday. Where every
/// other wreck bonus prices an *ongoing* situation, a rampage
/// ([`WRECK_STREAK_BONUS`]), the capture leader ([`most_wanted_wreck_bonus`]), a
/// flag carrier ([`carrier_takedown_wreck_bonus`]), or a run being ended
/// ([`shutdown_wreck_bonus`]), this rewards being the side that gets stuck in
/// first, so the opening of a round is a scramble for the kill rather than a tame
/// lap farming pickups. Paid once per round, on the very first wreck, on top of
/// the base [`WRECK_CASH_BOUNTY`] and any other bonus that frame. A simultaneous
/// double wreck on the opening frame pays both sides, mirroring how the base
/// bounty and rampage both restart for each team at once.
pub const FIRST_BLOOD_CASH_BONUS: u32 = 100;
/// First blood must be a real payday, not a token, enforced at compile time.
const _: () = assert!(FIRST_BLOOD_CASH_BONUS > 0);
/// Drawing first blood must never out-earn scoring a capture, enforced at compile
/// time, so the opening-kill reward never eclipses the objective, mirroring the
/// ceiling every other wreck bonus respects.
const _: () = assert!(FIRST_BLOOD_CASH_BONUS < CAPTURE_CASH_BOUNTY);
/// Cash a team banks for a payback wreck: trading its own wreck straight back.
///
/// The grudge-match riposte. Where every other wreck bonus prices the *victim's*
/// situation, a rampage ([`WRECK_STREAK_BONUS`]), the capture leader
/// ([`most_wanted_wreck_bonus`]), a flag carrier ([`carrier_takedown_wreck_bonus`])
/// or a run being ended ([`shutdown_wreck_bonus`]), and first blood prices being
/// first to a kill, this keys on the *dealer* having just been knocked out itself:
/// a team ground to a wreck that climbs off the canvas and wrecks an enemy back
/// within [`PAYBACK_WINDOW_FRAMES`] banks the retaliation. The fourth anti-snowball
/// lever in spirit, it bankrolls precisely the side being ground down in the scrum,
/// turning a wreck-for-wreck trade into a momentum swing rather than a quiet reset.
/// Paid on top of the base [`WRECK_CASH_BOUNTY`] and any other bonus that frame.
pub const PAYBACK_CASH_BONUS: u32 = 75;
/// A payback must be a real payday, not a token, enforced at compile time.
const _: () = assert!(PAYBACK_CASH_BONUS > 0);
/// A payback tops up the kill, never doubles it, enforced at compile time, so the
/// riposte rewards hitting back without being worth a second wreck on its own.
const _: () = assert!(PAYBACK_CASH_BONUS < WRECK_CASH_BOUNTY);
/// Paying back a wreck must never out-earn scoring a capture, enforced at compile
/// time, so the riposte rewards the comeback without eclipsing the objective,
/// mirroring the ceiling every other wreck bonus respects.
const _: () = assert!(PAYBACK_CASH_BONUS < CAPTURE_CASH_BOUNTY);
/// Fixed update frames a wrecked team stays owed a payback after it is knocked out.
///
/// The window in which trading a wreck straight back counts as a riposte rather
/// than an unrelated kill later in the round. Opened the frame a team is wrecked
/// and wound down each frame by [`payback_window_decay_system`], it must outlast
/// the [`WRECK_STUN_FRAMES`] spin-out so a team can shake off the stagger and
/// actually strike back, yet stay short enough that the retaliation reads as a
/// direct answer to the wreck it avenges. At the game's 60 FPS convention this is
/// five seconds.
pub const PAYBACK_WINDOW_FRAMES: u32 = 300;
/// A payback window must outlast the spin-out it answers, enforced at compile
/// time, so a wrecked team can recover and still land its riposte in time.
const _: () = assert!(PAYBACK_WINDOW_FRAMES > WRECK_STUN_FRAMES);
/// Cash a team banks for a clutch wreck: grinding an enemy down in closing time.
///
/// The dying-seconds heroics reward, the combat companion to the CTF nail-biter
/// purse. Where every other wreck bonus prices a *combat* situation, a rampage
/// ([`WRECK_STREAK_BONUS`]), the capture leader ([`most_wanted_wreck_bonus`]), a
/// flag carrier ([`carrier_takedown_wreck_bonus`]), a run being ended
/// ([`shutdown_wreck_bonus`]), the opening kill ([`first_blood_wreck_bonus`]) or a
/// riposte ([`payback_wreck_bonus`]), this keys on the *clock*: a wreck landed in
/// the round's closing stretch ([`crate::gameplay::ctf::MatchClock::is_closing_time`])
/// pays extra. It bankrolls precisely the moment the match hangs in the balance,
/// where a wreck also breaks a level overtime in the wrecking team's favour
/// (the same closing-time push the virtual players commit to), so the final
/// seconds are a scramble for the kill rather than a tame run down the clock.
/// Paid on top of the base [`WRECK_CASH_BOUNTY`] and any other bonus that frame.
pub const CLUTCH_WRECK_CASH_BONUS: u32 = 100;
/// A clutch wreck must be a real payday, not a token, enforced at compile time.
const _: () = assert!(CLUTCH_WRECK_CASH_BONUS > 0);
/// Landing a clutch wreck must never out-earn scoring a capture, enforced at
/// compile time, so the dying-seconds reward never eclipses the objective,
/// mirroring the ceiling every other wreck bonus respects.
const _: () = assert!(CLUTCH_WRECK_CASH_BONUS < CAPTURE_CASH_BOUNTY);
/// Fixed update frames a freshly wrecked team spins out before it recovers.
///
/// The wreck's punch: the instant a team's integrity is ground to zero its cars
/// spin out, barely creeping for a brief window before they drive again. This
/// is what turns the [`WRECK_CASH_BOUNTY`] from a quiet payout into a real swing
/// of the round, the wrecking team gets a clear opening to push the flag home or
/// break away while the wreck flounders. At the game's 60 FPS convention this is
/// 1.5 seconds, long enough to feel and capitalise on, short enough that a wreck
/// is a setback rather than a death sentence. Fires once on the frame integrity
/// crosses to zero, so a team only spins out anew after a repair lifts it back
/// above zero and it is wrecked again.
pub const WRECK_STUN_FRAMES: u32 = 90;
/// Speed multiplier a team's cars suffer while spinning out from a fresh wreck.
///
/// Stacks on top of the integrity speed penalty, so a wrecked-and-spinning car
/// barely crawls. Harsh enough that the spin-out reads as a real stagger, yet
/// above a dead stop so a stunned car keeps inching and never feels frozen.
pub const WRECK_STUN_SPEED_MULTIPLIER: f32 = 0.35;
/// Fixed update frames a team's cars surge after wrecking an enemy.
///
/// The reward mirror of [`WRECK_STUN_FRAMES`]: the instant a team grinds an
/// enemy car to a full wreck, its own cars get a short burst of speed, the
/// adrenaline of the kill. Matched to the spin-out window so the surge and the
/// victim's stagger overlap exactly, handing the wrecking team a clean opening
/// to push the flag home, break away, or chain a second wreck. Fires once on the
/// frame integrity crosses to zero, mirroring the spin-out, so a team only surges
/// anew on its next kill. At the game's 60 FPS convention this is 1.5 seconds.
pub const WRECK_SURGE_FRAMES: u32 = 90;
/// Speed multiplier a team's cars enjoy while surging from a fresh wreck.
///
/// A moderate burst that rewards landing the kill without eclipsing nitro: kept
/// below the 1.5x nitro boost so nitro stays the fastest a car ever goes, yet
/// high enough that the surge reads as a real swing. Stacks on top of nitro, so a
/// boosted wrecker briefly screams; stacks under the spin-out too, so a team
/// caught in a double wreck still crawls.
pub const WRECK_SURGE_SPEED_MULTIPLIER: f32 = 1.25;
/// A surge must be a real speed-up, enforced at compile time.
const _: () = assert!(WRECK_SURGE_SPEED_MULTIPLIER > 1.0);
/// Nitro must stay the fastest a car can go, enforced at compile time.
const _: () =
    assert!(WRECK_SURGE_SPEED_MULTIPLIER < crate::gameplay::pickup::NITRO_SPEED_MULTIPLIER);
/// Extra durability the enemy of a surging car loses each frame the two are
/// trading paint.
///
/// The offensive companion to [`WRECK_SURGE_SPEED_MULTIPLIER`] and the mirror of
/// [`NITRO_RAM_DAMAGE_PER_FRAME`]: where a fresh wreck speeds the wrecking team's
/// cars up, it also lets them ram the *next* enemy harder, the adrenaline of the
/// kill carried into the next hit. This closes the wreck -> surge -> chain loop the
/// surge was built for: its window is matched frame-for-frame to the victim's
/// spin-out precisely so a team can "chain a second wreck", and now the surging
/// team both reaches the next foe quicker *and* grinds it down faster. Like the
/// nitro charge it needs no aim, landing on whoever the surging car is trading
/// paint with, so it rewards pressing a reeling enemy in the opening the kill made.
/// Priced below the earned [`NITRO_RAM_DAMAGE_PER_FRAME`] so a nitro burst stays
/// the single hardest bite, exactly as the surge speed stays below the nitro speed,
/// and the anti-snowball levers (the capped [`WRECK_STREAK_BONUS`] and the trailing
/// team's [`MOST_WANTED_BOUNTY_PER_CAPTURE_LEAD`]) keep a rampage in check. Fires
/// for whichever team is surging, so a trailing side landing a most-wanted kill
/// chains its comeback just as a leader presses its advantage.
pub const SURGE_RAM_DAMAGE_PER_FRAME: f32 = 0.25;
/// A surge ram must be a real bite, enforced at compile time.
const _: () = assert!(SURGE_RAM_DAMAGE_PER_FRAME > 0.0);
/// A surge ram must stay under the earned nitro charge, enforced at compile time,
/// so a boosted ram remains the single hardest source of wear, mirroring how the
/// surge speed stays under the nitro speed.
const _: () = assert!(SURGE_RAM_DAMAGE_PER_FRAME < NITRO_RAM_DAMAGE_PER_FRAME);
/// Extra durability a flag-carrying car's team loses each frame it is trading
/// paint with an enemy.
///
/// A car hauling the enemy flag is not just slow, it is fragile: defenders who
/// ram the carrier wear its team down twice as fast as the base scrape. This
/// deepens the capture-the-flag gauntlet, the run home becomes a real risk, not
/// a victory lap, and pairs with the flag-carrier slowdown so a battered
/// carrier crawls back into reach of its pursuers.
pub const FLAG_CARRIER_RAM_DAMAGE_PER_FRAME: f32 = 0.5;
/// A flag carrier's team must bleed faster than an incidental open-field scrape,
/// enforced at compile time, so the documented "twice as fast as the base scrape"
/// gauntlet can never silently soften below the [`RAM_DAMAGE_PER_FRAME`] floor it
/// is priced against, the same pin the pincer and wall-crush hits already carry.
const _: () = assert!(FLAG_CARRIER_RAM_DAMAGE_PER_FRAME > RAM_DAMAGE_PER_FRAME);
/// Heading alignment a car needs with an opponent to count as charging it.
///
/// Measured as the dot product between the car's facing direction and the
/// direction to the opponent, so `1.0` is a dead-on charge and `0.0` a
/// side-swipe. At `0.5` the opponent must sit within a 60-degree cone ahead of
/// the car, the spread of a committed ram rather than an incidental scrape.
pub const AGGRESSOR_RAM_ALIGNMENT: f32 = 0.5;
/// Extra durability the target of a car charging head-first into it loses each
/// frame the two are trading paint.
///
/// The heart of the Death Rally ram: pointing your car at an opponent and
/// driving through them wears the target down faster than merely grinding
/// alongside. It stacks on top of the base [`ram_damage`] scrape and rewards
/// aim over accident, so a driver who lines up a hit comes out ahead. Priced
/// below the earned [`NITRO_RAM_DAMAGE_PER_FRAME`] so a boosted charge still
/// bites hardest, yet above the base scrape so committing to a ram always pays.
pub const AGGRESSOR_RAM_DAMAGE_PER_FRAME: f32 = 0.35;
/// An aimed charge must out-bite the incidental base scrape, enforced at compile
/// time, so the documented "committing to a ram always pays" can never silently
/// invert below the [`RAM_DAMAGE_PER_FRAME`] floor, completing the directional
/// hierarchy's footing that the pincer and wall-crush pins already stand on.
const _: () = assert!(AGGRESSOR_RAM_DAMAGE_PER_FRAME > RAM_DAMAGE_PER_FRAME);
/// Heading alignment off a victim's own facing within which a ram counts as
/// catching its exposed flank rather than its nose or tail.
///
/// Measured as the absolute dot product between the victim's facing and the
/// direction to the car striking it, so `0.0` is a dead-square broadside and
/// `1.0` a pure head-on or rear-end. At `0.5` the striker must come from within
/// the victim's side arc (60-120 degrees off its nose), the spread of a genuine
/// T-bone rather than a glancing front-quarter clip.
pub const BROADSIDE_RAM_FLANK_THRESHOLD: f32 = 0.5;
/// Extra durability a car caught side-on by a charging enemy loses each frame
/// the two are trading paint.
///
/// The classic Death Rally T-bone: catching an enemy square in the flank with a
/// committed charge punishes it harder than a head-on meeting, because the
/// struck car cannot trade the hit back, its own nose is pointed elsewhere. A
/// broadside only lands when the striker is *also* charging (the same nose-on
/// commitment [`AGGRESSOR_RAM_ALIGNMENT`] demands), so it stacks on the
/// [`AGGRESSOR_RAM_DAMAGE_PER_FRAME`] hit a clean cut-off already earns and
/// rewards lining up the kill shot on a fleeing or turning foe. Priced a notch
/// above the head-on aggressor bite, since a flank hit is the more punishing
/// angle, yet kept under the earned [`NITRO_RAM_DAMAGE_PER_FRAME`] so a boosted
/// charge stays the single hardest source of wear.
pub const BROADSIDE_RAM_DAMAGE_PER_FRAME: f32 = 0.4;
/// A flank hit must out-bite the head-on aggressor charge, enforced at compile
/// time, so catching a foe side-on always beats meeting it nose-to-nose.
const _: () = assert!(BROADSIDE_RAM_DAMAGE_PER_FRAME > AGGRESSOR_RAM_DAMAGE_PER_FRAME);
/// A flank hit must stay under the earned nitro charge, enforced at compile
/// time, so a boosted ram remains the hardest single hit a car can land.
const _: () = assert!(BROADSIDE_RAM_DAMAGE_PER_FRAME < NITRO_RAM_DAMAGE_PER_FRAME);
/// The flank arc must be a real wedge, enforced at compile time: a positive
/// threshold opens the side window, below `1.0` keeps a pure head-on out of it.
const _: () = assert!(BROADSIDE_RAM_FLANK_THRESHOLD > 0.0 && BROADSIDE_RAM_FLANK_THRESHOLD < 1.0);
/// Extra durability a car caught from directly behind by a charging enemy loses
/// each frame the two are trading paint.
///
/// The classic Death Rally chase-down (the racing-game "PIT" tap): running a
/// fleeing enemy down and driving through its tail. Like the
/// [`BROADSIDE_RAM_DAMAGE_PER_FRAME`] T-bone, the struck car cannot trade the
/// hit back, its nose is pointed away, so a committed rear ram punishes it
/// harder than a head-on meeting. A rear-end only lands when the striker is
/// *also* charging (the same nose-on commitment [`AGGRESSOR_RAM_ALIGNMENT`]
/// demands), so it stacks on the [`AGGRESSOR_RAM_DAMAGE_PER_FRAME`] hit a clean
/// run-down already earns and rewards chasing a fleeing flag carrier or a
/// reeling foe. Priced a notch above the head-on aggressor bite, since the
/// victim cannot retaliate, yet kept under the [`BROADSIDE_RAM_DAMAGE_PER_FRAME`]
/// flank, the more violent perpendicular angle, so a T-bone stays the hardest
/// positional hit and the earned [`NITRO_RAM_DAMAGE_PER_FRAME`] charge the
/// hardest hit of all.
pub const REAR_END_RAM_DAMAGE_PER_FRAME: f32 = 0.375;
/// A rear-end must out-bite the head-on aggressor charge, enforced at compile
/// time, so running a foe down from behind always beats meeting it nose-to-nose.
const _: () = assert!(REAR_END_RAM_DAMAGE_PER_FRAME > AGGRESSOR_RAM_DAMAGE_PER_FRAME);
/// A flank T-bone must stay the hardest positional hit, enforced at compile
/// time, so a clean broadside always out-bites a rear-end run-down.
const _: () = assert!(REAR_END_RAM_DAMAGE_PER_FRAME < BROADSIDE_RAM_DAMAGE_PER_FRAME);
/// Extra durability each car in a nose-to-nose head-on meeting loses every frame
/// the two are trading paint.
///
/// The classic Death Rally game of chicken: when two enemy cars both commit a
/// nose-first charge straight into each other (each inside the other's
/// [`AGGRESSOR_RAM_ALIGNMENT`] cone) the smash wears *both* teams down at once,
/// on top of the base [`ram_damage`] scrape and the mutual
/// [`aggressor_ram_damage`] charge a head-on already trades. Where the one-sided
/// flank ([`BROADSIDE_RAM_DAMAGE_PER_FRAME`]) and rear-end
/// ([`REAR_END_RAM_DAMAGE_PER_FRAME`]) hits concentrate their punishment on a
/// victim that cannot retaliate, a head-on shares it: the cost of meeting a foe
/// nose-to-nose is that you pay it too, so out-positioning into a T-bone or a
/// run-down always beats blinking into a head-on. Priced at the same shared-bite
/// floor as the no-aim [`PINCER_RAM_DAMAGE_PER_FRAME`] gang-up, below even the
/// one-sided aggressor charge it stacks on, since the smash's bite is that both
/// pay rather than that either out-trades the other. A battered car feels it
/// hardest: the same absolute bite eats a larger slice of its thinner pool, so a
/// reeling car should duck a head-on while a healthy one can use it to finish a
/// foe off.
pub const HEAD_ON_RAM_DAMAGE_PER_FRAME: f32 = 0.3;
/// A head-on smash must be a real bite, enforced at compile time.
const _: () = assert!(HEAD_ON_RAM_DAMAGE_PER_FRAME > 0.0);
/// The shared head-on smash must never out-bite the one-sided charge it stacks
/// on, enforced at compile time, so meeting a foe nose-to-nose costs the mutual
/// jolt rather than a bigger hit than you deal back.
const _: () = assert!(HEAD_ON_RAM_DAMAGE_PER_FRAME < AGGRESSOR_RAM_DAMAGE_PER_FRAME);
/// A head-on smash must stay under the earned nitro charge, enforced at compile
/// time, so a boosted ram remains the single hardest source of wear.
const _: () = assert!(HEAD_ON_RAM_DAMAGE_PER_FRAME < NITRO_RAM_DAMAGE_PER_FRAME);
/// Simultaneous enemy cars within ram range for a car to count as pincered.
///
/// A lone attacker is just a ram, already covered by the base scrape and the
/// directional bonuses; it takes a *second* enemy piling on at once to spring the
/// classic Death Rally pincer, a car hemmed in with no lane left to escape.
pub const PINCER_MIN_ATTACKERS: usize = 2;
/// Extra durability a car hemmed in by a pincer of enemies loses each frame.
///
/// The classic Death Rally gang-up: a car surrounded by two or more foes at once
/// cannot escape and is ground down faster than one trading paint with a single
/// enemy. The base [`ram_damage`] scrape charges each car in contact into its own
/// pool, which perversely makes a lone car's team bleed *less* than the pack
/// bracketing it (one scrape against the pack's several); this rights that,
/// bleeding into the surrounded car's *own* pool so being outnumbered at a point
/// is the disadvantage it should be. Needs no aim commitment, only numbers,
/// unlike the aggressor/broadside/rear-end charges, so it is priced below them,
/// yet above the lone base scrape so a pincer always out-bites a solo ram.
pub const PINCER_RAM_DAMAGE_PER_FRAME: f32 = 0.3;
/// A pincer must out-bite a lone scrape, enforced at compile time, so being
/// ganged up on always beats trading paint with a single foe.
const _: () = assert!(PINCER_RAM_DAMAGE_PER_FRAME > RAM_DAMAGE_PER_FRAME);
/// The *minimum* (two-attacker) pincer must not out-bite an aimed charge,
/// enforced at compile time, so a bare gang-up of two stays worth less than a
/// lined-up hit. A larger swarm earns the right to surpass it via
/// [`PINCER_RAM_DAMAGE_PER_EXTRA_ATTACKER`].
const _: () = assert!(PINCER_RAM_DAMAGE_PER_FRAME < AGGRESSOR_RAM_DAMAGE_PER_FRAME);
/// A pincer needs a genuine gang-up, enforced at compile time, so a single
/// attacker never trips it.
const _: () = assert!(PINCER_MIN_ATTACKERS >= 2);
/// Extra durability a pincered car loses each frame for every enemy beyond the
/// [`PINCER_MIN_ATTACKERS`]th piling in at once.
///
/// The base [`ram_damage`] scrape charges the *attacking* team once per attacker,
/// so a three- or four-car swarm makes the attackers' own pool bleed more (three,
/// four scrapes) while the lone victim bled a single flat pincer. A flat pincer
/// therefore only partly rights the outnumbered asymmetry once a third car joins.
/// Scaling the surrounded car's bite with the size of the swarm keeps its penalty
/// in step: the more foes hem it in, the harder it is ground down, the classic
/// Death Rally "they swarmed me" punishment deepening with every extra attacker.
pub const PINCER_RAM_DAMAGE_PER_EXTRA_ATTACKER: f32 = 0.075;
/// Most extra attackers (beyond [`PINCER_MIN_ATTACKERS`]) that still raise a
/// pincer's bite.
///
/// Caps the swarm payday so a huge dogpile cannot deal unbounded wear, mirroring
/// [`WRECK_STREAK_BONUS_CAP`]: attackers past this point still pincer, just at the
/// capped top rate. With the per-extra step this tops a pincer out at
/// [`PINCER_MAX_RAM_DAMAGE_PER_FRAME`].
pub const PINCER_MAX_EXTRA_ATTACKERS: usize = 2;
/// Most durability a single pincered car can lose per frame to the pincer bonus,
/// reached once [`PINCER_MAX_EXTRA_ATTACKERS`] extra foes have piled in.
pub const PINCER_MAX_RAM_DAMAGE_PER_FRAME: f32 =
    pincer_ram_bonus(PINCER_MIN_ATTACKERS + PINCER_MAX_EXTRA_ATTACKERS);
/// The swarm must actually scale the bite, enforced at compile time.
const _: () = assert!(PINCER_RAM_DAMAGE_PER_EXTRA_ATTACKER > 0.0);
/// There must be room for at least one extra attacker to matter, enforced at
/// compile time, so the scaling is never a dead knob.
const _: () = assert!(PINCER_MAX_EXTRA_ATTACKERS >= 1);
/// Even a maxed-out swarm must stay under the earned nitro charge, enforced at
/// compile time, so a boosted ram remains the single hardest source of wear and
/// the swarm bonus stays bounded.
const _: () = assert!(PINCER_MAX_RAM_DAMAGE_PER_FRAME < NITRO_RAM_DAMAGE_PER_FRAME);
/// World-space distance from an arena wall within which a car counts as pinned
/// against it for [`wall_crush_ram_damage`].
///
/// A car's centre clamps to the arena half-extents, so a car shoved up against
/// the invisible boundary sits comfortably inside this band, while open-field
/// trading well away from the edge never trips it.
pub const WALL_CRUSH_MARGIN: f32 = 120.0;
/// Extra durability a car crushed against the arena wall by a charging enemy
/// loses each frame the two are trading paint.
///
/// The classic Death Rally wall slam: shoving a foe up against the arena
/// boundary leaves it nowhere to escape, so the wall plays the part of a second
/// attacker and a lone charging car grinds the pinned victim down. Lands only
/// when the victim is within [`WALL_CRUSH_MARGIN`] of a wall and an enemy is
/// *charging* it (nose-on, the same [`AGGRESSOR_RAM_ALIGNMENT`] commitment the
/// other directional hits demand) from the open side, shoving it into that wall.
/// Bleeds into the victim's *own* team pool on top of the base [`ram_damage`]
/// scrape, mirroring the per-victim model of [`broadside_ram_damage`]: charged
/// once per crushed car however many enemies pile in. Priced as punishingly as a
/// flank T-bone, since a pinned car likewise cannot trade the hit back, above
/// the lone base scrape so a wall pin always out-bites open-field trading, yet
/// kept under the earned [`NITRO_RAM_DAMAGE_PER_FRAME`] so a boosted ram stays
/// the single hardest source of wear.
pub const WALL_CRUSH_RAM_DAMAGE_PER_FRAME: f32 = 0.4;
/// A wall pin must out-bite a lone scrape, enforced at compile time, so being
/// crushed against the boundary always beats trading paint in the open.
const _: () = assert!(WALL_CRUSH_RAM_DAMAGE_PER_FRAME > RAM_DAMAGE_PER_FRAME);
/// A wall pin must stay under the earned nitro charge, enforced at compile time,
/// so a boosted ram remains the hardest single hit a car can land.
const _: () = assert!(WALL_CRUSH_RAM_DAMAGE_PER_FRAME < NITRO_RAM_DAMAGE_PER_FRAME);
/// Extra durability a car wedged into an arena corner by a charging enemy loses
/// each frame, on top of the [`wall_crush_ram_damage`] pin it already eats.
///
/// The classic Death Rally corner trap: a single wall
/// ([`WALL_CRUSH_RAM_DAMAGE_PER_FRAME`]) leaves a pinned car one escape lane left
/// to run along the boundary, but shoving it into the corner where two walls meet
/// seals that lane too, so a lone charger grinds the wholly trapped victim down
/// harder still. The second wall plays the part of a second attacker, the corner
/// the [`pincer_ram_damage`] gang-up no open field can spring. Lands only when the
/// victim sits within [`WALL_CRUSH_MARGIN`] of two perpendicular walls at once and
/// a *charging* enemy (nose-on, the same [`AGGRESSOR_RAM_ALIGNMENT`] commitment the
/// other directional hits demand) shoves it into *both* (see
/// [`is_pinned_in_corner`]). Bleeds into the victim's *own* team pool on top of the
/// single-wall crush the corner already trips, mirroring the per-victim model of
/// [`broadside_ram_damage`]: charged once per cornered car however many enemies pin
/// it. Priced below the first wall's bite, since the second wall only completes a
/// trap the first already sprang, and kept under the earned
/// [`NITRO_RAM_DAMAGE_PER_FRAME`] so a boosted ram stays the single hardest source
/// of wear.
pub const CORNER_CRUSH_RAM_DAMAGE_PER_FRAME: f32 = 0.2;
/// The completing second wall must add a real bite, enforced at compile time.
const _: () = assert!(CORNER_CRUSH_RAM_DAMAGE_PER_FRAME > 0.0);
/// The second wall must add less than the first pin it completes, enforced at
/// compile time, so the corner premium stays a top-up rather than a fresh crush.
const _: () = assert!(CORNER_CRUSH_RAM_DAMAGE_PER_FRAME < WALL_CRUSH_RAM_DAMAGE_PER_FRAME);
/// The corner top-up must stay under the earned nitro charge, enforced at compile
/// time, so a boosted ram remains the hardest single hit a car can land.
const _: () = assert!(CORNER_CRUSH_RAM_DAMAGE_PER_FRAME < NITRO_RAM_DAMAGE_PER_FRAME);
/// Fraction of incoming ram damage a shielded team still takes.
///
/// The defensive counter to the all-offence ramming loop: while a team's shield
/// (from a [`crate::gameplay::pickup::PickupKind::Shield`] pickup) is up, every
/// source of ram wear it would take, base scrape, nitro charge, aggressor hit,
/// even the flag-carrier's own bleed, is halved. Strong enough to turn a losing
/// scrum, short-lived enough (see [`crate::gameplay::pickup::SHIELD_BOOST_FRAMES`])
/// that it is a window to exploit rather than a free pass.
pub const SHIELD_DAMAGE_MULTIPLIER: f32 = 0.5;
/// A shield must actually blunt damage, enforced at compile time.
const _: () = assert!(SHIELD_DAMAGE_MULTIPLIER < 1.0);
/// A shield must not heal (negative damage) or fully negate it, enforced at
/// compile time, so a shielded team can still be worn down with enough pressure.
const _: () = assert!(SHIELD_DAMAGE_MULTIPLIER > 0.0);
/// World-space radius around a team's own home base within which its cars
/// slowly patch up: the home-turf pit zone.
///
/// Matched to [`crate::gameplay::ctf::BASE_CAPTURE_RADIUS`] so the recovery zone
/// is exactly the base footprint a team already fights over, rather than a new
/// area to learn.
pub const BASE_REPAIR_RADIUS: f32 = crate::gameplay::ctf::BASE_CAPTURE_RADIUS;
/// Durability a team regains each frame while one of its cars sits in its own
/// base zone.
///
/// The classic pit-stop recovery and the wreck loop's missing reliable patch-up:
/// a battered team can break off and crawl home to undo ram wear instead of
/// hunting a contested repair pickup. Pitched below the lightest ram
/// ([`RAM_DAMAGE_PER_FRAME`]) so a car still trading paint always nets damage,
/// the heal only bites once a team genuinely disengages to home. Slow enough
/// that a wreck still stings: recovering full durability costs a long stint off
/// the objective, a real tempo price paid in the open while not contesting.
pub const BASE_REPAIR_PER_FRAME: f32 = 0.15;
/// The pit heal must never out-pace even the lightest ram, enforced at compile
/// time, so parking in your base while under fire still loses integrity.
const _: () = assert!(BASE_REPAIR_PER_FRAME < RAM_DAMAGE_PER_FRAME);
/// The pit heal must actually restore durability, enforced at compile time.
const _: () = assert!(BASE_REPAIR_PER_FRAME > 0.0);

/// Whether the given team has any car hauling the enemy flag this frame.
///
/// Read before a wreck knocks flags loose so the carrier-takedown bonus can tell
/// whether the team it just wrecked was actually running a flag home.
#[must_use]
fn team_was_carrying(cars: &[RamCar], team: AiTeam) -> bool {
    cars.iter().any(|car| car.team == team && car.carrying_flag)
}

/// Drops every flag held by a team that was freshly wrecked this frame.
///
/// A spun-out wreck cannot keep its grip on a stolen flag, so the holder of any
/// flag carried by a newly wrecked team is cleared, handing the wrecking team a
/// scramble to recover it. A no-op on frames without a wreck.
fn drop_wrecked_carriers_flags(
    wrecks: WreckEvents,
    car_teams: &[(Entity, AiTeam)],
    flag_query: &mut Query<(Entity, &mut CtfFlag)>,
) {
    if !wrecks.any() {
        return;
    }

    let team_of = |holder: Entity| {
        car_teams
            .iter()
            .find(|(entity, _)| *entity == holder)
            .map(|(_, team)| *team)
    };
    let carried: Vec<CarriedFlag> = flag_query
        .iter()
        .filter_map(|(flag_entity, flag)| {
            Some(CarriedFlag {
                flag: flag_entity,
                carrier_team: team_of(flag.holder?)?,
            })
        })
        .collect();
    let dropped = flags_dropped_by_wrecks(&carried, wrecks);
    for (flag_entity, mut flag) in flag_query.iter_mut() {
        if dropped.contains(&flag_entity) {
            flag.holder = None;
        }
    }
}

/// Logs the full bounty breakdown for any frame that produced a wreck.
///
/// A quiet frame logs nothing; otherwise it attributes every reward each team
/// banked, including the first-blood opening-kill bonus, the payback riposte
/// bonus and the clutch closing-time bonus, so the wreck economy is auditable
/// from the logs alone.
fn log_wreck_bounties(wrecks: WreckEvents, bounties: WreckBounties) {
    if !wrecks.any() {
        return;
    }
    info!(
        "Wreck! player_down={} opponent_down={}; rampage streaks player={} opponent={}; \
         most-wanted bonus player={} opponent={}; carrier-takedown bonus player={} \
         opponent={}; shutdown bonus player={} opponent={}; first-blood bonus player={} \
         opponent={}; payback bonus player={} opponent={}; clutch bonus player={} \
         opponent={}; banking player_bounty={} opponent_bounty={}",
        wrecks.player,
        wrecks.opponent,
        bounties.streaks.player,
        bounties.streaks.opponent,
        bounties.player_most_wanted,
        bounties.opponent_most_wanted,
        bounties.player_carrier_takedown,
        bounties.opponent_carrier_takedown,
        bounties.player_shutdown,
        bounties.opponent_shutdown,
        bounties.player_first_blood,
        bounties.opponent_first_blood,
        bounties.player_payback,
        bounties.opponent_payback,
        bounties.player_clutch,
        bounties.opponent_clutch,
        bounties.player,
        bounties.opponent,
    );
}

/// Wears down both teams whenever their cars are trading paint, and pays a
/// wreck bounty to whichever team grinds an enemy down to zero this frame.
#[allow(clippy::too_many_arguments)]
pub fn ram_damage_system(
    match_result: Option<Res<CtfMatchResult>>,
    match_clock: Option<Res<MatchClock>>,
    captures: Option<Res<CaptureScore>>,
    nitro_boosts: Option<Res<NitroBoosts>>,
    armour_boosts: Option<Res<ArmourBoosts>>,
    mut integrity: ResMut<VehicleIntegrity>,
    mut wreck_streaks: Option<ResMut<WreckStreaks>>,
    mut wreck_stuns: Option<ResMut<WreckStuns>>,
    mut wreck_surges: Option<ResMut<WreckSurges>>,
    mut first_blood: Option<ResMut<FirstBloodClaimed>>,
    mut payback_windows: Option<ResMut<PaybackWindows>>,
    mut score: Option<ResMut<Score>>,
    mut opponent_score: Option<ResMut<OpponentScore>>,
    player_query: Query<(Entity, &Transform), With<Player>>,
    virtual_player_query: Query<(Entity, &VirtualPlayer, &Transform), Without<Player>>,
    mut flag_query: Query<(Entity, &mut CtfFlag)>,
) {
    if match_result
        .as_ref()
        .is_some_and(|result| result.winner.is_some())
    {
        return;
    }

    let carriers: Vec<Entity> = flag_query
        .iter()
        .filter_map(|(_, flag)| flag.holder)
        .collect();
    let is_carrying = |entity: Entity| carriers.contains(&entity);

    let mut cars: Vec<RamCar> = Vec::new();
    // Maps each car's entity to its team so a wreck can find the flags it drops.
    let mut car_teams: Vec<(Entity, AiTeam)> = Vec::new();
    if let Ok((entity, transform)) = player_query.get_single() {
        cars.push(RamCar {
            team: AiTeam::Blue,
            position: transform.translation.xy(),
            forward: (transform.rotation * Vec3::Y).xy(),
            carrying_flag: is_carrying(entity),
        });
        car_teams.push((entity, AiTeam::Blue));
    }
    for (entity, virtual_player, transform) in &virtual_player_query {
        cars.push(RamCar {
            team: virtual_player.team,
            position: transform.translation.xy(),
            forward: (transform.rotation * Vec3::Y).xy(),
            carrying_flag: is_carrying(entity),
        });
        car_teams.push((entity, virtual_player.team));
    }

    let boost = nitro_boosts
        .as_deref()
        .map(RamBoost::from_nitro)
        .unwrap_or_default();
    let shield = armour_boosts
        .as_deref()
        .map(RamShield::from_armour)
        .unwrap_or_default();
    // Read the surge state *before* this frame's wreck is resolved, so only a
    // surge earned by a *prior* kill bites this frame: a team that wrecks an enemy
    // this frame surges from the next one on.
    let surge = wreck_surges
        .as_deref()
        .copied()
        .map(RamSurge::from_surges)
        .unwrap_or_default();
    let damage = frame_ram_damage(&cars, boost, surge, shield, BOUNDS / 2.0);

    let before = *integrity;
    integrity.apply_damage(damage);
    let wrecks = integrity.newly_wrecked(before);

    // Read the payback windows *before* this frame's wreck opens any new ones, so a
    // payback only ever answers a *prior* wreck: a side wrecked this same frame is
    // owed nothing yet, making a wreck-for-wreck trade a double wreck, not a riposte.
    let payback_before = payback_windows.as_deref().copied().unwrap_or_default();

    // A freshly wrecked team spins out: stagger its cars for a brief window so
    // the wrecking team gets a real opening to capitalise.
    if let Some(stuns) = wreck_stuns.as_deref_mut() {
        stuns.apply_wrecks(wrecks);
    }

    // A freshly wrecked team is owed a payback: open its window so a wreck it lands
    // back inside the next window banks the retaliation bonus.
    if let Some(windows) = payback_windows.as_deref_mut() {
        windows.apply_wrecks(wrecks);
    }

    // The team that dealt the wreck surges: a short burst of speed, the mirror
    // of the victim's spin-out, so the kill opens a clean window to exploit.
    if let Some(surges) = wreck_surges.as_deref_mut() {
        surges.reward_wreckers(wrecks);
    }

    // A spun-out wreck cannot keep its grip on a stolen flag: drop every flag a
    // freshly wrecked team was hauling so the wrecking team can scramble to
    // recover it.
    drop_wrecked_carriers_flags(wrecks, &car_teams, &mut flag_query);

    // Resolve every cash reward this frame's wrecks pay: the rampage streak
    // payout, the most-wanted leader bonus, and the carrier-takedown bonus. The
    // carrying flags were read into `cars` before the wreck knocked them loose,
    // so they still reflect who was actually hauling when the wreck landed.
    let before_streaks = wreck_streaks.as_deref().copied().unwrap_or_default();
    let captures = captures.as_deref().copied().unwrap_or_default();
    // First blood is the round's opening wreck: available only while its latch is
    // present and unclaimed. An absent latch leaves the bonus off, mirroring how
    // every other optional combat resource degrades when missing.
    let first_blood_available = first_blood.as_deref().is_some_and(|claimed| !claimed.0);
    // A clutch wreck pays extra in the round's closing stretch. An absent clock
    // leaves the bonus off, mirroring how every other optional CTF resource
    // degrades when missing.
    let closing_time = match_clock
        .as_deref()
        .is_some_and(|clock| clock.is_closing_time());
    let bounties = resolve_wreck_bounties(
        before_streaks,
        wrecks,
        captures,
        WreckCarriers {
            player: team_was_carrying(&cars, AiTeam::Blue),
            opponent: team_was_carrying(&cars, AiTeam::Red),
        },
        first_blood_available,
        payback_before,
        closing_time,
    );
    if let Some(streaks) = wreck_streaks.as_deref_mut() {
        *streaks = bounties.streaks;
    }
    // Spend the round's first blood the frame it is drawn, so it pays exactly once.
    if let Some(claimed) = first_blood.as_deref_mut() {
        claimed.0 |= first_blood_available && wrecks.any();
    }

    log_wreck_bounties(wrecks, bounties);

    // The wrecking team banks the bounty: a wrecked opponent pays the player
    // team, a wrecked player team pays the opponents.
    if bounties.player > 0 {
        if let Some(score) = score.as_deref_mut() {
            score.bank_wreck_bounty(bounties.player);
        }
    }
    if bounties.opponent > 0 {
        if let Some(opponent_score) = opponent_score.as_deref_mut() {
            opponent_score.bank_wreck_bounty(bounties.opponent);
        }
    }
}

/// Patches up any team that has retreated to its own base this frame.
///
/// The pit-stop recovery: a battered team can break off and crawl home to undo
/// ram wear, a reliable alternative to a contested repair pickup. Each team's
/// home base is read from its flag; a resolved match is skipped so a decided
/// round stays frozen, and a frame missing either flag heals no one.
pub fn base_repair_system(
    match_result: Option<Res<CtfMatchResult>>,
    mut integrity: ResMut<VehicleIntegrity>,
    player_query: Query<&Transform, With<Player>>,
    virtual_player_query: Query<(&VirtualPlayer, &Transform), Without<Player>>,
    flag_query: Query<&CtfFlag>,
) {
    if match_result
        .as_ref()
        .is_some_and(|result| result.winner.is_some())
    {
        return;
    }

    let Some((blue_home, red_home)) = team_home_bases(&flag_query) else {
        return;
    };

    let mut cars: Vec<(AiTeam, Vec2)> = Vec::new();
    if let Ok(transform) = player_query.get_single() {
        cars.push((AiTeam::Blue, transform.translation.xy()));
    }
    for (virtual_player, transform) in &virtual_player_query {
        cars.push((virtual_player.team, transform.translation.xy()));
    }

    integrity.apply_base_repair(base_repair(&cars, blue_home, red_home));
}

/// Reads each team's home base from its flag, returning `None` until both flags
/// are present so a half-loaded arena never heals against a stale base.
fn team_home_bases(flag_query: &Query<&CtfFlag>) -> Option<(Vec2, Vec2)> {
    let mut blue_home = None;
    let mut red_home = None;
    for flag in flag_query {
        match flag.team {
            FlagTeam::Blue => blue_home = Some(flag.home),
            FlagTeam::Red => red_home = Some(flag.home),
        }
    }
    Some((blue_home?, red_home?))
}

fn reset_vehicle_integrity(mut integrity: ResMut<VehicleIntegrity>) {
    *integrity = VehicleIntegrity::default();
}

fn reset_wreck_streaks(mut streaks: ResMut<WreckStreaks>) {
    *streaks = WreckStreaks::default();
}

fn reset_wreck_stuns(mut stuns: ResMut<WreckStuns>) {
    *stuns = WreckStuns::default();
}

fn reset_wreck_surges(mut surges: ResMut<WreckSurges>) {
    *surges = WreckSurges::default();
}

fn reset_first_blood(mut first_blood: ResMut<FirstBloodClaimed>) {
    *first_blood = FirstBloodClaimed::default();
}

fn reset_payback_windows(mut windows: ResMut<PaybackWindows>) {
    *windows = PaybackWindows::default();
}

/// Winds every team's wreck spin-out down by one frame.
///
/// Runs before [`ram_damage_system`] each frame so a spin-out triggered this
/// frame keeps its full [`WRECK_STUN_FRAMES`] window before the next tick.
fn wreck_stun_decay_system(mut stuns: ResMut<WreckStuns>) {
    stuns.tick();
}

/// Winds every team's wreck surge down by one frame.
///
/// Runs before [`ram_damage_system`] each frame so a surge triggered this frame
/// keeps its full [`WRECK_SURGE_FRAMES`] window before the next tick.
fn wreck_surge_decay_system(mut surges: ResMut<WreckSurges>) {
    surges.tick();
}

/// Winds every team's payback window down by one frame.
///
/// Runs before [`ram_damage_system`] each frame so a window opened this frame
/// keeps its full [`PAYBACK_WINDOW_FRAMES`] before the next tick, mirroring the
/// spin-out and surge decay.
fn payback_window_decay_system(mut windows: ResMut<PaybackWindows>) {
    windows.tick();
}

#[derive(Default)]
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VehicleIntegrity>()
            .init_resource::<WreckStreaks>()
            .init_resource::<WreckStuns>()
            .init_resource::<WreckSurges>()
            .init_resource::<FirstBloodClaimed>()
            .init_resource::<PaybackWindows>()
            .add_system_set(
                SystemSet::on_enter(AppState::InGame)
                    .with_system(reset_vehicle_integrity)
                    .with_system(reset_wreck_streaks)
                    .with_system(reset_wreck_stuns)
                    .with_system(reset_wreck_surges)
                    .with_system(reset_first_blood)
                    .with_system(reset_payback_windows),
            )
            .add_system(wreck_stun_decay_system.before(ram_damage_system))
            .add_system(wreck_surge_decay_system.before(ram_damage_system))
            .add_system(payback_window_decay_system.before(ram_damage_system))
            .add_system(ram_damage_system)
            // Pit recovery runs after the frame's wear is settled, so a battered
            // car that has just disengaged to home patches up against its
            // post-damage integrity rather than racing the scrape.
            .add_system(base_repair_system.after(ram_damage_system));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 1e-4,
            "actual={actual}, expected={expected}"
        );
    }

    #[test]
    fn combined_sums_both_teams_damage() {
        let total = TeamDamage {
            player: 0.25,
            opponent: 0.5,
        }
        .combined(TeamDamage {
            player: 1.0,
            opponent: 0.25,
        });
        assert_near(total.player, 1.25);
        assert_near(total.opponent, 0.75);
    }

    #[test]
    fn system_adds_pincer_ram_bonus_when_two_cars_gang_up() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        app.add_system(ram_damage_system);
        // The player and both reds keep their default +Y facing, placed along the
        // X-axis so no car charges another: only the base scrape and the pincer
        // land, never the aggressor/broadside/rear-end bonuses.
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(-30.0, 0.0, 0.0)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        // The player is bracketed by two reds, so it eats the base scrape and the
        // pincer bonus on top; each red faces only the lone player, so the
        // opponents take a base scrape per car and no pincer.
        assert_near(
            integrity.player,
            MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME - PINCER_RAM_DAMAGE_PER_FRAME,
        );
        // Each of the two reds takes a base scrape from the lone player.
        assert_near(
            integrity.opponent,
            MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME - RAM_DAMAGE_PER_FRAME,
        );
    }

    #[test]
    fn system_adds_rear_end_ram_bonus_on_a_tail_charge() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        app.add_system(ram_damage_system);
        // The player keeps its default +Y facing, exposing its tail along -Y.
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        // The red car chases from directly behind, keeping its own +Y facing so
        // its nose is on the player's tail.
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(0.0, -30.0, 0.0)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        // Base scrape wears both teams 0.25; a tail charge is necessarily an
        // aggressor charge too (the chaser's nose is on the victim), so the
        // player eats the aggressor charge and the rear-end bonus on top, while
        // the chaser only takes the base scrape back.
        assert_near(
            integrity.player,
            MAX_INTEGRITY
                - RAM_DAMAGE_PER_FRAME
                - AGGRESSOR_RAM_DAMAGE_PER_FRAME
                - REAR_END_RAM_DAMAGE_PER_FRAME,
        );
        assert_near(integrity.opponent, MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn system_adds_broadside_ram_bonus_on_a_flank_charge() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        app.add_system(ram_damage_system);
        // The player keeps its default +Y facing, exposing its flank along +X.
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        // The red car sits on the player's flank and charges -X into its door
        // (a quarter-turn from its default +Y heading).
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0))
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        // Base scrape wears both teams 0.25; the red charges the player's exposed
        // flank, so the player also eats the aggressor charge and the broadside
        // bonus on top, while the red car only takes the base scrape back.
        assert_near(
            integrity.player,
            MAX_INTEGRITY
                - RAM_DAMAGE_PER_FRAME
                - AGGRESSOR_RAM_DAMAGE_PER_FRAME
                - BROADSIDE_RAM_DAMAGE_PER_FRAME,
        );
        assert_near(integrity.opponent, MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn system_adds_aggressor_ram_bonus_when_a_car_charges() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        app.add_system(ram_damage_system);
        // Nose to nose: the player charges +X into the red car while the red car
        // charges -X straight back. Each strikes the other dead on the front, so
        // both eat the aggressor charge but neither the flank nor the rear bonus.
        app.world.spawn((
            player_stub(),
            Transform::from_translation(Vec3::ZERO)
                .with_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)),
        ));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0))
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        // Base scrape wears both teams 0.25; a nose-to-nose meeting adds the
        // mutual aggressor charge and the shared head-on smash to each, while a
        // dead-on hit triggers neither the flank nor the rear bonus.
        assert_near(
            integrity.player,
            MAX_INTEGRITY
                - RAM_DAMAGE_PER_FRAME
                - AGGRESSOR_RAM_DAMAGE_PER_FRAME
                - HEAD_ON_RAM_DAMAGE_PER_FRAME,
        );
        assert_near(
            integrity.opponent,
            MAX_INTEGRITY
                - RAM_DAMAGE_PER_FRAME
                - AGGRESSOR_RAM_DAMAGE_PER_FRAME
                - HEAD_ON_RAM_DAMAGE_PER_FRAME,
        );
    }

    #[test]
    fn system_adds_carrier_ram_bonus_when_a_carrier_collides() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        app.add_system(ram_damage_system);
        let carrier = app
            .world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)))
            .id();
        // The blue carrier hauls the red flag, held by the human player.
        app.world.spawn((
            CtfFlag {
                team: crate::gameplay::ctf::FlagTeam::Red,
                home: Vec2::new(500.0, 0.0),
                holder: Some(carrier),
            },
            Transform::from_translation(Vec3::ZERO),
        ));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        // Base scrape wears both teams 0.25; the blue carrier also bleeds the
        // carrier tax on top because the red defender is trading paint with it.
        assert_near(
            integrity.player,
            MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME - FLAG_CARRIER_RAM_DAMAGE_PER_FRAME,
        );
        assert_near(integrity.opponent, MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn system_drops_the_flag_when_a_carrier_team_is_wrecked() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            // One frame of the base scrape (0.25) plus the carrier tax (0.5)
            // grinds the player team to a wreck.
            player: 0.2,
            opponent: MAX_INTEGRITY,
        });
        app.add_system(ram_damage_system);
        let carrier = app
            .world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)))
            .id();
        // The blue carrier hauls the red flag, held by the human player.
        let flag = app
            .world
            .spawn((
                CtfFlag {
                    team: crate::gameplay::ctf::FlagTeam::Red,
                    home: Vec2::new(500.0, 0.0),
                    holder: Some(carrier),
                },
                Transform::from_translation(Vec3::ZERO),
            ))
            .id();
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        assert_near(app.world.resource::<VehicleIntegrity>().player, 0.0);
        assert_eq!(
            app.world.get::<CtfFlag>(flag).unwrap().holder,
            None,
            "a wrecked carrier must drop the flag it was hauling"
        );
    }

    #[test]
    fn system_keeps_the_flag_with_an_operational_carrier() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        app.add_system(ram_damage_system);
        let carrier = app
            .world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)))
            .id();
        let flag = app
            .world
            .spawn((
                CtfFlag {
                    team: crate::gameplay::ctf::FlagTeam::Red,
                    home: Vec2::new(500.0, 0.0),
                    holder: Some(carrier),
                },
                Transform::from_translation(Vec3::ZERO),
            ))
            .id();
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        assert_eq!(
            app.world.get::<CtfFlag>(flag).unwrap().holder,
            Some(carrier),
            "an operational carrier must keep its grip on the flag"
        );
    }

    const BLUE_HOME: Vec2 = Vec2::new(-500.0, 0.0);
    const RED_HOME: Vec2 = Vec2::new(500.0, 0.0);

    fn spawn_base_flags(app: &mut App) {
        app.world.spawn((
            CtfFlag {
                team: FlagTeam::Blue,
                home: BLUE_HOME,
                holder: None,
            },
            Transform::from_translation(BLUE_HOME.extend(0.0)),
        ));
        app.world.spawn((
            CtfFlag {
                team: FlagTeam::Red,
                home: RED_HOME,
                holder: None,
            },
            Transform::from_translation(RED_HOME.extend(0.0)),
        ));
    }

    #[test]
    fn system_patches_up_a_battered_team_parked_in_its_base() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: 20.0,
            opponent: MAX_INTEGRITY,
        });
        app.add_system(base_repair_system);
        spawn_base_flags(&mut app);
        // The battered human sits on the blue base.
        app.world.spawn((
            player_stub(),
            Transform::from_translation(BLUE_HOME.extend(0.0)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        assert_near(integrity.player, 20.0 + BASE_REPAIR_PER_FRAME);
        // No red car is home, so the opponent earns no pit recovery.
        assert_near(integrity.opponent, MAX_INTEGRITY);
    }

    #[test]
    fn system_leaves_a_team_fighting_in_the_field_unhealed() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: 20.0,
            opponent: MAX_INTEGRITY,
        });
        app.add_system(base_repair_system);
        spawn_base_flags(&mut app);
        // The player is out in midfield, far from its base.
        app.world.spawn((
            player_stub(),
            Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
        ));

        app.update();

        assert_near(app.world.resource::<VehicleIntegrity>().player, 20.0);
    }

    #[test]
    fn system_heals_a_red_virtual_player_on_its_own_base() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            opponent: 20.0,
        });
        app.add_system(base_repair_system);
        spawn_base_flags(&mut app);
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(RED_HOME.extend(0.0)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        assert_near(integrity.opponent, 20.0 + BASE_REPAIR_PER_FRAME);
        assert_near(integrity.player, MAX_INTEGRITY);
    }

    #[test]
    fn system_does_not_patch_up_after_the_match_resolves() {
        use crate::gameplay::ctf::CtfMatchWinner;
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: 20.0,
            opponent: MAX_INTEGRITY,
        });
        app.insert_resource(CtfMatchResult {
            winner: Some(CtfMatchWinner::Player),
        });
        app.add_system(base_repair_system);
        spawn_base_flags(&mut app);
        app.world.spawn((
            player_stub(),
            Transform::from_translation(BLUE_HOME.extend(0.0)),
        ));

        app.update();

        assert_near(app.world.resource::<VehicleIntegrity>().player, 20.0);
    }

    #[test]
    fn system_heals_no_one_when_a_base_flag_is_missing() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: 20.0,
            opponent: MAX_INTEGRITY,
        });
        app.add_system(base_repair_system);
        // Only the blue flag exists: without both bases the system bails out.
        app.world.spawn((
            CtfFlag {
                team: FlagTeam::Blue,
                home: BLUE_HOME,
                holder: None,
            },
            Transform::from_translation(BLUE_HOME.extend(0.0)),
        ));
        app.world.spawn((
            player_stub(),
            Transform::from_translation(BLUE_HOME.extend(0.0)),
        ));

        app.update();

        assert_near(app.world.resource::<VehicleIntegrity>().player, 20.0);
    }

    #[test]
    fn system_adds_nitro_ram_bonus_when_a_boosted_team_collides() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        let mut boosts = NitroBoosts::default();
        boosts.trigger_opponent();
        app.insert_resource(boosts);
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        // Base scrape wears both teams 0.25; the boosted reds also ram the
        // player team for the nitro bonus on top.
        assert_near(
            integrity.player,
            MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME - NITRO_RAM_DAMAGE_PER_FRAME,
        );
        assert_near(integrity.opponent, MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn system_adds_surge_ram_bonus_when_a_surging_team_collides() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        // The reds are surging from a prior wreck (set up before this frame), so
        // their ongoing scrape with the player bites for the surge bonus on top.
        let mut surges = WreckSurges::default();
        surges.trigger_opponent();
        app.insert_resource(surges);
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        // Base scrape wears both teams 0.25; the surging reds also ram the player
        // team for the surge bonus on top.
        assert_near(
            integrity.player,
            MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME - SURGE_RAM_DAMAGE_PER_FRAME,
        );
        assert_near(integrity.opponent, MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn a_freshly_wrecking_team_surges_into_a_harder_next_ram() {
        // The wreck -> surge -> chain loop end to end: the opponents grind a lone
        // player car to a wreck this frame and, because they were *already* surging
        // from a prior kill, the same frame's scrape on the player also lands the
        // surge bite. Proves the surge a prior kill earned bites the next contact.
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: RAM_DAMAGE_PER_FRAME + SURGE_RAM_DAMAGE_PER_FRAME,
            opponent: MAX_INTEGRITY,
        });
        let mut surges = WreckSurges::default();
        surges.trigger_opponent();
        app.insert_resource(surges);
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        assert_near(integrity.player, 0.0);
    }

    #[test]
    fn system_halves_ram_damage_for_a_shielded_team() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        let mut armour = ArmourBoosts::default();
        armour.trigger_player();
        app.insert_resource(armour);
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        // The shielded player team eats only half the base scrape; the
        // unshielded opponents take it in full.
        assert_near(
            integrity.player,
            RAM_DAMAGE_PER_FRAME.mul_add(-SHIELD_DAMAGE_MULTIPLIER, MAX_INTEGRITY),
        );
        assert_near(integrity.opponent, MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn system_shield_blunts_every_ram_source_at_once() {
        // Reds are boosting (a nitro-ram bonus on the player) and the player is
        // shielded: the player should eat half of base + nitro combined, proving
        // the shield mitigates the whole frame's damage, not just the base scrape.
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        let mut boosts = NitroBoosts::default();
        boosts.trigger_opponent();
        app.insert_resource(boosts);
        let mut armour = ArmourBoosts::default();
        armour.trigger_player();
        app.insert_resource(armour);
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        assert_near(
            integrity.player,
            (RAM_DAMAGE_PER_FRAME + NITRO_RAM_DAMAGE_PER_FRAME)
                .mul_add(-SHIELD_DAMAGE_MULTIPLIER, MAX_INTEGRITY),
        );
        assert_near(integrity.opponent, MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn system_wears_down_both_teams_when_cars_collide() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        assert_near(integrity.player, MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME);
        assert_near(integrity.opponent, MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn system_pays_the_player_team_a_bounty_for_wrecking_an_opponent() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            // One frame of the base scrape (0.25) tips this to zero.
            opponent: 0.2,
        });
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        assert_near(app.world.resource::<VehicleIntegrity>().opponent, 0.0);
        let score = app.world.resource::<Score>();
        assert_eq!(score.cash, WRECK_CASH_BOUNTY);
        assert_eq!(score.wrecks, 1);
        // The wrecking team earns nothing for the enemy: opponents stay empty.
        assert_eq!(app.world.resource::<OpponentScore>().wrecks, 0);
    }

    #[test]
    fn system_pays_the_opponents_a_bounty_for_wrecking_the_player_team() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: 0.2,
            opponent: MAX_INTEGRITY,
        });
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        assert_near(app.world.resource::<VehicleIntegrity>().player, 0.0);
        let opponent_score = app.world.resource::<OpponentScore>();
        assert_eq!(opponent_score.cash, WRECK_CASH_BOUNTY);
        assert_eq!(opponent_score.wrecks, 1);
        assert_eq!(app.world.resource::<Score>().wrecks, 0);
    }

    #[test]
    fn system_pays_a_clutch_bonus_for_a_closing_time_wreck() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            // One frame of the base scrape (0.25) tips this to zero.
            opponent: 0.2,
        });
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        // Sudden death is always closing time, so the wreck lands a clutch bonus.
        let mut clock = MatchClock::default();
        clock.enter_sudden_death();
        app.insert_resource(clock);
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let score = app.world.resource::<Score>();
        assert_eq!(
            score.cash,
            WRECK_CASH_BOUNTY + CLUTCH_WRECK_CASH_BONUS,
            "a wreck in closing time banks the clutch bonus on top of the base bounty"
        );
        assert_eq!(score.wrecks, 1, "the clutch bonus rides the same wreck");
    }

    #[test]
    fn system_pays_no_clutch_bonus_outside_closing_time() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            opponent: 0.2,
        });
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        // A fresh regulation clock is far from its closing stretch, so an early
        // wreck pays only the base bounty.
        app.insert_resource(MatchClock::default());
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        assert_eq!(
            app.world.resource::<Score>().cash,
            WRECK_CASH_BOUNTY,
            "a wreck before closing time banks no clutch bonus"
        );
    }

    #[test]
    fn system_draws_first_blood_for_the_opening_wreck() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            // One frame of the base scrape (0.25) tips this to zero.
            opponent: 0.2,
        });
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<FirstBloodClaimed>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let score = app.world.resource::<Score>();
        assert_eq!(
            score.cash,
            WRECK_CASH_BOUNTY + FIRST_BLOOD_CASH_BONUS,
            "the round's opening wreck banks first blood on top of the base bounty"
        );
        assert_eq!(score.wrecks, 1, "first blood rides the same wreck");
        assert!(
            app.world.resource::<FirstBloodClaimed>().0,
            "drawing first blood spends it for the rest of the round"
        );
    }

    #[test]
    fn system_draws_first_blood_only_once_per_round() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            opponent: 0.2,
        });
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        // First blood has already been drawn earlier this round.
        app.insert_resource(FirstBloodClaimed(true));
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        assert_eq!(
            app.world.resource::<Score>().cash,
            WRECK_CASH_BOUNTY,
            "a wreck after first blood is spent pays only the base bounty"
        );
    }

    #[test]
    fn entering_a_match_clears_first_blood_for_the_new_round() {
        let mut app = App::new();
        app.insert_resource(FirstBloodClaimed(true));
        app.add_system(reset_first_blood);

        app.update();

        assert!(
            !app.world.resource::<FirstBloodClaimed>().0,
            "a fresh round must put first blood back up for grabs"
        );
    }

    #[test]
    fn system_pays_a_payback_bonus_for_a_retaliation_wreck() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            // One frame of the base scrape (0.25) tips this to zero.
            opponent: 0.2,
        });
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        // The player team was wrecked moments ago, so it is still owed a riposte.
        app.insert_resource(PaybackWindows {
            player_frames: PAYBACK_WINDOW_FRAMES,
            opponent_frames: 0,
        });
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let score = app.world.resource::<Score>();
        assert_eq!(
            score.cash,
            WRECK_CASH_BOUNTY + PAYBACK_CASH_BONUS,
            "wrecking an enemy while owed a riposte banks payback on top of the base bounty"
        );
        assert_eq!(score.wrecks, 1, "the payback rides the same wreck");
    }

    #[test]
    fn system_pays_no_payback_without_a_prior_wreck() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            opponent: 0.2,
        });
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        // No one has been wrecked yet this round, so no payback is owed.
        app.init_resource::<PaybackWindows>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        assert_eq!(
            app.world.resource::<Score>().cash,
            WRECK_CASH_BOUNTY,
            "a kill by a team not recently wrecked pays only the base bounty"
        );
    }

    #[test]
    fn system_opens_a_payback_window_when_a_team_is_wrecked() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            // One frame of the base scrape (0.25) tips the player team to zero.
            player: 0.2,
            opponent: MAX_INTEGRITY,
        });
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<PaybackWindows>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let windows = app.world.resource::<PaybackWindows>();
        assert_eq!(
            windows.player_frames, PAYBACK_WINDOW_FRAMES,
            "the wrecked player team is owed a fresh riposte window"
        );
        assert!(
            !windows.is_opponent_live(),
            "the team that landed the wreck is owed no payback"
        );
    }

    #[test]
    fn system_pays_no_payback_on_the_very_frame_a_team_is_wrecked() {
        // A simultaneous wreck-for-wreck trade is a double wreck, not a riposte:
        // neither side was owed a payback entering the frame, so neither banks one
        // even though both deal a wreck and both windows open afterwards.
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: 0.2,
            opponent: 0.2,
        });
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.init_resource::<PaybackWindows>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        assert_eq!(
            app.world.resource::<Score>().cash,
            WRECK_CASH_BOUNTY,
            "a same-frame trade pays only the base bounty, no payback"
        );
        assert_eq!(
            app.world.resource::<OpponentScore>().cash,
            WRECK_CASH_BOUNTY,
            "neither side was owed a riposte entering the frame"
        );
        let windows = app.world.resource::<PaybackWindows>();
        assert!(
            windows.is_player_live() && windows.is_opponent_live(),
            "both freshly wrecked teams are now owed a riposte"
        );
    }

    #[test]
    fn entering_a_match_clears_payback_windows_for_the_new_round() {
        let mut app = App::new();
        app.insert_resource(PaybackWindows {
            player_frames: PAYBACK_WINDOW_FRAMES,
            opponent_frames: PAYBACK_WINDOW_FRAMES,
        });
        app.add_system(reset_payback_windows);

        app.update();

        assert_eq!(
            *app.world.resource::<PaybackWindows>(),
            PaybackWindows::default(),
            "a fresh round must wipe every outstanding riposte"
        );
    }

    #[test]
    fn payback_window_decay_winds_every_window_down_a_frame() {
        let mut app = App::new();
        app.insert_resource(PaybackWindows {
            player_frames: 2,
            opponent_frames: 1,
        });
        app.add_system(payback_window_decay_system);

        app.update();

        let windows = app.world.resource::<PaybackWindows>();
        assert_eq!(windows.player_frames, 1);
        assert_eq!(
            windows.opponent_frames, 0,
            "a window saturates at zero, never underflowing"
        );
    }

    #[test]
    fn system_pays_a_most_wanted_bonus_for_wrecking_the_capture_leader() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            // One frame of the base scrape (0.25) tips this to zero.
            opponent: 0.2,
        });
        // The opponents lead the round by two captures: a price on their head.
        app.insert_resource(CaptureScore {
            player: 0,
            opponents: 2,
        });
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let score = app.world.resource::<Score>();
        assert_eq!(
            score.cash,
            WRECK_CASH_BOUNTY + most_wanted_wreck_bonus(2, 0),
            "wrecking the two-capture leader must add the most-wanted comeback bonus"
        );
        assert_eq!(
            score.wrecks, 1,
            "the comeback bonus rides the same wreck, not a phantom second one"
        );
    }

    #[test]
    fn system_pays_no_most_wanted_bonus_for_wrecking_a_trailing_team() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            opponent: 0.2,
        });
        // The player team is the one ahead, so wrecking the trailing opponents
        // earns only the base bounty: no comeback cash for the side already up.
        app.insert_resource(CaptureScore {
            player: 2,
            opponents: 0,
        });
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let score = app.world.resource::<Score>();
        assert_eq!(
            score.cash, WRECK_CASH_BOUNTY,
            "the leader earns no comeback bonus for wrecking the team chasing it"
        );
        assert_eq!(score.wrecks, 1);
    }

    #[test]
    fn system_pays_a_carrier_takedown_bonus_for_wrecking_a_flag_carrier() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            // One frame of the base scrape (0.25) tips the carrier to a wreck.
            opponent: 0.2,
        });
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        // The red car is hauling the blue flag, so wrecking it both denies the
        // capture and forces a turnover: the marquee defensive takedown.
        let carrier = app
            .world
            .spawn((
                virtual_player_stub(AiTeam::Red),
                Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
            ))
            .id();
        app.world.spawn((
            CtfFlag {
                team: crate::gameplay::ctf::FlagTeam::Blue,
                home: Vec2::new(-500.0, 0.0),
                holder: Some(carrier),
            },
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let score = app.world.resource::<Score>();
        assert_eq!(
            score.cash,
            WRECK_CASH_BOUNTY + carrier_takedown_wreck_bonus(true),
            "wrecking the enemy flag carrier must add the carrier-takedown bonus"
        );
        assert_eq!(
            score.wrecks, 1,
            "the takedown bonus rides the same wreck, not a phantom second one"
        );
    }

    #[test]
    fn system_pays_no_carrier_takedown_bonus_for_wrecking_an_empty_handed_car() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            opponent: 0.2,
        });
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        // The red car carries nothing, so wrecking it pays only the base bounty
        // even though a loose flag sits elsewhere on the board.
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));
        app.world.spawn((
            CtfFlag {
                team: crate::gameplay::ctf::FlagTeam::Blue,
                home: Vec2::new(-500.0, 0.0),
                holder: None,
            },
            Transform::from_translation(Vec3::new(-500.0, 0.0, 0.0)),
        ));

        app.update();

        let score = app.world.resource::<Score>();
        assert_eq!(
            score.cash, WRECK_CASH_BOUNTY,
            "wrecking an empty-handed car must not earn the carrier-takedown bonus"
        );
        assert_eq!(score.wrecks, 1);
    }

    #[test]
    fn system_pays_a_shutdown_bonus_for_ending_an_enemy_rampage() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            // One frame of the base scrape (0.25) tips the opponent to a wreck.
            opponent: 0.2,
        });
        // The opponents are three wrecks deep into a rampage: a price on the
        // dangerous driver's head that the player team collects by ending the run.
        app.insert_resource(WreckStreaks {
            player: 0,
            opponent: 3,
        });
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let score = app.world.resource::<Score>();
        assert_eq!(
            score.cash,
            WRECK_CASH_BOUNTY + shutdown_wreck_bonus(3),
            "ending the opponents' rampage must add the shutdown bounty"
        );
        assert_eq!(
            score.wrecks, 1,
            "the shutdown bonus rides the same wreck, not a phantom second one"
        );
        assert_eq!(
            app.world.resource::<WreckStreaks>().opponent,
            0,
            "the wreck resets the rampage it ended"
        );
    }

    #[test]
    fn system_pays_no_bounty_while_both_teams_stay_operational() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        assert_eq!(app.world.resource::<Score>().wrecks, 0);
        assert_eq!(app.world.resource::<OpponentScore>().wrecks, 0);
    }

    #[test]
    fn system_pays_a_wreck_bounty_only_once_until_the_team_recovers() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            opponent: 0.2,
        });
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        // First frame wrecks the opponent and pays the bounty once.
        app.update();
        // Second frame: the opponent is still flat-lined and in contact, so the
        // bounty must not pay again until a repair lifts them off zero.
        app.update();

        let score = app.world.resource::<Score>();
        assert_eq!(score.cash, WRECK_CASH_BOUNTY);
        assert_eq!(score.wrecks, 1);
    }

    #[test]
    fn system_leaves_integrity_alone_once_the_match_is_decided() {
        use crate::gameplay::ctf::CtfMatchWinner;
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        app.insert_resource(CtfMatchResult {
            winner: Some(CtfMatchWinner::Player),
        });
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        assert_near(integrity.player, MAX_INTEGRITY);
        assert_near(integrity.opponent, MAX_INTEGRITY);
    }

    #[test]
    fn entering_match_resets_integrity_to_full() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: 12.0,
            opponent: 34.0,
        });
        app.add_system(reset_vehicle_integrity);

        app.update();

        assert_eq!(
            *app.world.resource::<VehicleIntegrity>(),
            VehicleIntegrity::default()
        );
    }

    #[test]
    fn system_escalates_the_wreck_bounty_across_a_rampage() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            opponent: 0.2,
        });
        app.init_resource::<WreckStreaks>();
        app.init_resource::<Score>();
        app.init_resource::<OpponentScore>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        // First wreck pays the base bounty and opens the rampage.
        app.update();
        assert_eq!(app.world.resource::<WreckStreaks>().player, 1);
        assert_eq!(app.world.resource::<Score>().cash, WRECK_CASH_BOUNTY);

        // The opponent limps back from a repair and is wrecked anew: the second
        // wreck in the rampage pays more than the first.
        app.world.resource_mut::<VehicleIntegrity>().opponent = 0.2;
        app.update();

        let score = app.world.resource::<Score>();
        assert_eq!(app.world.resource::<WreckStreaks>().player, 2);
        assert_eq!(
            score.cash,
            wreck_bounty_for_streak(1) + wreck_bounty_for_streak(2)
        );
        assert_eq!(score.wrecks, 2);
    }

    #[test]
    fn entering_match_resets_wreck_streaks() {
        let mut app = App::new();
        app.insert_resource(WreckStreaks {
            player: 3,
            opponent: 2,
        });
        app.add_system(reset_wreck_streaks);

        app.update();

        assert_eq!(
            *app.world.resource::<WreckStreaks>(),
            WreckStreaks::default()
        );
    }

    #[test]
    fn system_spins_out_a_team_it_grinds_to_a_wreck() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            // One frame of the base scrape (0.25) tips this to zero.
            opponent: 0.2,
        });
        app.init_resource::<WreckStuns>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let stuns = app.world.resource::<WreckStuns>();
        // The wrecked opponent spins out; the wrecking player team does not.
        assert_eq!(stuns.opponent_frames, WRECK_STUN_FRAMES);
        assert_eq!(stuns.player_frames, 0);
    }

    #[test]
    fn system_leaves_operational_teams_unstunned() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        app.init_resource::<WreckStuns>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        assert_eq!(*app.world.resource::<WreckStuns>(), WreckStuns::default());
    }

    #[test]
    fn wreck_stun_decay_system_winds_down_each_team() {
        let mut app = App::new();
        app.insert_resource(WreckStuns {
            player_frames: 2,
            opponent_frames: 1,
        });
        app.add_system(wreck_stun_decay_system);

        app.update();
        assert_eq!(
            *app.world.resource::<WreckStuns>(),
            WreckStuns {
                player_frames: 1,
                opponent_frames: 0,
            }
        );

        app.update();
        assert_eq!(*app.world.resource::<WreckStuns>(), WreckStuns::default());
    }

    #[test]
    fn entering_match_resets_wreck_stuns() {
        let mut app = App::new();
        app.insert_resource(WreckStuns {
            player_frames: 12,
            opponent_frames: 34,
        });
        app.add_system(reset_wreck_stuns);

        app.update();

        assert_eq!(*app.world.resource::<WreckStuns>(), WreckStuns::default());
    }

    #[test]
    fn system_surges_a_team_that_grinds_an_enemy_to_a_wreck() {
        let mut app = App::new();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            // One frame of the base scrape (0.25) tips this to zero.
            opponent: 0.2,
        });
        app.init_resource::<WreckSurges>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let surges = app.world.resource::<WreckSurges>();
        // The player team landed the kill and surges; the wreck does not.
        assert_eq!(surges.player_frames, WRECK_SURGE_FRAMES);
        assert_eq!(surges.opponent_frames, 0);
    }

    #[test]
    fn system_leaves_operational_teams_without_a_surge() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        app.init_resource::<WreckSurges>();
        app.add_system(ram_damage_system);
        app.world
            .spawn((player_stub(), Transform::from_translation(Vec3::ZERO)));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        assert_eq!(*app.world.resource::<WreckSurges>(), WreckSurges::default());
    }

    #[test]
    fn wreck_surge_decay_system_winds_down_each_team() {
        let mut app = App::new();
        app.insert_resource(WreckSurges {
            player_frames: 2,
            opponent_frames: 1,
        });
        app.add_system(wreck_surge_decay_system);

        app.update();
        assert_eq!(
            *app.world.resource::<WreckSurges>(),
            WreckSurges {
                player_frames: 1,
                opponent_frames: 0,
            }
        );

        app.update();
        assert_eq!(*app.world.resource::<WreckSurges>(), WreckSurges::default());
    }

    #[test]
    fn entering_match_resets_wreck_surges() {
        let mut app = App::new();
        app.insert_resource(WreckSurges {
            player_frames: 12,
            opponent_frames: 34,
        });
        app.add_system(reset_wreck_surges);

        app.update();

        assert_eq!(*app.world.resource::<WreckSurges>(), WreckSurges::default());
    }

    #[test]
    fn system_adds_corner_crush_bonus_on_top_of_the_wall_crush_in_a_corner() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        app.add_system(ram_damage_system);
        // Nose to nose along the diagonal into the +X/+Y corner: the player (blue)
        // charges up-right into the red, which charges down-left straight back.
        // Both eat the base scrape, the mutual head-on aggressor charge and the
        // shared head-on smash, but only the red is wedged into the corner, so it
        // alone takes the wall-crush pin *and* the corner-crush top-up on top.
        app.world.spawn((
            player_stub(),
            Transform::from_translation(Vec3::new(820.0, 420.0, 0.0))
                .with_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_4)),
        ));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(900.0, 500.0, 0.0))
                .with_rotation(Quat::from_rotation_z(3.0 * std::f32::consts::FRAC_PI_4)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        // The unpinned player eats the base scrape, the aggressor charge and the
        // shared head-on smash.
        assert_near(
            integrity.player,
            MAX_INTEGRITY
                - RAM_DAMAGE_PER_FRAME
                - AGGRESSOR_RAM_DAMAGE_PER_FRAME
                - HEAD_ON_RAM_DAMAGE_PER_FRAME,
        );
        // The corner-wedged red eats the same three plus both the wall-crush pin
        // and the corner-crush top-up.
        assert_near(
            integrity.opponent,
            MAX_INTEGRITY
                - RAM_DAMAGE_PER_FRAME
                - AGGRESSOR_RAM_DAMAGE_PER_FRAME
                - HEAD_ON_RAM_DAMAGE_PER_FRAME
                - WALL_CRUSH_RAM_DAMAGE_PER_FRAME
                - CORNER_CRUSH_RAM_DAMAGE_PER_FRAME,
        );
    }

    #[test]
    fn system_adds_wall_crush_bonus_when_a_charge_pins_a_car_to_the_wall() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        app.add_system(ram_damage_system);
        // Nose to nose against the +X wall: the player (blue) charges +X into the
        // red car, which charges -X straight back. Both eat the base scrape, the
        // mutual head-on aggressor charge and the shared head-on smash, but only
        // the red is pinned against the wall, so the wall-crush bonus is the sole
        // extra wear it takes beyond the shared smash.
        app.world.spawn((
            player_stub(),
            Transform::from_translation(Vec3::new(810.0, 0.0, 0.0))
                .with_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)),
        ));
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(900.0, 0.0, 0.0))
                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        // The unpinned player eats the base scrape, the aggressor charge and the
        // shared head-on smash.
        assert_near(
            integrity.player,
            MAX_INTEGRITY
                - RAM_DAMAGE_PER_FRAME
                - AGGRESSOR_RAM_DAMAGE_PER_FRAME
                - HEAD_ON_RAM_DAMAGE_PER_FRAME,
        );
        // The wall-pinned red eats the wall-crush bonus on top of the same three.
        assert_near(
            integrity.opponent,
            MAX_INTEGRITY
                - RAM_DAMAGE_PER_FRAME
                - AGGRESSOR_RAM_DAMAGE_PER_FRAME
                - HEAD_ON_RAM_DAMAGE_PER_FRAME
                - WALL_CRUSH_RAM_DAMAGE_PER_FRAME,
        );
    }

    fn player_stub() -> Player {
        Player {
            movement_speed: 0.0,
            rotation_speed: 0.0,
            engine_max_speed_multiplier: 0.0,
            forward_max_speed_base: 0.0,
            backward_max_speed_base: 0.0,
            wheels_turning_multiplier: 0.0,
        }
    }

    fn virtual_player_stub(team: AiTeam) -> VirtualPlayer {
        VirtualPlayer {
            team,
            movement_speed: 0.0,
            rotation_speed: 0.0,
            waypoints: vec![],
            current_waypoint: 0,
            player_pursuit_radius: 0.0,
            pickup_pursuit_radius: 0.0,
            corner_throttle: 0.3,
        }
    }
}
