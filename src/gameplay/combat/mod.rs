use crate::gameplay::ctf::{
    CaptureScore, CtfFlag, CtfMatchResult, FlagTeam, CAPTURES_TO_WIN, CAPTURE_CASH_BOUNTY,
    FLAG_RETURN_CASH_BOUNTY,
};
use crate::gameplay::pickup::{ArmourBoosts, NitroBoosts, OpponentScore, Score};
use crate::gameplay::player::Player;
use crate::gameplay::virtual_player::ai::AiTeam;
use crate::gameplay::virtual_player::VirtualPlayer;
use crate::{App, AppState, Plugin};
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;

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
/// Extra durability a flag-carrying car's team loses each frame it is trading
/// paint with an enemy.
///
/// A car hauling the enemy flag is not just slow, it is fragile: defenders who
/// ram the carrier wear its team down twice as fast as the base scrape. This
/// deepens the capture-the-flag gauntlet, the run home becomes a real risk, not
/// a victory lap, and pairs with the flag-carrier slowdown so a battered
/// carrier crawls back into reach of its pursuers.
pub const FLAG_CARRIER_RAM_DAMAGE_PER_FRAME: f32 = 0.5;
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

/// Per-team vehicle durability, mirroring [`crate::gameplay::pickup::NitroBoosts`].
///
/// Cars wear down by ramming opposing cars and are patched up by repair
/// pickups. A battered team drives slower, so trading paint with the flag
/// carrier becomes a real way to slow them down, the classic Death Rally way.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct VehicleIntegrity {
    /// Durability of the player team (the human and blue virtual players).
    pub player: f32,
    /// Durability of the opponent team (the red virtual players).
    pub opponent: f32,
}

impl Default for VehicleIntegrity {
    fn default() -> Self {
        Self {
            player: MAX_INTEGRITY,
            opponent: MAX_INTEGRITY,
        }
    }
}

impl VehicleIntegrity {
    /// Speed multiplier the player team's wear translates to.
    #[must_use]
    pub fn player_multiplier(self) -> f32 {
        integrity_speed_multiplier(self.player)
    }

    /// Speed multiplier the opponent team's wear translates to.
    #[must_use]
    pub fn opponent_multiplier(self) -> f32 {
        integrity_speed_multiplier(self.opponent)
    }

    /// Durability fraction (`0.0`..=`1.0`) of the given team's own wear.
    ///
    /// Virtual players price repair pickups against their *own* team's wear: a
    /// pristine team gains nothing from a patch-up (durability is capped at
    /// [`MAX_INTEGRITY`]), so it must not chase one however battered the enemy is.
    #[must_use]
    pub fn fraction_for_team(self, team: AiTeam) -> f32 {
        let integrity = match team {
            AiTeam::Blue => self.player,
            AiTeam::Red => self.opponent,
        };
        (integrity / MAX_INTEGRITY).clamp(0.0, 1.0)
    }

    /// Speed multiplier for the given team's current wear.
    #[must_use]
    pub fn multiplier_for_team(self, team: AiTeam) -> f32 {
        match team {
            AiTeam::Blue => self.player_multiplier(),
            AiTeam::Red => self.opponent_multiplier(),
        }
    }

    /// Patches up a team's durability, capped at [`MAX_INTEGRITY`].
    pub fn repair(&mut self, team: AiTeam) {
        match team {
            AiTeam::Blue => self.player = (self.player + REPAIR_INTEGRITY).min(MAX_INTEGRITY),
            AiTeam::Red => self.opponent = (self.opponent + REPAIR_INTEGRITY).min(MAX_INTEGRITY),
        }
    }

    /// Wears down a team's durability, floored at zero.
    pub fn apply_damage(&mut self, damage: TeamDamage) {
        self.player = (self.player - damage.player).max(0.0);
        self.opponent = (self.opponent - damage.opponent).max(0.0);
    }

    /// Patches each team up by its home-base pit recovery, capped at
    /// [`MAX_INTEGRITY`]. The recovery mirror of [`Self::apply_damage`]: where ram
    /// wear is subtracted, a team parked on its home turf adds durability back.
    pub fn apply_base_repair(&mut self, repair: BaseRepair) {
        self.player = (self.player + repair.player).min(MAX_INTEGRITY);
        self.opponent = (self.opponent + repair.opponent).min(MAX_INTEGRITY);
    }

    /// Teams this frame's wear drove from operational (`> 0`) to a full wreck
    /// (`0`), given the durability `before` the damage was applied.
    ///
    /// The crossing is the trigger: a team already flat-lined when `before` was
    /// taken does not re-fire, so each wreck is reported exactly once until a
    /// repair lifts the team back above zero and it can be wrecked anew. This is
    /// what makes [`WRECK_CASH_BOUNTY`] pay per wreck rather than per frame.
    #[must_use]
    pub fn newly_wrecked(self, before: Self) -> WreckEvents {
        WreckEvents {
            player: before.player > 0.0 && self.player <= 0.0,
            opponent: before.opponent > 0.0 && self.opponent <= 0.0,
        }
    }
}

/// Which teams crossed into a full wreck (zero integrity) this frame.
///
/// A wreck pays the wrecking team a [`WRECK_CASH_BOUNTY`]: a wrecked player team
/// banks the bounty for the opponents, a wrecked opponent team banks it for the
/// player team.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WreckEvents {
    /// The player team (human + blue virtual players) was wrecked this frame.
    pub player: bool,
    /// The opponent team (red virtual players) was wrecked this frame.
    pub opponent: bool,
}

impl WreckEvents {
    /// Whether either team was wrecked this frame.
    #[must_use]
    pub const fn any(self) -> bool {
        self.player || self.opponent
    }

    /// Whether the given team was among those wrecked this frame.
    #[must_use]
    pub const fn includes(self, team: AiTeam) -> bool {
        match team {
            AiTeam::Blue => self.player,
            AiTeam::Red => self.opponent,
        }
    }
}

/// A flag currently being hauled by a car, tagged with the carrying team.
///
/// Bridges the per-team [`WreckEvents`] to the per-flag CTF state so a wreck can
/// strip the flag from whichever car was carrying it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarriedFlag {
    /// The flag entity being hauled.
    pub flag: Entity,
    /// The team of the car hauling it.
    pub carrier_team: AiTeam,
}

/// Flags a freshly wrecked team must drop this frame.
///
/// The classic capture-the-flag turnover: when a team's integrity is ground to
/// zero its cars spin out, and a spun-out wreck cannot keep its grip on a stolen
/// flag. Every flag carried by a member of a newly wrecked team is dropped where
/// it lies, handing the wrecking team a real shot at recovering it and closing
/// the loop the ramming systems open (steal -> slowed + fragile -> rammed ->
/// wrecked -> drop the flag). Symmetric: both teams' carriers are equally fragile.
#[must_use]
pub fn flags_dropped_by_wrecks(carried: &[CarriedFlag], wrecks: WreckEvents) -> Vec<Entity> {
    carried
        .iter()
        .filter(|held| wrecks.includes(held.carrier_team))
        .map(|held| held.flag)
        .collect()
}

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
}

/// Resolves every cash reward a frame's wrecks pay: the rampage streak payout,
/// the most-wanted leader bonus, and the carrier-takedown bonus.
///
/// The player team deals a wreck when the opponents fall (and vice versa), so it
/// collects on the opponents' capture lead and on a wrecked opponent carrier.
/// `player_was_carrying`/`opponent_was_carrying` say whether each team had a car
/// hauling the enemy flag the frame it fell. Bonuses are folded into the per-team
/// totals and also returned individually for the wreck log.
#[must_use]
pub const fn resolve_wreck_bounties(
    before_streaks: WreckStreaks,
    wrecks: WreckEvents,
    captures: CaptureScore,
    player_was_carrying: bool,
    opponent_was_carrying: bool,
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
        carrier_takedown_wreck_bonus(opponent_was_carrying)
    } else {
        0
    };
    let opponent_carrier_takedown = if wrecks.player {
        carrier_takedown_wreck_bonus(player_was_carrying)
    } else {
        0
    };

    WreckBounties {
        streaks: payout.streaks,
        player: payout.player_bounty + player_most_wanted + player_carrier_takedown,
        opponent: payout.opponent_bounty + opponent_most_wanted + opponent_carrier_takedown,
        player_most_wanted,
        opponent_most_wanted,
        player_carrier_takedown,
        opponent_carrier_takedown,
    }
}

/// Brief spin-out each team suffers the instant its cars are wrecked.
///
/// Mirrors [`crate::gameplay::pickup::NitroBoosts`]: a per-team frame timer that
/// translates into a speed multiplier while it burns down. Triggered by
/// [`ram_damage_system`] on the frame a team is newly wrecked, wound down each
/// frame by [`wreck_stun_decay_system`], and read by the movement systems to
/// stagger a freshly wrecked team's cars.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WreckStuns {
    /// Frames the player team's cars keep spinning out.
    pub player_frames: u32,
    /// Frames the opponent team's cars keep spinning out.
    pub opponent_frames: u32,
}

impl WreckStuns {
    /// Speed multiplier the player team suffers while spinning out.
    #[must_use]
    pub const fn player_multiplier(self) -> f32 {
        if self.player_frames > 0 {
            WRECK_STUN_SPEED_MULTIPLIER
        } else {
            1.0
        }
    }

