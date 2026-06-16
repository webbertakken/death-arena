use crate::{App, AppState, Plugin};
use bevy::prelude::*;

pub mod collect;
mod spawn;
mod system;

pub use collect::PickupKind;

/// World-space distance at which a car collects a pickup it drives over.
pub const PICKUP_RADIUS: f32 = 120.0;
/// Number of fixed update frames before a collected pickup returns.
pub const PICKUP_RESPAWN_FRAMES: u32 = 600;
/// Number of fixed update frames a nitro pickup boosts a car.
pub const NITRO_BOOST_FRAMES: u32 = 180;
/// Speed multiplier applied while a nitro boost is active.
pub const NITRO_SPEED_MULTIPLIER: f32 = 1.5;
/// Number of fixed update frames a shield pickup armours a team.
///
/// Longer than the nitro window (5s at 60 FPS) so a defensive grab buys a real
/// breather to limp home or trade paint on the front foot, the counter-play to
/// the all-offence ramming loop.
pub const SHIELD_BOOST_FRAMES: u32 = 300;
/// A shield must outlast a nitro burst to feel like a real breather, enforced at
/// compile time.
const _: () = assert!(SHIELD_BOOST_FRAMES > NITRO_BOOST_FRAMES);
/// Number of fixed update frames a sabotage charge slows the enemy team.
///
/// Pitched to match the nitro window (3s at 60 FPS): a sabotage is the offensive
/// mirror of nitro, so a charge denies roughly a nitro's worth of speed.
pub const SABOTAGE_FRAMES: u32 = 180;
/// Speed multiplier applied to a team whose engines are sabotaged.
///
/// A 15% slow: meaningful (it blunts a getaway or a counter-attack and helps run
/// down a fleeing flag carrier) without being a crippling stop. An enemy nitro
/// burst still nets a gain on top of it (`1.5 * 0.85 = 1.275`), so sabotage
/// blunts a boost rather than negating it, keeping the counter-play alive.
pub const SABOTAGE_SPEED_MULTIPLIER: f32 = 0.85;
/// A sabotage must slow, never speed up or stop, the enemy, enforced at compile
/// time.
const _: () = assert!(SABOTAGE_SPEED_MULTIPLIER > 0.0 && SABOTAGE_SPEED_MULTIPLIER < 1.0);

/// A trackside collectible the player drives over to bank a bounty.
#[derive(Component, Debug)]
pub struct Pickup {
    pub kind: PickupKind,
}

/// A pickup waiting to return to the arena after collection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PendingPickupRespawn {
    pub kind: PickupKind,
    pub position: Vec2,
    pub frames_remaining: u32,
}

/// Queue of pickups that will respawn after a short cooldown.
#[derive(Resource, Default, Debug, PartialEq)]
pub struct PickupRespawns {
    pub pending: Vec<PendingPickupRespawn>,
}

impl PickupRespawns {
    pub fn queue(&mut self, kind: PickupKind, position: Vec2) {
        self.pending.push(PendingPickupRespawn {
            kind,
            position,
            frames_remaining: PICKUP_RESPAWN_FRAMES,
        });
    }
}

/// Timed nitro boosts currently active for the player and opponent team.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct NitroBoosts {
    pub player_frames: u32,
    pub opponent_frames: u32,
}

impl NitroBoosts {
    pub const fn player_multiplier(&self) -> f32 {
        if self.player_frames > 0 {
            NITRO_SPEED_MULTIPLIER
        } else {
            1.0
        }
    }

    pub const fn opponent_multiplier(&self) -> f32 {
        if self.opponent_frames > 0 {
            NITRO_SPEED_MULTIPLIER
        } else {
            1.0
        }
    }

    /// Whether the player team's nitro boost is currently burning.
    pub const fn is_player_active(&self) -> bool {
        self.player_frames > 0
    }

    /// Whether the opponent team's nitro boost is currently burning.
    pub const fn is_opponent_active(&self) -> bool {
        self.opponent_frames > 0
    }

