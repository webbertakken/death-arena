use crate::gameplay::ctf::{CtfFlag, CtfMatchResult};
use crate::gameplay::pickup::{NitroBoosts, OpponentScore, Score};
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

/// Wears down both teams whenever their cars are trading paint, and pays a
/// wreck bounty to whichever team grinds an enemy down to zero this frame.
#[allow(clippy::too_many_arguments)]
pub fn ram_damage_system(
    match_result: Option<Res<CtfMatchResult>>,
    nitro_boosts: Option<Res<NitroBoosts>>,
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
    let damage = ram_damage(&cars)
        .combined(nitro_ram_damage(&cars, boost))
        .combined(carrier_ram_damage(&cars))
        .combined(aggressor_ram_damage(&cars));

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
    if wrecks.any() {
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
        for (flag_entity, mut flag) in &mut flag_query {
            if dropped.contains(&flag_entity) {
                flag.holder = None;
            }
        }
    }

    let before_streaks = wreck_streaks.as_deref().copied().unwrap_or_default();
    let payout = resolve_wreck_streaks(before_streaks, wrecks);
    if let Some(streaks) = wreck_streaks.as_deref_mut() {
        *streaks = payout.streaks;
    }

    if wrecks.any() {
        info!(
            "Wreck! player_down={} opponent_down={}; rampage streaks player={} opponent={}; \
             banking player_bounty={} opponent_bounty={}",
            wrecks.player,
            wrecks.opponent,
            payout.streaks.player,
            payout.streaks.opponent,
            payout.player_bounty,
            payout.opponent_bounty,
        );
    }

    // The wrecking team banks the bounty: a wrecked opponent pays the player
    // team, a wrecked player team pays the opponents. Each consecutive wreck in
    // a rampage pays more, up to the streak cap.
    if payout.player_bounty > 0 {
        if let Some(score) = score.as_deref_mut() {
            score.bank_wreck_bounty(payout.player_bounty);
        }
    }
    if payout.opponent_bounty > 0 {
        if let Some(opponent_score) = opponent_score.as_deref_mut() {
            opponent_score.bank_wreck_bounty(payout.opponent_bounty);
        }
    }
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
            .add_system(ram_damage_system);
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
    fn system_adds_aggressor_ram_bonus_when_a_car_charges() {
        let mut app = App::new();
        app.init_resource::<VehicleIntegrity>();
        app.add_system(ram_damage_system);
        // The player charges head-first (+X) into the red car ahead of it.
        app.world.spawn((
            player_stub(),
            Transform::from_translation(Vec3::ZERO)
                .with_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)),
        ));
        // The red car faces +Y by default, so it only takes the charge, it does
        // not deal one back.
        app.world.spawn((
            virtual_player_stub(AiTeam::Red),
            Transform::from_translation(Vec3::new(30.0, 0.0, 0.0)),
        ));

        app.update();

        let integrity = app.world.resource::<VehicleIntegrity>();
        // Base scrape wears both teams 0.25; the charging player also wears the
        // red team for the aggressor bonus on top.
        assert_near(integrity.player, MAX_INTEGRITY - RAM_DAMAGE_PER_FRAME);
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