    /// Speed multiplier the opponent team suffers while spinning out.
    #[must_use]
    pub const fn opponent_multiplier(self) -> f32 {
        if self.opponent_frames > 0 {
            WRECK_STUN_SPEED_MULTIPLIER
        } else {
            1.0
        }
    }

    /// Speed multiplier for the given team's current spin-out.
    #[must_use]
    pub const fn multiplier_for_team(self, team: AiTeam) -> f32 {
        match team {
            AiTeam::Blue => self.player_multiplier(),
            AiTeam::Red => self.opponent_multiplier(),
        }
    }

    /// Spins out the player team for a fresh [`WRECK_STUN_FRAMES`] window.
    pub const fn trigger_player(&mut self) {
        self.player_frames = WRECK_STUN_FRAMES;
    }

    /// Spins out the opponent team for a fresh [`WRECK_STUN_FRAMES`] window.
    pub const fn trigger_opponent(&mut self) {
        self.opponent_frames = WRECK_STUN_FRAMES;
    }

    /// Spins out whichever teams crossed into a full wreck this frame.
    pub const fn apply_wrecks(&mut self, wrecks: WreckEvents) {
        if wrecks.player {
            self.trigger_player();
        }
        if wrecks.opponent {
            self.trigger_opponent();
        }
    }

    /// Winds every team's spin-out down by one frame.
    pub const fn tick(&mut self) {
        self.player_frames = self.player_frames.saturating_sub(1);
        self.opponent_frames = self.opponent_frames.saturating_sub(1);
    }
}

/// Brief speed surge each team enjoys the instant it wrecks an enemy.
///
/// The reward mirror of [`WreckStuns`]: where the wrecked team spins out, the
/// team that dealt the wreck surges. Same per-team frame-timer shape, triggered
/// by [`ram_damage_system`] on the frame an enemy is newly wrecked, wound down
/// each frame by [`wreck_surge_decay_system`], and read by the movement systems
/// to give a fresh wrecker a burst of speed it can capitalise on.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WreckSurges {
    /// Frames the player team's cars keep surging.
    pub player_frames: u32,
    /// Frames the opponent team's cars keep surging.
    pub opponent_frames: u32,
}

impl WreckSurges {
    /// Speed multiplier the player team enjoys while surging.
    #[must_use]
    pub const fn player_multiplier(self) -> f32 {
        if self.player_frames > 0 {
            WRECK_SURGE_SPEED_MULTIPLIER
        } else {
            1.0
        }
    }

    /// Speed multiplier the opponent team enjoys while surging.
    #[must_use]
    pub const fn opponent_multiplier(self) -> f32 {
        if self.opponent_frames > 0 {
            WRECK_SURGE_SPEED_MULTIPLIER
        } else {
            1.0
        }
    }

    /// Speed multiplier for the given team's current surge.
    #[must_use]
    pub const fn multiplier_for_team(self, team: AiTeam) -> f32 {
        match team {
            AiTeam::Blue => self.player_multiplier(),
            AiTeam::Red => self.opponent_multiplier(),
        }
    }

    /// Surges the player team for a fresh [`WRECK_SURGE_FRAMES`] window.
    pub const fn trigger_player(&mut self) {
        self.player_frames = WRECK_SURGE_FRAMES;
    }

    /// Surges the opponent team for a fresh [`WRECK_SURGE_FRAMES`] window.
    pub const fn trigger_opponent(&mut self) {
        self.opponent_frames = WRECK_SURGE_FRAMES;
    }

    /// Surges whichever team *dealt* a wreck this frame: the enemy of each
    /// wrecked team.
    ///
    /// A wrecked opponent means the player team landed the kill and surges, and
    /// vice versa. A double wreck surges both teams at once, mirroring how both
    /// also spin out.
    pub const fn reward_wreckers(&mut self, wrecks: WreckEvents) {
        if wrecks.opponent {
            self.trigger_player();
        }
        if wrecks.player {
            self.trigger_opponent();
        }
    }

    /// Winds every team's surge down by one frame.
    pub const fn tick(&mut self) {
        self.player_frames = self.player_frames.saturating_sub(1);
        self.opponent_frames = self.opponent_frames.saturating_sub(1);
    }
}

/// Maps a durability value onto the linear speed penalty it imposes.
fn integrity_speed_multiplier(integrity: f32) -> f32 {
    let fraction = (integrity / MAX_INTEGRITY).clamp(0.0, 1.0);
    (1.0 - MIN_INTEGRITY_SPEED_MULTIPLIER).mul_add(fraction, MIN_INTEGRITY_SPEED_MULTIPLIER)
}

/// A car considered for ram damage this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RamCar {
    pub team: AiTeam,
    pub position: Vec2,
    /// The car's facing direction, used by [`aggressor_ram_damage`] to tell a
    /// committed head-first charge from an incidental side-scrape.
    pub forward: Vec2,
    /// Whether this car is currently hauling the enemy flag, making it a
    /// fragile target for [`carrier_ram_damage`].
    pub carrying_flag: bool,
}

/// Durability each team loses from ramming in a single frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamDamage {
    pub player: f32,
    pub opponent: f32,
}

impl TeamDamage {
    /// Sums two frames' worth of damage, e.g. the base scrape plus a nitro ram.
    #[must_use]
    pub fn combined(self, other: Self) -> Self {
        Self {
            player: self.player + other.player,
            opponent: self.opponent + other.opponent,
        }
    }
}

/// Which teams are burning nitro this frame, for offensive ram bonuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RamBoost {
    pub player: bool,
    pub opponent: bool,
}

impl RamBoost {
    /// Reads the live nitro timers into the teams that are currently boosting.
    #[must_use]
    pub const fn from_nitro(boosts: &NitroBoosts) -> Self {
        Self {
            player: boosts.is_player_active(),
            opponent: boosts.is_opponent_active(),
        }
    }

    const fn is_team_boosting(self, team: AiTeam) -> bool {
        match team {
            AiTeam::Blue => self.player,
            AiTeam::Red => self.opponent,
        }
    }
}

/// Which teams have their shield up this frame, for defensive damage mitigation.
///
/// The defensive mirror of [`RamBoost`]: where a boost makes a team's rams bite,
/// a shield blunts the ram damage that team *takes*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RamShield {
    pub player: bool,
    pub opponent: bool,
}

impl RamShield {
    /// Reads the live shield timers into the teams that are currently armoured.
    #[must_use]
    pub const fn from_armour(boosts: &ArmourBoosts) -> Self {
        Self {
            player: boosts.is_player_active(),
            opponent: boosts.is_opponent_active(),
        }
    }
}

/// Blunts the ram damage a shielded team takes by [`SHIELD_DAMAGE_MULTIPLIER`].
///
/// Applied once to the already-summed frame damage, so a shield mitigates every
/// ram source at once (base scrape, nitro charge, aggressor hit, carrier bleed).
/// An unshielded team's damage passes through untouched.
#[must_use]
pub fn armour_mitigated_damage(damage: TeamDamage, shield: RamShield) -> TeamDamage {
    TeamDamage {
        player: if shield.player {
            damage.player * SHIELD_DAMAGE_MULTIPLIER
        } else {
            damage.player
        },
        opponent: if shield.opponent {
            damage.opponent * SHIELD_DAMAGE_MULTIPLIER
        } else {
            damage.opponent
        },
    }
}

/// Computes the ram damage each team takes from the current car positions.
///
/// A car is "ramming" when an opposing car sits within [`RAM_RADIUS`]. Every
/// such car bleeds [`RAM_DAMAGE_PER_FRAME`] into its own team's pool, so being
/// outnumbered in a scrum wears a team down faster.
#[must_use]
pub fn ram_damage(cars: &[RamCar]) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, car) in cars.iter().enumerate() {
        let in_contact = cars.iter().enumerate().any(|(other_index, other)| {
            other_index != index
                && other.team != car.team
                && other.position.distance_squared(car.position) <= radius_sq
        });
        if in_contact {
            match car.team {
                AiTeam::Blue => damage.player += RAM_DAMAGE_PER_FRAME,
                AiTeam::Red => damage.opponent += RAM_DAMAGE_PER_FRAME,
            }
        }
    }

    damage
}

/// Computes the bonus ram damage nitro-boosted cars inflict on the enemy.
///
/// For every boosted car in contact with an opposing car, the *enemy* team
/// bleeds [`NITRO_RAM_DAMAGE_PER_FRAME`] on top of the base [`ram_damage`]
/// scrape. The hit lands on whoever the boosted car is charging, so the
/// aggressor's nitro window is what makes ramming bite.
#[must_use]
pub fn nitro_ram_damage(cars: &[RamCar], boost: RamBoost) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, car) in cars.iter().enumerate() {
        if !boost.is_team_boosting(car.team) {
            continue;
        }

        let in_contact = cars.iter().enumerate().any(|(other_index, other)| {
            other_index != index
                && other.team != car.team
                && other.position.distance_squared(car.position) <= radius_sq
        });
        if in_contact {
            // The enemy of the boosted car eats the charge.
            match car.team {
                AiTeam::Blue => damage.opponent += NITRO_RAM_DAMAGE_PER_FRAME,
                AiTeam::Red => damage.player += NITRO_RAM_DAMAGE_PER_FRAME,
            }
        }
    }

    damage
}