    pub const fn trigger_player(&mut self) {
        self.player_frames = NITRO_BOOST_FRAMES;
    }

    pub const fn trigger_opponent(&mut self) {
        self.opponent_frames = NITRO_BOOST_FRAMES;
    }

    pub const fn tick(&mut self) {
        self.player_frames = self.player_frames.saturating_sub(1);
        self.opponent_frames = self.opponent_frames.saturating_sub(1);
    }
}

/// Timed shield armour currently active for the player and opponent team.
///
/// The defensive mirror of [`NitroBoosts`]: where nitro makes a team's rams
/// bite, a shield blunts the ram damage that team takes. Same per-team frame
/// timer, armed by collecting a [`PickupKind::Shield`], wound down each frame by
/// the decay system, and read by combat to mitigate incoming wear.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct ArmourBoosts {
    pub player_frames: u32,
    pub opponent_frames: u32,
}

impl ArmourBoosts {
    /// Whether the player team's shield is currently up.
    pub const fn is_player_active(&self) -> bool {
        self.player_frames > 0
    }

    /// Whether the opponent team's shield is currently up.
    pub const fn is_opponent_active(&self) -> bool {
        self.opponent_frames > 0
    }

    pub const fn trigger_player(&mut self) {
        self.player_frames = SHIELD_BOOST_FRAMES;
    }

    pub const fn trigger_opponent(&mut self) {
        self.opponent_frames = SHIELD_BOOST_FRAMES;
    }

    pub const fn tick(&mut self) {
        self.player_frames = self.player_frames.saturating_sub(1);
        self.opponent_frames = self.opponent_frames.saturating_sub(1);
    }
}

/// Timed engine sabotage currently slowing the player and opponent team.
///
/// The offensive mirror of [`NitroBoosts`]: where nitro speeds a team's *own*
/// cars up, a sabotage charge slows the *enemy* team down. Same per-team frame
/// timer, but armed by the opposing side: a [`PickupKind::Sabotage`] collected
/// by one team winds up the other team's timer (see the collection system).
/// Wound down each frame by the decay system and read by both movement systems
/// to throttle the sabotaged team.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct SabotageEffects {
    /// Frames the player team's engines remain sabotaged (slowed).
    pub player_frames: u32,
    /// Frames the opponent team's engines remain sabotaged (slowed).
    pub opponent_frames: u32,
}

impl SabotageEffects {
    /// Speed multiplier the player team carries while sabotaged.
    pub const fn player_multiplier(&self) -> f32 {
        if self.player_frames > 0 {
            SABOTAGE_SPEED_MULTIPLIER
        } else {
            1.0
        }
    }

    /// Speed multiplier the opponent team carries while sabotaged.
    pub const fn opponent_multiplier(&self) -> f32 {
        if self.opponent_frames > 0 {
            SABOTAGE_SPEED_MULTIPLIER
        } else {
            1.0
        }
    }

    /// Sabotage the player team: the opponents collected a sabotage charge.
    pub const fn sabotage_player(&mut self) {
        self.player_frames = SABOTAGE_FRAMES;
    }

    /// Sabotage the opponent team: the player team collected a sabotage charge.
    pub const fn sabotage_opponent(&mut self) {
        self.opponent_frames = SABOTAGE_FRAMES;
    }

    pub const fn tick(&mut self) {
        self.player_frames = self.player_frames.saturating_sub(1);
        self.opponent_frames = self.opponent_frames.saturating_sub(1);
    }
}

/// Running tally of what the player has collected this session.
///
/// Mirrors the Death Rally loop where banked cash drives upgrades.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct Score {
    /// Total cash banked from pickups and capture bonuses.
    pub cash: u32,
    /// Number of pickups collected.
    pub collected: u32,
    /// Number of CTF captures rewarded to the player team.
    pub captures: u32,
    /// Number of enemy flag steals rewarded to the player team.
    pub steals: u32,
    /// Number of home flag returns rewarded to the player team.
    pub returns: u32,
    /// Number of enemy cars the player team wrecked by ramming.
    pub wrecks: u32,
}

