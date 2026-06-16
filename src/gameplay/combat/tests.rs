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