/// Computes the bonus ram damage flag carriers bleed while trading paint.
///
/// For every car carrying the enemy flag that is in contact with an opposing
/// car, the carrier's *own* team bleeds [`FLAG_CARRIER_RAM_DAMAGE_PER_FRAME`] on
/// top of the base [`ram_damage`] scrape. The hit lands on the carrier's team,
/// so hauling the flag through a scrum is what makes it bite.
#[must_use]
pub fn carrier_ram_damage(cars: &[RamCar]) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, car) in cars.iter().enumerate() {
        if !car.carrying_flag {
            continue;
        }

        let in_contact = cars.iter().enumerate().any(|(other_index, other)| {
            other_index != index
                && other.team != car.team
                && other.position.distance_squared(car.position) <= radius_sq
        });
        if in_contact {
            match car.team {
                AiTeam::Blue => damage.player += FLAG_CARRIER_RAM_DAMAGE_PER_FRAME,
                AiTeam::Red => damage.opponent += FLAG_CARRIER_RAM_DAMAGE_PER_FRAME,
            }
        }
    }

    damage
}

/// Computes the bonus ram damage cars charging head-first into an enemy deal.
///
/// A car is "charging" when an opposing car sits within [`RAM_RADIUS`] and
/// inside the forward cone set by [`AGGRESSOR_RAM_ALIGNMENT`]. For every such
/// car the *enemy* team bleeds [`AGGRESSOR_RAM_DAMAGE_PER_FRAME`] on top of the
/// base [`ram_damage`] scrape, so lining up a ram beats stumbling into one. A
/// head-on collision charges both cars, wearing both teams down at once.
#[must_use]
pub fn aggressor_ram_damage(cars: &[RamCar]) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, car) in cars.iter().enumerate() {
        let Some(heading) = car.forward.try_normalize() else {
            continue;
        };

        let is_charging = cars.iter().enumerate().any(|(other_index, other)| {
            if other_index == index || other.team == car.team {
                return false;
            }
            let offset = other.position - car.position;
            if offset.length_squared() > radius_sq {
                return false;
            }
            offset
                .try_normalize()
                .is_some_and(|direction| heading.dot(direction) >= AGGRESSOR_RAM_ALIGNMENT)
        });
        if is_charging {
            // The enemy the charging car is aiming at eats the extra hit.
            match car.team {
                AiTeam::Blue => damage.opponent += AGGRESSOR_RAM_DAMAGE_PER_FRAME,
                AiTeam::Red => damage.player += AGGRESSOR_RAM_DAMAGE_PER_FRAME,
            }
        }
    }

    damage
}

/// Computes the bonus ram damage cars caught side-on by a charging enemy take.
///
/// A car is "broadsided" when an opposing car is within [`RAM_RADIUS`], is
/// charging it (the striker's nose inside the [`AGGRESSOR_RAM_ALIGNMENT`] cone,
/// the same commitment the aggressor bonus demands) and strikes from the
/// victim's flank (the approach falling inside the side arc set by
/// [`BROADSIDE_RAM_FLANK_THRESHOLD`]). Every broadsided car bleeds
/// [`BROADSIDE_RAM_DAMAGE_PER_FRAME`] into its *own* team's pool on top of the
/// base [`ram_damage`] scrape, so a clean T-bone wears the struck team down
/// faster than a head-on meeting. Charged once per struck car however many
/// enemies pile into its flank, mirroring [`carrier_ram_damage`].
#[must_use]
pub fn broadside_ram_damage(cars: &[RamCar]) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, victim) in cars.iter().enumerate() {
        let Some(victim_heading) = victim.forward.try_normalize() else {
            continue;
        };

        let is_broadsided = cars.iter().enumerate().any(|(other_index, striker)| {
            if other_index == index || striker.team == victim.team {
                return false;
            }
            let to_striker = striker.position - victim.position;
            if to_striker.length_squared() > radius_sq {
                return false;
            }
            let Some(approach) = to_striker.try_normalize() else {
                return false;
            };
            // The striker is committing to the hit: its nose is on the victim.
            let charging = striker
                .forward
                .try_normalize()
                .is_some_and(|heading| heading.dot(-approach) >= AGGRESSOR_RAM_ALIGNMENT);
            // The victim is caught square: the strike falls on its side arc.
            let flanked = victim_heading.dot(approach).abs() <= BROADSIDE_RAM_FLANK_THRESHOLD;
            charging && flanked
        });
        if is_broadsided {
            match victim.team {
                AiTeam::Blue => damage.player += BROADSIDE_RAM_DAMAGE_PER_FRAME,
                AiTeam::Red => damage.opponent += BROADSIDE_RAM_DAMAGE_PER_FRAME,
            }
        }
    }

    damage
}

/// Computes the bonus ram damage cars run down from behind by a charging enemy
/// take.
///
/// A car is "rear-ended" when an opposing car is within [`RAM_RADIUS`], is
/// charging it (the striker's nose inside the [`AGGRESSOR_RAM_ALIGNMENT`] cone,
/// the same commitment the aggressor and broadside bonuses demand) and strikes
/// from the victim's rear arc, the wedge *behind* the flank arc set by
/// [`BROADSIDE_RAM_FLANK_THRESHOLD`]. Every rear-ended car bleeds
/// [`REAR_END_RAM_DAMAGE_PER_FRAME`] into its *own* team's pool on top of the
/// base [`ram_damage`] scrape, so running a fleeing foe down wears it faster
/// than meeting it head-on. The rear arc starts exactly where the flank arc
/// ends, so a single strike is ever only a flank *or* a rear hit, never both.
/// Charged once per struck car however many enemies pile into its tail,
/// mirroring [`broadside_ram_damage`].
#[must_use]
pub fn rear_end_ram_damage(cars: &[RamCar]) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, victim) in cars.iter().enumerate() {
        let Some(victim_heading) = victim.forward.try_normalize() else {
            continue;
        };

        let is_rear_ended = cars.iter().enumerate().any(|(other_index, striker)| {
            if other_index == index || striker.team == victim.team {
                return false;
            }
            let to_striker = striker.position - victim.position;
            if to_striker.length_squared() > radius_sq {
                return false;
            }
            let Some(approach) = to_striker.try_normalize() else {
                return false;
            };
            // The striker is committing to the hit: its nose is on the victim.
            let charging = striker
                .forward
                .try_normalize()
                .is_some_and(|heading| heading.dot(-approach) >= AGGRESSOR_RAM_ALIGNMENT);
            // The victim is caught from behind: the strike falls past its flank
            // arc, onto the rear wedge where it faces dead away from the striker.
            let rear = victim_heading.dot(approach) < -BROADSIDE_RAM_FLANK_THRESHOLD;
            charging && rear
        });
        if is_rear_ended {
            match victim.team {
                AiTeam::Blue => damage.player += REAR_END_RAM_DAMAGE_PER_FRAME,
                AiTeam::Red => damage.opponent += REAR_END_RAM_DAMAGE_PER_FRAME,
            }
        }
    }

    damage
}

/// The pincer bite a single surrounded car takes from `attacker_count` enemies
/// hemming it in at once.
///
/// Below [`PINCER_MIN_ATTACKERS`] there is no pincer (just a ram, covered by the
/// base scrape), so the bonus is zero. At the minimum it is
/// [`PINCER_RAM_DAMAGE_PER_FRAME`]; every further attacker adds
/// [`PINCER_RAM_DAMAGE_PER_EXTRA_ATTACKER`] up to [`PINCER_MAX_EXTRA_ATTACKERS`]
/// extra, topping out at [`PINCER_MAX_RAM_DAMAGE_PER_FRAME`]. The single charge
/// scales with the swarm but never stacks into one hit per attacker, mirroring
/// the per-victim model of [`broadside_ram_damage`].
///
/// Accumulates the per-extra step without a `usize`-to-`f32` count cast (the
/// pedantic clippy gate forbids it), keeping the module's near-zero-cast
/// convention.
#[must_use]
pub const fn pincer_ram_bonus(attacker_count: usize) -> f32 {
    if attacker_count < PINCER_MIN_ATTACKERS {
        return 0.0;
    }
    let extra = attacker_count - PINCER_MIN_ATTACKERS;
    let capped = if extra > PINCER_MAX_EXTRA_ATTACKERS {
        PINCER_MAX_EXTRA_ATTACKERS
    } else {
        extra
    };
    // Accumulate the per-extra step by repeated addition: a small bounded loop
    // that avoids a `usize`-to-`f32` count cast the pedantic clippy gate forbids.
    let mut bonus = PINCER_RAM_DAMAGE_PER_FRAME;
    let mut step = 0;
    while step < capped {
        bonus += PINCER_RAM_DAMAGE_PER_EXTRA_ATTACKER;
        step += 1;
    }
    bonus
}