impl Score {
    /// Apply a collected pickup's reward to the tally.
    pub const fn collect(&mut self, kind: PickupKind) {
        self.cash += kind.bounty();
        self.collected += 1;
    }

    /// Apply flag capture rewards to the tally.
    pub const fn bank_capture_bonus(&mut self, captures: u32, bounty: u32) {
        self.cash += captures * bounty;
        self.captures += captures;
    }

    /// Apply enemy flag steal rewards to the tally.
    pub const fn bank_flag_steal_bonus(&mut self, steals: u32, bounty: u32) {
        self.cash += steals * bounty;
        self.steals += steals;
    }

    /// Apply home flag return rewards to the tally.
    pub const fn bank_flag_return_bonus(&mut self, returns: u32, bounty: u32) {
        self.cash += returns * bounty;
        self.returns += returns;
    }

    /// Bank a wreck bounty for grinding an enemy car down to a full wreck.
    pub const fn bank_wreck_bounty(&mut self, bounty: u32) {
        self.cash += bounty;
        self.wrecks += 1;
    }

    /// Bank an end-of-match purse: the Death Rally race winnings a team pockets
    /// for taking the round. Pure cash, banked on top of every in-match bounty,
    /// so it leaves the play tallies untouched.
    pub const fn bank_match_purse(&mut self, purse: u32) {
        self.cash += purse;
    }

    /// Bank a comeback bonus: the anti-snowball cash a team behind on captures
    /// pockets for clawing one back. Pure cash on top of the capture bounty, so
    /// it leaves the play tallies untouched.
    pub const fn bank_comeback_capture_bonus(&mut self, bonus: u32) {
        self.cash += bonus;
    }
}

/// Running tally of pickups stolen by virtual opponents.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct OpponentScore {
    /// Total cash banked by virtual players.
    pub cash: u32,
    /// Number of pickups collected by virtual players.
    pub collected: u32,
    /// Number of CTF captures rewarded to virtual opponents.
    pub captures: u32,
    /// Number of enemy flag steals rewarded to virtual opponents.
    pub steals: u32,
    /// Number of home flag returns rewarded to virtual opponents.
    pub returns: u32,
    /// Number of player-team cars the opponents wrecked by ramming.
    pub wrecks: u32,
}

impl OpponentScore {
    /// Apply a collected pickup's reward to the opponent tally.
    pub const fn collect(&mut self, kind: PickupKind) {
        self.cash += kind.bounty();
        self.collected += 1;
    }

    /// Apply flag capture rewards to the opponent tally.
    pub const fn bank_capture_bonus(&mut self, captures: u32, bounty: u32) {
        self.cash += captures * bounty;
        self.captures += captures;
    }

    /// Apply enemy flag steal rewards to the opponent tally.
    pub const fn bank_flag_steal_bonus(&mut self, steals: u32, bounty: u32) {
        self.cash += steals * bounty;
        self.steals += steals;
    }

    /// Apply home flag return rewards to the tally.
    pub const fn bank_flag_return_bonus(&mut self, returns: u32, bounty: u32) {
        self.cash += returns * bounty;
        self.returns += returns;
    }

    /// Bank a wreck bounty for grinding a player-team car down to a full wreck.
    pub const fn bank_wreck_bounty(&mut self, bounty: u32) {
        self.cash += bounty;
        self.wrecks += 1;
    }

    /// Bank an end-of-match purse: the race winnings the opponents pocket for
    /// taking the round. Pure cash, leaving the play tallies untouched.
    pub const fn bank_match_purse(&mut self, purse: u32) {
        self.cash += purse;
    }

    /// Bank a comeback bonus: the anti-snowball cash a team behind on captures
    /// pockets for clawing one back. Pure cash on top of the capture bounty, so
    /// it leaves the play tallies untouched.
    pub const fn bank_comeback_capture_bonus(&mut self, bonus: u32) {
        self.cash += bonus;
    }
}

#[derive(Default)]
pub struct PickupPlugin;