/// Computes the bonus ram damage cars hemmed in by a pincer of enemies take.
///
/// A car is "pincered" when at least [`PINCER_MIN_ATTACKERS`] opposing cars sit
/// within [`RAM_RADIUS`] at once: a gang-up with no lane left to escape. Every
/// such car bleeds [`pincer_ram_bonus`] for the size of its swarm into its *own*
/// team's pool on top of the base [`ram_damage`] scrape, so being outnumbered at
/// a point wears the surrounded team down faster, and the more foes pile in the
/// harder it is ground down, the Death Rally "they swarmed me" punishment.
/// Charged once per surrounded car (the single charge scales with the swarm but
/// never stacks per attacker), mirroring [`broadside_ram_damage`]; the bonus
/// rewards the converging team coordination the virtual-player brain already
/// drives (massing defenders, the finish-off hunter) without needing the aim the
/// directional bonuses demand.
#[must_use]
pub fn pincer_ram_damage(cars: &[RamCar]) -> TeamDamage {
    let radius_sq = RAM_RADIUS * RAM_RADIUS;
    let mut damage = TeamDamage {
        player: 0.0,
        opponent: 0.0,
    };

    for (index, victim) in cars.iter().enumerate() {
        let attackers = cars
            .iter()
            .enumerate()
            .filter(|&(other_index, other)| {
                other_index != index
                    && other.team != victim.team
                    && other.position.distance_squared(victim.position) <= radius_sq
            })
            .count();
        let bonus = pincer_ram_bonus(attackers);
        if bonus > 0.0 {
            match victim.team {
                AiTeam::Blue => damage.player += bonus,
                AiTeam::Red => damage.opponent += bonus,
            }
        }
    }

    damage
}

/// Durability each team regains from home-base pit recovery in a single frame.
///
/// The recovery mirror of [`TeamDamage`]: where ram wear is subtracted from a
/// team's pool, this is added back for a team that has retreated to its own base.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BaseRepair {
    pub player: f32,
    pub opponent: f32,
}

/// Computes the home-base pit recovery each team earns from the current car
/// positions.
///
/// A team with at least one car within [`BASE_REPAIR_RADIUS`] of its own home
/// base regains [`BASE_REPAIR_PER_FRAME`] durability; a team with no car home
/// regains nothing. Presence is binary, one car home patches the whole team pool
/// (matching the per-team integrity model), so massing cars at base grants no
/// extra heal. A car loitering in the *enemy* base earns its own team nothing,
/// so the recovery only ever rewards retreating to home turf.
#[must_use]
pub fn base_repair(cars: &[(AiTeam, Vec2)], blue_home: Vec2, red_home: Vec2) -> BaseRepair {
    let radius_sq = BASE_REPAIR_RADIUS * BASE_REPAIR_RADIUS;
    let home_for = |team: AiTeam| match team {
        AiTeam::Blue => blue_home,
        AiTeam::Red => red_home,
    };
    let team_at_home = |team: AiTeam| {
        cars.iter().any(|&(car_team, position)| {
            car_team == team && position.distance_squared(home_for(team)) <= radius_sq
        })
    };

    BaseRepair {
        player: if team_at_home(AiTeam::Blue) {
            BASE_REPAIR_PER_FRAME
        } else {
            0.0
        },
        opponent: if team_at_home(AiTeam::Red) {
            BASE_REPAIR_PER_FRAME
        } else {
            0.0
        },
    }
}

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