impl Plugin for PickupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Score>()
            .init_resource::<OpponentScore>()
            .init_resource::<NitroBoosts>()
            .init_resource::<ArmourBoosts>()
            .init_resource::<SabotageEffects>()
            .init_resource::<PickupRespawns>()
            .add_system_set(
                SystemSet::on_enter(AppState::InGame)
                    .with_system(reset_pickup_match_resources)
                    .with_system(spawn::setup),
            );
        app.add_system(system::nitro_boost_decay_system.before(system::pickup_collection_system))
            .add_system(system::armour_boost_decay_system.before(system::pickup_collection_system))
            .add_system(
                system::sabotage_effect_decay_system.before(system::pickup_collection_system),
            )
            .add_system(system::pickup_collection_system)
            .add_system(system::pickup_respawn_system.after(system::pickup_collection_system));
    }
}

fn reset_pickup_match_resources(
    mut score: ResMut<Score>,
    mut opponent_score: ResMut<OpponentScore>,
    mut nitro_boosts: ResMut<NitroBoosts>,
    mut armour_boosts: ResMut<ArmourBoosts>,
    mut sabotage_effects: ResMut<SabotageEffects>,
    mut respawns: ResMut<PickupRespawns>,
) {
    *score = Score::default();
    *opponent_score = OpponentScore::default();
    *nitro_boosts = NitroBoosts::default();
    *armour_boosts = ArmourBoosts::default();
    *sabotage_effects = SabotageEffects::default();
    *respawns = PickupRespawns::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collecting_accumulates_cash_and_count() {
        let mut score = Score::default();
        score.collect(PickupKind::Cash);
        score.collect(PickupKind::Repair);
        assert_eq!(
            score,
            Score {
                cash: PickupKind::Cash.bounty() + PickupKind::Repair.bounty(),
                collected: 2,
                captures: 0,
                steals: 0,
                returns: 0,
                wrecks: 0,
            }
        );
    }

    #[test]
    fn capture_bonus_accumulates_cash_and_capture_count() {
        let mut score = Score::default();
        score.bank_capture_bonus(2, 250);
        assert_eq!(
            score,
            Score {
                cash: 500,
                collected: 0,
                captures: 2,
                steals: 0,
                returns: 0,
                wrecks: 0,
            }
        );
    }

    #[test]
    fn flag_steal_bonus_accumulates_cash_and_steal_count() {
        let mut score = Score::default();
        score.bank_flag_steal_bonus(2, 50);
        assert_eq!(
            score,
            Score {
                cash: 100,
                collected: 0,
                captures: 0,
                steals: 2,
                returns: 0,
                wrecks: 0,
            }
        );
    }

    #[test]
    fn flag_return_bonus_accumulates_cash_and_return_count() {
        let mut score = Score::default();
        score.bank_flag_return_bonus(2, 75);
        assert_eq!(
            score,
            Score {
                cash: 150,
                collected: 0,
                captures: 0,
                steals: 0,
                returns: 2,
                wrecks: 0,
            }
        );
    }

    #[test]
    fn wreck_bounty_accumulates_cash_and_wreck_count() {
        let mut score = Score::default();
        score.bank_wreck_bounty(150);
        score.bank_wreck_bounty(150);
        assert_eq!(
            score,
            Score {
                cash: 300,
                collected: 0,
                captures: 0,
                steals: 0,
                returns: 0,
                wrecks: 2,
            }
        );
    }

    #[test]
    fn match_purse_banks_pure_cash_without_touching_tallies() {
        let mut score = Score {
            cash: 200,
            collected: 3,
            captures: 1,
            steals: 2,
            returns: 1,
            wrecks: 4,
        };
        score.bank_match_purse(1000);
        assert_eq!(
            score,
            Score {
                cash: 1200,
                collected: 3,
                captures: 1,
                steals: 2,
                returns: 1,
                wrecks: 4,
            },
            "a victory purse should add cash only, leaving every play tally untouched"
        );
    }

    #[test]
    fn opponent_collecting_accumulates_cash_and_count() {
        let mut score = OpponentScore::default();
        score.collect(PickupKind::Nitro);
        score.collect(PickupKind::Cash);
        assert_eq!(
            score,
            OpponentScore {
                cash: PickupKind::Nitro.bounty() + PickupKind::Cash.bounty(),
                collected: 2,
                captures: 0,
                steals: 0,
                returns: 0,
                wrecks: 0,
            }
        );
    }

    #[test]
    fn opponent_capture_bonus_accumulates_cash_and_capture_count() {
        let mut score = OpponentScore::default();
        score.bank_capture_bonus(1, 250);
        assert_eq!(
            score,
            OpponentScore {
                cash: 250,
                collected: 0,
                captures: 1,
                steals: 0,
                returns: 0,
                wrecks: 0,
            }
        );
    }

    #[test]
    fn opponent_flag_steal_bonus_accumulates_cash_and_steal_count() {
        let mut score = OpponentScore::default();
        score.bank_flag_steal_bonus(1, 50);
        assert_eq!(
            score,
            OpponentScore {
                cash: 50,
                collected: 0,
                captures: 0,
                steals: 1,
                returns: 0,
                wrecks: 0,
            }
        );
    }

    #[test]
    fn opponent_flag_return_bonus_accumulates_cash_and_return_count() {
        let mut score = OpponentScore::default();
        score.bank_flag_return_bonus(1, 75);
        assert_eq!(
            score,
            OpponentScore {
                cash: 75,
                collected: 0,
                captures: 0,
                steals: 0,
                returns: 1,
                wrecks: 0,
            }
        );
    }

    #[test]
    fn opponent_wreck_bounty_accumulates_cash_and_wreck_count() {
        let mut score = OpponentScore::default();
        score.bank_wreck_bounty(150);
        assert_eq!(
            score,
            OpponentScore {
                cash: 150,
                collected: 0,
                captures: 0,
                steals: 0,
                returns: 0,
                wrecks: 1,
            }
        );
    }

    #[test]
    fn opponent_match_purse_banks_pure_cash_without_touching_tallies() {
        let mut score = OpponentScore {
            cash: 75,
            collected: 1,
            captures: 0,
            steals: 1,
            returns: 0,
            wrecks: 2,
        };
        score.bank_match_purse(1000);
        assert_eq!(
            score,
            OpponentScore {
                cash: 1075,
                collected: 1,
                captures: 0,
                steals: 1,
                returns: 0,
                wrecks: 2,
            },
            "an opponent victory purse should add cash only, leaving every tally untouched"
        );
    }

    fn assert_multiplier_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= f32::EPSILON,
            "actual={actual}, expected={expected}"
        );
    }

    #[test]
    fn nitro_boost_starts_active_and_expires() {
        let mut boosts = NitroBoosts::default();

        boosts.trigger_player();
        boosts.trigger_opponent();

        assert_multiplier_eq(boosts.player_multiplier(), NITRO_SPEED_MULTIPLIER);
        assert_multiplier_eq(boosts.opponent_multiplier(), NITRO_SPEED_MULTIPLIER);
        assert_eq!(boosts.player_frames, NITRO_BOOST_FRAMES);
        assert_eq!(boosts.opponent_frames, NITRO_BOOST_FRAMES);

        for _ in 0..NITRO_BOOST_FRAMES {
            boosts.tick();
        }

        assert_multiplier_eq(boosts.player_multiplier(), 1.0);
        assert_multiplier_eq(boosts.opponent_multiplier(), 1.0);
        assert_eq!(boosts.player_frames, 0);
        assert_eq!(boosts.opponent_frames, 0);
    }

    #[test]
    fn active_flags_track_whether_a_team_is_boosting() {
        let mut boosts = NitroBoosts::default();
        assert!(!boosts.is_player_active());
        assert!(!boosts.is_opponent_active());

        boosts.trigger_player();
        assert!(boosts.is_player_active());
        assert!(!boosts.is_opponent_active());

        boosts.trigger_opponent();
        assert!(boosts.is_opponent_active());

        for _ in 0..NITRO_BOOST_FRAMES {
            boosts.tick();
        }
        assert!(!boosts.is_player_active());
        assert!(!boosts.is_opponent_active());
    }

    #[test]
    fn entering_match_resets_pickup_economy_and_timers() {
        let mut app = App::new();
        app.insert_resource(Score {
            cash: 500,
            collected: 2,
            captures: 1,
            steals: 1,
            returns: 1,
            wrecks: 1,
        });
        app.insert_resource(OpponentScore {
            cash: 300,
            collected: 1,
            captures: 1,
            steals: 1,
            returns: 1,
            wrecks: 1,
        });
        app.insert_resource(NitroBoosts {
            player_frames: 12,
            opponent_frames: 34,
        });
        app.insert_resource(ArmourBoosts {
            player_frames: 56,
            opponent_frames: 78,
        });
        app.insert_resource(SabotageEffects {
            player_frames: 90,
            opponent_frames: 11,
        });
        let mut respawns = PickupRespawns::default();
        respawns.queue(PickupKind::Cash, Vec2::new(1.0, 2.0));
        app.insert_resource(respawns);
        app.add_system(reset_pickup_match_resources);

        app.update();

        assert_eq!(*app.world.resource::<Score>(), Score::default());
        assert_eq!(
            *app.world.resource::<OpponentScore>(),
            OpponentScore::default()
        );
        assert_eq!(*app.world.resource::<NitroBoosts>(), NitroBoosts::default());
        assert_eq!(
            *app.world.resource::<ArmourBoosts>(),
            ArmourBoosts::default()
        );
        assert_eq!(
            *app.world.resource::<SabotageEffects>(),
            SabotageEffects::default()
        );
        assert_eq!(
            *app.world.resource::<PickupRespawns>(),
            PickupRespawns::default()
        );
    }

    #[test]
    fn sabotage_starts_active_and_expires() {
        let mut effects = SabotageEffects::default();
        assert_multiplier_eq(effects.player_multiplier(), 1.0);
        assert_multiplier_eq(effects.opponent_multiplier(), 1.0);

        effects.sabotage_player();
        effects.sabotage_opponent();

        assert_multiplier_eq(effects.player_multiplier(), SABOTAGE_SPEED_MULTIPLIER);
        assert_multiplier_eq(effects.opponent_multiplier(), SABOTAGE_SPEED_MULTIPLIER);
        assert_eq!(effects.player_frames, SABOTAGE_FRAMES);
        assert_eq!(effects.opponent_frames, SABOTAGE_FRAMES);

        for _ in 0..SABOTAGE_FRAMES {
            effects.tick();
        }

        assert_eq!(effects.player_frames, 0);
        assert_eq!(effects.opponent_frames, 0);
        assert_multiplier_eq(effects.player_multiplier(), 1.0);
        assert_multiplier_eq(effects.opponent_multiplier(), 1.0);
    }

    #[test]
    fn sabotaging_one_team_leaves_the_other_at_full_speed() {
        let mut effects = SabotageEffects::default();
        effects.sabotage_opponent();

        assert_multiplier_eq(effects.opponent_multiplier(), SABOTAGE_SPEED_MULTIPLIER);
        assert_eq!(effects.player_frames, 0);
        assert_multiplier_eq(effects.player_multiplier(), 1.0);
    }

    #[test]
    fn armour_boost_starts_up_and_expires() {
        let mut boosts = ArmourBoosts::default();
        assert!(!boosts.is_player_active());
        assert!(!boosts.is_opponent_active());

        boosts.trigger_player();
        boosts.trigger_opponent();
        assert!(boosts.is_player_active());
        assert!(boosts.is_opponent_active());
        assert_eq!(boosts.player_frames, SHIELD_BOOST_FRAMES);
        assert_eq!(boosts.opponent_frames, SHIELD_BOOST_FRAMES);

        for _ in 0..SHIELD_BOOST_FRAMES {
            boosts.tick();
        }

        assert!(!boosts.is_player_active());
        assert!(!boosts.is_opponent_active());
    }
}