/// Wears down both teams whenever their cars are trading paint, and pays a
/// wreck bounty to whichever team grinds an enemy down to zero this frame.
#[allow(clippy::too_many_arguments)]
pub fn ram_damage_system(
    match_result: Option<Res<CtfMatchResult>>,
    captures: Option<Res<CaptureScore>>,
    nitro_boosts: Option<Res<NitroBoosts>>,
    armour_boosts: Option<Res<ArmourBoosts>>,
    mut integrity: ResMut<VehicleIntegrity>,
    mut wreck_streaks: Option<ResMut<WreckStreaks>>,
    mut wreck_stuns: Option<ResMut<WreckStuns>>,
    mut wreck_surges: Option<ResMut<WreckSurges>>,
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
    let raw_damage = ram_damage(&cars)
        .combined(nitro_ram_damage(&cars, boost))
        .combined(carrier_ram_damage(&cars))
        .combined(aggressor_ram_damage(&cars))
        .combined(broadside_ram_damage(&cars))
        .combined(rear_end_ram_damage(&cars))
        .combined(pincer_ram_damage(&cars));
    // A team with its shield up shrugs off part of every ram it eats this frame.
    let damage = armour_mitigated_damage(raw_damage, shield);

    let before = *integrity;
    integrity.apply_damage(damage);
    let wrecks = integrity.newly_wrecked(before);

    // A freshly wrecked team spins out: stagger its cars for a brief window so
    // the wrecking team gets a real opening to capitalise.
    if let Some(stuns) = wreck_stuns.as_deref_mut() {
        stuns.apply_wrecks(wrecks);
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
    let bounties = resolve_wreck_bounties(
        before_streaks,
        wrecks,
        captures,
        team_was_carrying(&cars, AiTeam::Blue),
        team_was_carrying(&cars, AiTeam::Red),
    );
    if let Some(streaks) = wreck_streaks.as_deref_mut() {
        *streaks = bounties.streaks;
    }

    if wrecks.any() {
        info!(
            "Wreck! player_down={} opponent_down={}; rampage streaks player={} opponent={}; \
             most-wanted bonus player={} opponent={}; carrier-takedown bonus player={} \
             opponent={}; banking player_bounty={} opponent_bounty={}",
            wrecks.player,
            wrecks.opponent,
            bounties.streaks.player,
            bounties.streaks.opponent,
            bounties.player_most_wanted,
            bounties.opponent_most_wanted,
            bounties.player_carrier_takedown,
            bounties.opponent_carrier_takedown,
            bounties.player,
            bounties.opponent,
        );
    }

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

#[derive(Default)]
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VehicleIntegrity>()
            .init_resource::<WreckStreaks>()
            .init_resource::<WreckStuns>()
            .init_resource::<WreckSurges>()
            .add_system_set(
                SystemSet::on_enter(AppState::InGame)
                    .with_system(reset_vehicle_integrity)
                    .with_system(reset_wreck_streaks)
                    .with_system(reset_wreck_stuns)
                    .with_system(reset_wreck_surges),
            )
            .add_system(wreck_stun_decay_system.before(ram_damage_system))
            .add_system(wreck_surge_decay_system.before(ram_damage_system))
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

    fn blue(position: Vec2) -> RamCar {
        RamCar {
            team: AiTeam::Blue,
            position,
            // Facing +Y, perpendicular to the +X contact axis these helpers
            // place cars on, so the base ram tests never trip the aggressor cone.
            forward: Vec2::Y,
            carrying_flag: false,
        }
    }

    fn red(position: Vec2) -> RamCar {
        RamCar {
            team: AiTeam::Red,
            position,
            forward: Vec2::Y,
            carrying_flag: false,
        }
    }

    /// A blue car at `position` charging head-first towards `target`.
    fn blue_facing(position: Vec2, target: Vec2) -> RamCar {
        RamCar {
            forward: (target - position).normalize_or_zero(),
            ..blue(position)
        }
    }

    /// A red car at `position` charging head-first towards `target`.
    fn red_facing(position: Vec2, target: Vec2) -> RamCar {
        RamCar {
            forward: (target - position).normalize_or_zero(),
            ..red(position)
        }
    }

    fn blue_carrier(position: Vec2) -> RamCar {
        RamCar {
            carrying_flag: true,
            ..blue(position)
        }
    }

    fn red_carrier(position: Vec2) -> RamCar {
        RamCar {
            carrying_flag: true,
            ..red(position)
        }
    }

    #[test]
    fn integrity_defaults_to_full_for_both_teams() {
        let integrity = VehicleIntegrity::default();
        assert_near(integrity.player, MAX_INTEGRITY);
        assert_near(integrity.opponent, MAX_INTEGRITY);
    }

    #[test]
    fn full_integrity_imposes_no_speed_penalty() {
        let integrity = VehicleIntegrity::default();
        assert_near(integrity.player_multiplier(), 1.0);
        assert_near(integrity.opponent_multiplier(), 1.0);
    }

    #[test]
    fn zero_integrity_imposes_the_minimum_speed_multiplier() {
        let integrity = VehicleIntegrity {
            player: 0.0,
            opponent: 0.0,
        };
        assert_near(
            integrity.player_multiplier(),
            MIN_INTEGRITY_SPEED_MULTIPLIER,
        );
        assert_near(
            integrity.opponent_multiplier(),
            MIN_INTEGRITY_SPEED_MULTIPLIER,
        );
    }

    #[test]
    fn half_integrity_imposes_a_proportional_penalty() {
        let integrity = VehicleIntegrity {
            player: MAX_INTEGRITY / 2.0,
            opponent: MAX_INTEGRITY,
        };
        let expected =
            MIN_INTEGRITY_SPEED_MULTIPLIER + (1.0 - MIN_INTEGRITY_SPEED_MULTIPLIER) / 2.0;
        assert_near(integrity.player_multiplier(), expected);
    }

    #[test]
    fn fraction_for_team_reports_each_team_against_its_own_wear() {
        let integrity = VehicleIntegrity {
            player: MAX_INTEGRITY,
            opponent: MAX_INTEGRITY / 4.0,
        };
        assert_near(integrity.fraction_for_team(AiTeam::Blue), 1.0);
        assert_near(integrity.fraction_for_team(AiTeam::Red), 0.25);
    }

    #[test]
    fn fraction_for_team_clamps_into_the_unit_range() {
        let extremes = VehicleIntegrity {
            player: -MAX_INTEGRITY,
            opponent: MAX_INTEGRITY * 2.0,
        };
        assert_near(extremes.fraction_for_team(AiTeam::Blue), 0.0);
        assert_near(extremes.fraction_for_team(AiTeam::Red), 1.0);
    }

    #[test]
    fn multiplier_for_team_routes_to_the_right_pool() {
        let integrity = VehicleIntegrity {
            player: 0.0,
            opponent: MAX_INTEGRITY,
        };
        assert_near(
            integrity.multiplier_for_team(AiTeam::Blue),
            MIN_INTEGRITY_SPEED_MULTIPLIER,
        );
        assert_near(integrity.multiplier_for_team(AiTeam::Red), 1.0);
    }

    #[test]
    fn repair_restores_the_collecting_team_up_to_the_cap() {
        let mut integrity = VehicleIntegrity {
            player: 50.0,
            opponent: 50.0,
        };
        integrity.repair(AiTeam::Blue);
        assert_near(integrity.player, 50.0 + REPAIR_INTEGRITY);
        assert_near(integrity.opponent, 50.0);

        integrity.player = MAX_INTEGRITY - 5.0;
        integrity.repair(AiTeam::Blue);
        assert_near(integrity.player, MAX_INTEGRITY);
    }

    #[test]
    fn apply_damage_floors_each_team_at_zero() {
        let mut integrity = VehicleIntegrity {
            player: 10.0,
            opponent: 1.0,
        };
        integrity.apply_damage(TeamDamage {
            player: 4.0,
            opponent: 100.0,
        });
        assert_near(integrity.player, 6.0);
        assert_near(integrity.opponent, 0.0);
    }

    #[test]
    fn newly_wrecked_flags_each_team_that_crosses_to_zero() {
        let before = VehicleIntegrity {
            player: 5.0,
            opponent: 5.0,
        };

        let player_only = VehicleIntegrity {
            player: 0.0,
            opponent: 5.0,
        }
        .newly_wrecked(before);
        assert_eq!(
            player_only,
            WreckEvents {
                player: true,
                opponent: false,
            }
        );

        let both = VehicleIntegrity {
            player: 0.0,
            opponent: 0.0,
        }
        .newly_wrecked(before);
        assert_eq!(
            both,
            WreckEvents {
                player: true,
                opponent: true,
            }
        );
    }

    #[test]
    fn newly_wrecked_ignores_a_team_already_flatlined() {
        // The opponent was already wrecked when `before` was taken, so holding
        // at zero this frame must not re-fire the bounty.
        let before = VehicleIntegrity {
            player: 5.0,
            opponent: 0.0,
        };
        let after = VehicleIntegrity {
            player: 5.0,
            opponent: 0.0,
        };
        assert_eq!(after.newly_wrecked(before), WreckEvents::default());
    }

    #[test]
    fn newly_wrecked_ignores_a_team_still_operational() {
        let before = VehicleIntegrity {
            player: 5.0,
            opponent: 5.0,
        };
        let after = VehicleIntegrity {
            player: 4.0,
            opponent: 0.5,
        };
        assert_eq!(after.newly_wrecked(before), WreckEvents::default());
    }

    #[test]
    fn wreck_events_report_whether_any_team_fell() {
        assert!(!WreckEvents::default().any());
        assert!(WreckEvents {
            player: false,
            opponent: true,
        }
        .any());
    }

    #[test]
    fn wreck_events_report_per_team_membership() {
        let player_only = WreckEvents {
            player: true,
            opponent: false,
        };
        assert!(player_only.includes(AiTeam::Blue));
        assert!(!player_only.includes(AiTeam::Red));

        let opponent_only = WreckEvents {
            player: false,
            opponent: true,
        };
        assert!(!opponent_only.includes(AiTeam::Blue));
        assert!(opponent_only.includes(AiTeam::Red));
    }

    #[test]
    fn a_quiet_frame_drops_no_carried_flags() {
        let carried = [CarriedFlag {
            flag: Entity::from_raw(5),
            carrier_team: AiTeam::Blue,
        }];
        assert!(flags_dropped_by_wrecks(&carried, WreckEvents::default()).is_empty());
    }

    #[test]
    fn a_wrecked_team_drops_only_the_flag_it_was_hauling() {
        let blue_flag = Entity::from_raw(5);
        let red_flag = Entity::from_raw(6);
        let carried = [
            CarriedFlag {
                flag: blue_flag,
                carrier_team: AiTeam::Blue,
            },
            CarriedFlag {
                flag: red_flag,
                carrier_team: AiTeam::Red,
            },
        ];

        let dropped = flags_dropped_by_wrecks(
            &carried,
            WreckEvents {
                player: true,
                opponent: false,
            },
        );

        assert_eq!(dropped, vec![blue_flag]);
    }

    #[test]
    fn both_wrecked_teams_drop_their_flags() {
        let blue_flag = Entity::from_raw(5);
        let red_flag = Entity::from_raw(6);
        let carried = [
            CarriedFlag {
                flag: blue_flag,
                carrier_team: AiTeam::Blue,
            },
            CarriedFlag {
                flag: red_flag,
                carrier_team: AiTeam::Red,
            },
        ];

        let dropped = flags_dropped_by_wrecks(
            &carried,
            WreckEvents {
                player: true,
                opponent: true,
            },
        );

        assert_eq!(dropped, vec![blue_flag, red_flag]);
    }

    #[test]
    fn no_damage_when_no_cars_are_touching() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS + 1.0, 0.0))];
        let damage = ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn touching_opponents_each_wear_down_their_own_team() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = ram_damage(&cars);
        assert_near(damage.player, RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn same_team_contact_deals_no_damage() {
        let cars = [blue(Vec2::ZERO), blue(Vec2::new(10.0, 0.0))];
        let damage = ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn outnumbered_team_takes_damage_per_car_in_contact() {
        // Two reds bracket a single blue; both reds and the blue are in contact.
        let cars = [
            blue(Vec2::ZERO),
            red(Vec2::new(50.0, 0.0)),
            red(Vec2::new(-50.0, 0.0)),
        ];
        let damage = ram_damage(&cars);
        assert_near(damage.player, RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 2.0 * RAM_DAMAGE_PER_FRAME);
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
    fn no_nitro_means_no_ram_bonus() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = nitro_ram_damage(&cars, RamBoost::default());
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn boosted_player_ram_wears_the_opponent() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = nitro_ram_damage(
            &cars,
            RamBoost {
                player: true,
                opponent: false,
            },
        );
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, NITRO_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn boosted_opponent_ram_wears_the_player() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = nitro_ram_damage(
            &cars,
            RamBoost {
                player: false,
                opponent: true,
            },
        );
        assert_near(damage.player, NITRO_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn boosted_car_out_of_contact_deals_no_bonus() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS + 1.0, 0.0))];
        let damage = nitro_ram_damage(
            &cars,
            RamBoost {
                player: true,
                opponent: true,
            },
        );
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn both_teams_boosting_each_wear_the_enemy() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = nitro_ram_damage(
            &cars,
            RamBoost {
                player: true,
                opponent: true,
            },
        );
        assert_near(damage.player, NITRO_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, NITRO_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn same_team_contact_deals_no_nitro_bonus() {
        let cars = [blue(Vec2::ZERO), blue(Vec2::new(10.0, 0.0))];
        let damage = nitro_ram_damage(
            &cars,
            RamBoost {
                player: true,
                opponent: true,
            },
        );
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn nitro_bonus_scales_per_boosted_car_in_contact() {
        // Two boosted reds bracket a single blue: both reds are charging the
        // lone blue, so the player team eats two ram hits this frame.
        let cars = [
            blue(Vec2::ZERO),
            red(Vec2::new(50.0, 0.0)),
            red(Vec2::new(-50.0, 0.0)),
        ];
        let damage = nitro_ram_damage(
            &cars,
            RamBoost {
                player: false,
                opponent: true,
            },
        );
        assert_near(damage.player, 2.0 * NITRO_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn an_empty_handed_car_bleeds_no_carrier_bonus() {
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = carrier_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_rammed_blue_carrier_wears_the_player_team() {
        let cars = [
            blue_carrier(Vec2::ZERO),
            red(Vec2::new(RAM_RADIUS - 10.0, 0.0)),
        ];
        let damage = carrier_ram_damage(&cars);
        assert_near(damage.player, FLAG_CARRIER_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_rammed_red_carrier_wears_the_opponent_team() {
        let cars = [
            red_carrier(Vec2::ZERO),
            blue(Vec2::new(RAM_RADIUS - 10.0, 0.0)),
        ];
        let damage = carrier_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, FLAG_CARRIER_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn a_carrier_out_of_contact_bleeds_no_carrier_bonus() {
        let cars = [
            blue_carrier(Vec2::ZERO),
            red(Vec2::new(RAM_RADIUS + 1.0, 0.0)),
        ];
        let damage = carrier_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_carrier_touched_only_by_a_teammate_bleeds_no_carrier_bonus() {
        // A blue carrier escorted by a blue teammate is not being defended
        // against, so the carrier tax must not fire.
        let cars = [blue_carrier(Vec2::ZERO), blue(Vec2::new(10.0, 0.0))];
        let damage = carrier_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn carrier_bonus_scales_per_defender_in_contact() {
        // Two reds bracket the lone blue carrier; the carrier eats the tax once
        // per frame regardless of how many defenders crowd it, because the tax
        // is charged to the carrier, not summed per defender.
        let cars = [
            blue_carrier(Vec2::ZERO),
            red(Vec2::new(50.0, 0.0)),
            red(Vec2::new(-50.0, 0.0)),
        ];
        let damage = carrier_ram_damage(&cars);
        assert_near(damage.player, FLAG_CARRIER_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_side_scrape_inflicts_no_aggressor_bonus() {
        // Both cars face +Y while touching along the X axis: neither is charging
        // the other, so only the base scrape (handled elsewhere) applies.
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = aggressor_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_charging_blue_car_wears_the_opponent_it_aims_at() {
        let enemy = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [blue_facing(Vec2::ZERO, enemy), red(enemy)];
        let damage = aggressor_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, AGGRESSOR_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn a_charging_red_car_wears_the_player_it_aims_at() {
        let enemy = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [red_facing(Vec2::ZERO, enemy), blue(enemy)];
        let damage = aggressor_ram_damage(&cars);
        assert_near(damage.player, AGGRESSOR_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_head_on_collision_charges_both_teams() {
        let blue_pos = Vec2::ZERO;
        let red_pos = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [
            blue_facing(blue_pos, red_pos),
            red_facing(red_pos, blue_pos),
        ];
        let damage = aggressor_ram_damage(&cars);
        assert_near(damage.player, AGGRESSOR_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, AGGRESSOR_RAM_DAMAGE_PER_FRAME);
    }

    #[test]
    fn a_charge_at_a_distant_enemy_inflicts_no_aggressor_bonus() {
        let enemy = Vec2::new(RAM_RADIUS + 1.0, 0.0);
        let cars = [blue_facing(Vec2::ZERO, enemy), red(enemy)];
        let damage = aggressor_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn charging_a_teammate_inflicts_no_aggressor_bonus() {
        let mate = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [blue_facing(Vec2::ZERO, mate), blue(mate)];
        let damage = aggressor_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn facing_just_inside_the_cone_charges_but_just_outside_does_not() {
        // Place the enemy on the X axis and aim the car at the cone's edge by
        // rotating its heading until the dot product brackets the threshold.
        let enemy = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let inside_angle = AGGRESSOR_RAM_ALIGNMENT.acos() - 0.01;
        let outside_angle = AGGRESSOR_RAM_ALIGNMENT.acos() + 0.01;

        let inside = [
            RamCar {
                forward: Vec2::new(inside_angle.cos(), inside_angle.sin()),
                ..blue(Vec2::ZERO)
            },
            red(enemy),
        ];
        assert_near(
            aggressor_ram_damage(&inside).opponent,
            AGGRESSOR_RAM_DAMAGE_PER_FRAME,
        );

        let outside = [
            RamCar {
                forward: Vec2::new(outside_angle.cos(), outside_angle.sin()),
                ..blue(Vec2::ZERO)
            },
            red(enemy),
        ];
        assert_near(aggressor_ram_damage(&outside).opponent, 0.0);
    }

    #[test]
    fn aggressor_bonus_scales_per_charging_car_in_contact() {
        // Two reds both charge a lone blue from either side: the player team
        // eats one aggressor hit per charging car this frame.
        let blue_pos = Vec2::ZERO;
        let cars = [
            blue(blue_pos),
            red_facing(Vec2::new(50.0, 0.0), blue_pos),
            red_facing(Vec2::new(-50.0, 0.0), blue_pos),
        ];
        let damage = aggressor_ram_damage(&cars);
        assert_near(damage.player, 2.0 * AGGRESSOR_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_degenerate_heading_inflicts_no_aggressor_bonus() {
        let enemy = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [
            RamCar {
                forward: Vec2::ZERO,
                ..blue(Vec2::ZERO)
            },
            red(enemy),
        ];
        let damage = aggressor_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_side_on_charge_broadsides_the_struck_team() {
        // A red car charges in from the blue car's flank: blue faces +Y while
        // the red striker comes from +X with its nose on blue's door.
        let victim = Vec2::ZERO;
        let striker = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [blue(victim), red_facing(striker, victim)];
        let damage = broadside_ram_damage(&cars);
        assert_near(damage.player, BROADSIDE_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_flank_charge_wears_the_red_team_it_t_bones() {
        // The mirror: a blue car charges a red car square in the side.
        let victim = Vec2::ZERO;
        let striker = Vec2::new(-(RAM_RADIUS - 10.0), 0.0);
        let cars = [red(victim), blue_facing(striker, victim)];
        let damage = broadside_ram_damage(&cars);
        assert_near(damage.opponent, BROADSIDE_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.player, 0.0);
    }

    #[test]
    fn a_head_on_charge_is_no_broadside() {
        // Nose to nose: each car is hit on its front, not its flank, so the
        // broadside bonus stays silent and only the aggressor charge applies.
        let blue_pos = Vec2::ZERO;
        let red_pos = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [
            blue_facing(blue_pos, red_pos),
            red_facing(red_pos, blue_pos),
        ];
        let damage = broadside_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_parallel_scrape_is_no_broadside() {
        // Two cars running side by side, both facing +Y: a flank position alone
        // earns no broadside without a striker charging into it.
        let cars = [blue(Vec2::ZERO), red(Vec2::new(RAM_RADIUS - 10.0, 0.0))];
        let damage = broadside_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_glancing_front_quarter_charge_is_no_broadside() {
        // The striker charges from 30 degrees off the victim's nose: inside the
        // aggressor cone but short of the side arc, a frontal clip not a T-bone.
        let victim = Vec2::ZERO;
        let angle = std::f32::consts::FRAC_PI_6;
        let striker = Vec2::new(angle.sin(), angle.cos()) * (RAM_RADIUS - 10.0);
        let cars = [blue(victim), red_facing(striker, victim)];
        let damage = broadside_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_broadside_needs_contact() {
        // A perfect side-on charge just out of ram range deals nothing.
        let victim = Vec2::ZERO;
        let striker = Vec2::new(RAM_RADIUS + 1.0, 0.0);
        let cars = [blue(victim), red_facing(striker, victim)];
        let damage = broadside_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_double_flank_charges_the_victim_once() {
        // Two reds T-bone a lone blue from both flanks: the struck car bleeds a
        // single broadside, not one per striker (the per-victim model).
        let victim = Vec2::ZERO;
        let cars = [
            blue(victim),
            red_facing(Vec2::new(50.0, 0.0), victim),
            red_facing(Vec2::new(-50.0, 0.0), victim),
        ];
        let damage = broadside_ram_damage(&cars);
        assert_near(damage.player, BROADSIDE_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_degenerate_heading_inflicts_no_broadside() {
        // A victim with no facing cannot be judged side-on, so it is skipped.
        let striker = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [
            RamCar {
                forward: Vec2::ZERO,
                ..blue(Vec2::ZERO)
            },
            red_facing(striker, Vec2::ZERO),
        ];
        let damage = broadside_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_tail_charge_rear_ends_the_struck_team() {
        // A red car runs the blue car down from directly behind: blue faces +Y
        // while the red striker chases from -Y with its nose on blue's tail.
        let victim = Vec2::ZERO;
        let striker = Vec2::new(0.0, -(RAM_RADIUS - 10.0));
        let cars = [blue(victim), red_facing(striker, victim)];
        let damage = rear_end_ram_damage(&cars);
        assert_near(damage.player, REAR_END_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_tail_charge_wears_the_red_team_it_runs_down() {
        // The mirror: a blue car runs a red car down from directly behind.
        let victim = Vec2::ZERO;
        let striker = Vec2::new(0.0, -(RAM_RADIUS - 10.0));
        let cars = [red(victim), blue_facing(striker, victim)];
        let damage = rear_end_ram_damage(&cars);
        assert_near(damage.opponent, REAR_END_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.player, 0.0);
    }

    #[test]
    fn a_head_on_charge_is_no_rear_end() {
        // Nose to nose: each car is struck on its front, not its tail, so the
        // rear-end bonus stays silent and only the aggressor charge applies.
        let blue_pos = Vec2::ZERO;
        let red_pos = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [
            blue_facing(blue_pos, red_pos),
            red_facing(red_pos, blue_pos),
        ];
        let damage = rear_end_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_side_on_charge_is_no_rear_end() {
        // A clean flank T-bone falls inside the side arc, short of the rear
        // wedge, so it earns a broadside but never a rear-end: the two arcs are
        // disjoint, and a single strike is one or the other, never both.
        let victim = Vec2::ZERO;
        let striker = Vec2::new(RAM_RADIUS - 10.0, 0.0);
        let cars = [blue(victim), red_facing(striker, victim)];
        assert_near(
            broadside_ram_damage(&cars).player,
            BROADSIDE_RAM_DAMAGE_PER_FRAME,
        );
        let damage = rear_end_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_tail_position_without_a_charge_is_no_rear_end() {
        // A red car sits dead behind the blue car but faces away (-Y), so it is
        // tailing without committing: a rear position alone earns no rear-end.
        let victim = Vec2::ZERO;
        let cars = [
            blue(victim),
            RamCar {
                forward: Vec2::NEG_Y,
                ..red(Vec2::new(0.0, -(RAM_RADIUS - 10.0)))
            },
        ];
        let damage = rear_end_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_rear_end_needs_contact() {
        // A perfect tail charge just out of ram range deals nothing.
        let victim = Vec2::ZERO;
        let striker = Vec2::new(0.0, -(RAM_RADIUS + 1.0));
        let cars = [blue(victim), red_facing(striker, victim)];
        let damage = rear_end_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_double_tail_charge_rear_ends_the_victim_once() {
        // Two reds pile into a lone blue's tail: the struck car bleeds a single
        // rear-end, not one per striker (the per-victim model).
        let victim = Vec2::ZERO;
        let cars = [
            blue(victim),
            red_facing(Vec2::new(0.0, -50.0), victim),
            red_facing(Vec2::new(0.0, -90.0), victim),
        ];
        let damage = rear_end_ram_damage(&cars);
        assert_near(damage.player, REAR_END_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_degenerate_heading_inflicts_no_rear_end() {
        // A victim with no facing cannot be judged from behind, so it is skipped.
        let striker = Vec2::new(0.0, -(RAM_RADIUS - 10.0));
        let cars = [
            RamCar {
                forward: Vec2::ZERO,
                ..blue(Vec2::ZERO)
            },
            red_facing(striker, Vec2::ZERO),
        ];
        let damage = rear_end_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_lone_ram_is_no_pincer() {
        // A single enemy in contact is just a ram, not a gang-up.
        let cars = [blue(Vec2::ZERO), red(Vec2::new(50.0, 0.0))];
        let damage = pincer_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn two_enemies_pincer_the_surrounded_team() {
        // Two reds bracket a lone blue: the blue is hemmed in by a pincer, while
        // each red faces only the single blue, so only the blue team bleeds.
        let cars = [
            blue(Vec2::ZERO),
            red(Vec2::new(50.0, 0.0)),
            red(Vec2::new(-50.0, 0.0)),
        ];
        let damage = pincer_ram_damage(&cars);
        assert_near(damage.player, PINCER_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_bigger_swarm_bites_harder_but_still_lands_once() {
        // Three reds swarm one blue: the struck car bleeds a single, swarm-scaled
        // pincer (the three-attacker bite), not one charge per attacker. The
        // per-victim model holds (mirroring the broadside bonus), it just scales.
        let cars = [
            blue(Vec2::ZERO),
            red(Vec2::new(50.0, 0.0)),
            red(Vec2::new(-50.0, 0.0)),
            red(Vec2::new(0.0, 50.0)),
        ];
        let damage = pincer_ram_damage(&cars);
        assert_near(damage.player, pincer_ram_bonus(3));
        assert!(
            damage.player > PINCER_RAM_DAMAGE_PER_FRAME,
            "a three-car swarm must out-bite a two-car pincer: {}",
            damage.player
        );
        assert!(
            damage.player < 3.0 * PINCER_RAM_DAMAGE_PER_FRAME,
            "the scaled charge must not stack one full pincer per attacker: {}",
            damage.player
        );
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn pincer_bonus_is_zero_below_the_minimum_gang_up() {
        assert_near(pincer_ram_bonus(0), 0.0);
        assert_near(pincer_ram_bonus(PINCER_MIN_ATTACKERS - 1), 0.0);
        assert_near(
            pincer_ram_bonus(PINCER_MIN_ATTACKERS),
            PINCER_RAM_DAMAGE_PER_FRAME,
        );
    }

    #[test]
    fn pincer_bonus_rises_with_every_extra_attacker() {
        let two = pincer_ram_bonus(2);
        let three = pincer_ram_bonus(3);
        let four = pincer_ram_bonus(4);
        assert_near(two, PINCER_RAM_DAMAGE_PER_FRAME);
        // Each extra attacker adds exactly one per-extra step to the bite.
        assert_near(two - PINCER_RAM_DAMAGE_PER_FRAME, 0.0);
        assert_near(three - two, PINCER_RAM_DAMAGE_PER_EXTRA_ATTACKER);
        assert_near(four - three, PINCER_RAM_DAMAGE_PER_EXTRA_ATTACKER);
        assert!(three > two && four > three, "swarm bite must escalate");
    }

    #[test]
    fn pincer_bonus_caps_at_the_swarm_ceiling() {
        let max = PINCER_MAX_RAM_DAMAGE_PER_FRAME;
        // One past the cap and a huge dogpile both land at the ceiling, no more.
        let beyond_cap = PINCER_MIN_ATTACKERS + PINCER_MAX_EXTRA_ATTACKERS + 1;
        assert_near(pincer_ram_bonus(beyond_cap), max);
        assert_near(pincer_ram_bonus(64), max);
        assert!(
            max < NITRO_RAM_DAMAGE_PER_FRAME,
            "even a maxed swarm must stay under the earned nitro charge: {max}"
        );
    }

    #[test]
    fn a_growing_swarm_grinds_the_victim_down_harder() {
        // The same lone blue, hemmed in by two then three then four reds, bleeds a
        // strictly heavier pincer each time another foe piles in.
        let two = pincer_ram_damage(&[
            blue(Vec2::ZERO),
            red(Vec2::new(50.0, 0.0)),
            red(Vec2::new(-50.0, 0.0)),
        ])
        .player;
        let three = pincer_ram_damage(&[
            blue(Vec2::ZERO),
            red(Vec2::new(50.0, 0.0)),
            red(Vec2::new(-50.0, 0.0)),
            red(Vec2::new(0.0, 50.0)),
        ])
        .player;
        let four = pincer_ram_damage(&[
            blue(Vec2::ZERO),
            red(Vec2::new(50.0, 0.0)),
            red(Vec2::new(-50.0, 0.0)),
            red(Vec2::new(0.0, 50.0)),
            red(Vec2::new(0.0, -50.0)),
        ])
        .player;
        assert!(
            three > two && four > three,
            "the surrounded team must bleed more as the swarm grows: {two} {three} {four}"
        );
    }

    #[test]
    fn friendly_crowding_is_no_pincer() {
        // A car flanked by its own teammates is not pincered: only enemies count.
        let cars = [
            blue(Vec2::ZERO),
            blue(Vec2::new(50.0, 0.0)),
            blue(Vec2::new(-50.0, 0.0)),
        ];
        let damage = pincer_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_pincer_needs_contact() {
        // Two reds bracket a blue but both sit out of ram range: no pincer.
        let cars = [
            blue(Vec2::ZERO),
            red(Vec2::new(RAM_RADIUS + 1.0, 0.0)),
            red(Vec2::new(-(RAM_RADIUS + 1.0), 0.0)),
        ];
        let damage = pincer_ram_damage(&cars);
        assert_near(damage.player, 0.0);
        assert_near(damage.opponent, 0.0);
    }

    #[test]
    fn a_mutual_pincer_wears_both_teams() {
        // Two blues and two reds bunch together so every car has both enemies in
        // range: each of the four is pincered, so each team bleeds two pincers.
        let cars = [
            blue(Vec2::ZERO),
            blue(Vec2::new(20.0, 0.0)),
            red(Vec2::new(0.0, 20.0)),
            red(Vec2::new(20.0, 20.0)),
        ];
        let damage = pincer_ram_damage(&cars);
        assert_near(damage.player, 2.0 * PINCER_RAM_DAMAGE_PER_FRAME);
        assert_near(damage.opponent, 2.0 * PINCER_RAM_DAMAGE_PER_FRAME);
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
        // Base scrape wears both teams 0.25; the head-on charge adds the
        // aggressor bonus to each, and a dead-on hit triggers neither the flank
        // nor the rear bonus.
        assert_near(
            integrity.player,
            MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME - AGGRESSOR_RAM_DAMAGE_PER_FRAME,
        );
        assert_near(
            integrity.opponent,
            MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME - AGGRESSOR_RAM_DAMAGE_PER_FRAME,
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

    #[test]
    fn base_repair_heals_a_team_parked_in_its_own_base() {
        let cars = [(AiTeam::Blue, BLUE_HOME)];
        let repair = base_repair(&cars, BLUE_HOME, RED_HOME);
        assert_near(repair.player, BASE_REPAIR_PER_FRAME);
        assert_near(repair.opponent, 0.0);
    }

    #[test]
    fn base_repair_ignores_a_car_just_outside_its_base_radius() {
        let cars = [(
            AiTeam::Blue,
            BLUE_HOME + Vec2::new(BASE_REPAIR_RADIUS + 1.0, 0.0),
        )];
        let repair = base_repair(&cars, BLUE_HOME, RED_HOME);
        assert_near(repair.player, 0.0);
        assert_near(repair.opponent, 0.0);
    }

    #[test]
    fn base_repair_ignores_a_car_loitering_in_the_enemy_base() {
        // A blue car sitting on the red base earns neither team a heal.
        let cars = [(AiTeam::Blue, RED_HOME)];
        let repair = base_repair(&cars, BLUE_HOME, RED_HOME);
        assert_near(repair.player, 0.0);
        assert_near(repair.opponent, 0.0);
    }

    #[test]
    fn base_repair_heals_each_team_independently() {
        let cars = [(AiTeam::Blue, BLUE_HOME), (AiTeam::Red, RED_HOME)];
        let repair = base_repair(&cars, BLUE_HOME, RED_HOME);
        assert_near(repair.player, BASE_REPAIR_PER_FRAME);
        assert_near(repair.opponent, BASE_REPAIR_PER_FRAME);
    }

    #[test]
    fn base_repair_pools_per_team_so_a_second_home_car_adds_nothing() {
        let cars = [
            (AiTeam::Blue, BLUE_HOME),
            (AiTeam::Blue, BLUE_HOME + Vec2::new(10.0, 0.0)),
        ];
        let repair = base_repair(&cars, BLUE_HOME, RED_HOME);
        assert_near(repair.player, BASE_REPAIR_PER_FRAME);
    }

    #[test]
    fn apply_base_repair_caps_each_team_at_full_integrity() {
        let mut integrity = VehicleIntegrity::default();
        integrity.apply_base_repair(BaseRepair {
            player: BASE_REPAIR_PER_FRAME,
            opponent: BASE_REPAIR_PER_FRAME,
        });
        assert_near(integrity.player, MAX_INTEGRITY);
        assert_near(integrity.opponent, MAX_INTEGRITY);
    }

    #[test]
    fn apply_base_repair_lifts_only_the_recovering_team() {
        let mut integrity = VehicleIntegrity {
            player: 10.0,
            opponent: 50.0,
        };
        integrity.apply_base_repair(BaseRepair {
            player: BASE_REPAIR_PER_FRAME,
            opponent: 0.0,
        });
        assert_near(integrity.player, 10.0 + BASE_REPAIR_PER_FRAME);
        assert_near(integrity.opponent, 50.0);
    }

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
    fn armour_halves_only_a_shielded_teams_damage() {
        let damage = TeamDamage {
            player: 2.0,
            opponent: 4.0,
        };
        let mitigated = armour_mitigated_damage(
            damage,
            RamShield {
                player: true,
                opponent: false,
            },
        );
        assert_near(mitigated.player, 2.0 * SHIELD_DAMAGE_MULTIPLIER);
        assert_near(mitigated.opponent, 4.0);
    }

    #[test]
    fn armour_passes_unshielded_damage_through_untouched() {
        let damage = TeamDamage {
            player: 2.0,
            opponent: 4.0,
        };
        let mitigated = armour_mitigated_damage(damage, RamShield::default());
        assert_near(mitigated.player, 2.0);
        assert_near(mitigated.opponent, 4.0);
    }

    #[test]
    fn ram_shield_reads_active_armour_timers() {
        let mut boosts = ArmourBoosts::default();
        boosts.trigger_opponent();
        let shield = RamShield::from_armour(&boosts);
        assert!(!shield.player, "an idle team should not be shielded");
        assert!(shield.opponent, "a triggered team should be shielded");
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
            false,
            true,
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
            false,
            false,
        );

        assert_eq!(bounties.player, WRECK_CASH_BOUNTY);
        assert_eq!(bounties.player_most_wanted, 0);
        assert_eq!(bounties.player_carrier_takedown, 0);
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
    fn wreck_stuns_default_to_inactive_for_both_teams() {
        let stuns = WreckStuns::default();
        assert_eq!(stuns.player_frames, 0);
        assert_eq!(stuns.opponent_frames, 0);
        assert_near(stuns.player_multiplier(), 1.0);
        assert_near(stuns.opponent_multiplier(), 1.0);
    }

    #[test]
    fn triggering_a_stun_spins_out_only_that_team() {
        let mut stuns = WreckStuns::default();
        stuns.trigger_opponent();
        assert_eq!(stuns.opponent_frames, WRECK_STUN_FRAMES);
        assert_near(stuns.opponent_multiplier(), WRECK_STUN_SPEED_MULTIPLIER);
        // The wrecking team keeps full speed; only the wreck spins out.
        assert_eq!(stuns.player_frames, 0);
        assert_near(stuns.player_multiplier(), 1.0);
    }

    #[test]
    fn a_spin_out_expires_after_its_window() {
        let mut stuns = WreckStuns::default();
        stuns.trigger_player();
        for _ in 0..WRECK_STUN_FRAMES {
            assert_near(stuns.player_multiplier(), WRECK_STUN_SPEED_MULTIPLIER);
            stuns.tick();
        }
        assert_near(stuns.player_multiplier(), 1.0);
        // Ticking a spent timer never underflows.
        stuns.tick();
        assert_eq!(stuns.player_frames, 0);
    }

    #[test]
    fn stun_multiplier_for_team_routes_to_the_right_pool() {
        let mut stuns = WreckStuns::default();
        stuns.trigger_player();
        assert_near(
            stuns.multiplier_for_team(AiTeam::Blue),
            WRECK_STUN_SPEED_MULTIPLIER,
        );
        assert_near(stuns.multiplier_for_team(AiTeam::Red), 1.0);
    }

    #[test]
    fn apply_wrecks_spins_out_each_wrecked_team() {
        let mut player_only = WreckStuns::default();
        player_only.apply_wrecks(WreckEvents {
            player: true,
            opponent: false,
        });
        assert_eq!(player_only.player_frames, WRECK_STUN_FRAMES);
        assert_eq!(player_only.opponent_frames, 0);

        let mut both = WreckStuns::default();
        both.apply_wrecks(WreckEvents {
            player: true,
            opponent: true,
        });
        assert_eq!(both.player_frames, WRECK_STUN_FRAMES);
        assert_eq!(both.opponent_frames, WRECK_STUN_FRAMES);

        let mut quiet = WreckStuns::default();
        quiet.apply_wrecks(WreckEvents::default());
        assert_eq!(quiet, WreckStuns::default());
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
    fn wreck_surges_default_to_inactive_for_both_teams() {
        let surges = WreckSurges::default();
        assert_eq!(surges.player_frames, 0);
        assert_eq!(surges.opponent_frames, 0);
        assert_near(surges.player_multiplier(), 1.0);
        assert_near(surges.opponent_multiplier(), 1.0);
    }

    #[test]
    fn a_surge_speeds_only_the_team_that_landed_the_kill() {
        let mut surges = WreckSurges::default();
        surges.trigger_player();
        assert_eq!(surges.player_frames, WRECK_SURGE_FRAMES);
        assert_near(surges.player_multiplier(), WRECK_SURGE_SPEED_MULTIPLIER);
        // The wrecked team gets no surge; only the wrecker speeds up.
        assert_eq!(surges.opponent_frames, 0);
        assert_near(surges.opponent_multiplier(), 1.0);
    }

    #[test]
    fn a_surge_expires_after_its_window() {
        let mut surges = WreckSurges::default();
        surges.trigger_opponent();
        for _ in 0..WRECK_SURGE_FRAMES {
            assert_near(surges.opponent_multiplier(), WRECK_SURGE_SPEED_MULTIPLIER);
            surges.tick();
        }
        assert_near(surges.opponent_multiplier(), 1.0);
        // Ticking a spent timer never underflows.
        surges.tick();
        assert_eq!(surges.opponent_frames, 0);
    }

    #[test]
    fn surge_multiplier_for_team_routes_to_the_right_pool() {
        let mut surges = WreckSurges::default();
        surges.trigger_opponent();
        assert_near(
            surges.multiplier_for_team(AiTeam::Red),
            WRECK_SURGE_SPEED_MULTIPLIER,
        );
        assert_near(surges.multiplier_for_team(AiTeam::Blue), 1.0);
    }

    #[test]
    fn reward_wreckers_surges_the_enemy_of_each_wrecked_team() {
        // A wrecked opponent means the player team dealt the kill and surges.
        let mut player_dealt = WreckSurges::default();
        player_dealt.reward_wreckers(WreckEvents {
            player: false,
            opponent: true,
        });
        assert_eq!(player_dealt.player_frames, WRECK_SURGE_FRAMES);
        assert_eq!(player_dealt.opponent_frames, 0);

        // A wrecked player team means the opponents dealt the kill and surge.
        let mut opponent_dealt = WreckSurges::default();
        opponent_dealt.reward_wreckers(WreckEvents {
            player: true,
            opponent: false,
        });
        assert_eq!(opponent_dealt.opponent_frames, WRECK_SURGE_FRAMES);
        assert_eq!(opponent_dealt.player_frames, 0);

        // A double wreck surges both teams at once.
        let mut both = WreckSurges::default();
        both.reward_wreckers(WreckEvents {
            player: true,
            opponent: true,
        });
        assert_eq!(both.player_frames, WRECK_SURGE_FRAMES);
        assert_eq!(both.opponent_frames, WRECK_SURGE_FRAMES);

        // A quiet frame surges nobody.
        let mut quiet = WreckSurges::default();
        quiet.reward_wreckers(WreckEvents::default());
        assert_eq!(quiet, WreckSurges::default());
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
        }
    }
}
