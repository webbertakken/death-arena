use super::*;
use crate::gameplay::pickup::{OpponentScore, Score};

fn entity(id: u32) -> Entity {
    Entity::from_raw(id)
}

fn assert_near(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= f32::EPSILON,
        "actual={actual}, expected={expected}"
    );
}

#[test]
fn empty_handed_car_keeps_full_speed() {
    assert_near(flag_carrier_speed_multiplier(false), 1.0);
}

#[test]
fn flag_carrier_is_slowed_by_the_heavy_flag() {
    let carrying = flag_carrier_speed_multiplier(true);
    let empty_handed = flag_carrier_speed_multiplier(false);
    assert_near(carrying, FLAG_CARRIER_SPEED_MULTIPLIER);
    assert!(
        carrying < empty_handed,
        "carrying the flag must cost speed: carrying={carrying}, empty_handed={empty_handed}"
    );
    assert!(
        carrying > 0.0,
        "a carrier must still be able to move, multiplier={carrying}"
    );
}

#[test]
fn entering_match_resets_ctf_scores_and_result() {
    let mut app = App::new();
    app.insert_resource(CaptureScore {
        player: 2,
        opponents: 1,
    });
    app.insert_resource(FlagStealScore {
        player: 3,
        opponents: 4,
    });
    app.insert_resource(FlagReturnScore {
        player: 5,
        opponents: 6,
    });
    app.insert_resource(CtfMatchResult {
        winner: Some(CtfMatchWinner::Opponents),
    });
    app.insert_resource(MatchPursePaid(true));
    app.insert_resource(MatchClock {
        frames_remaining: 7,
        phase: MatchPhase::SuddenDeath,
    });
    app.insert_resource(LooseFlagTimers {
        blue_frames: 120,
        red_frames: 240,
    });
    app.insert_resource(FlagCarryTimers {
        blue_frames: 90,
        red_frames: 150,
    });
    app.add_system(reset_ctf_match_resources);

    app.update();

    assert_eq!(
        *app.world.resource::<CaptureScore>(),
        CaptureScore::default()
    );
    assert_eq!(
        *app.world.resource::<FlagStealScore>(),
        FlagStealScore::default()
    );
    assert_eq!(
        *app.world.resource::<FlagReturnScore>(),
        FlagReturnScore::default()
    );
    assert_eq!(
        *app.world.resource::<CtfMatchResult>(),
        CtfMatchResult::default()
    );
    assert_eq!(
        *app.world.resource::<MatchPursePaid>(),
        MatchPursePaid::default(),
        "a fresh match must clear the purse latch so the next win pays out"
    );
    assert_eq!(*app.world.resource::<MatchClock>(), MatchClock::default());
    assert_eq!(
        *app.world.resource::<LooseFlagTimers>(),
        LooseFlagTimers::default(),
        "a fresh match must clear loose-flag timers so a stale count never resets a flag"
    );
    assert_eq!(
        *app.world.resource::<FlagCarryTimers>(),
        FlagCarryTimers::default(),
        "a fresh match must clear carry timers so a carrier never starts the round tired"
    );
}

#[test]
fn auto_return_sends_an_abandoned_flag_home() {
    let home = Vec2::new(500.0, 0.0);
    let mut red = flag(1, FlagTeam::Red, home);
    red.position = Vec2::new(120.0, -40.0);
    let mut flags = [red];
    let mut timers = LooseFlagTimers {
        red_frames: FLAG_RESET_FRAMES - 1,
        ..Default::default()
    };

    auto_return_loose_flags(&mut flags, &mut timers);

    assert_eq!(
        flags[0].position, home,
        "an abandoned flag must reset to base"
    );
    assert_eq!(flags[0].holder, None);
    assert_eq!(timers.red_frames, 0, "the reset must clear the loose timer");
}

#[test]
fn auto_return_keeps_counting_a_still_loose_flag() {
    let mut blue = flag(2, FlagTeam::Blue, Vec2::ZERO);
    blue.position = Vec2::new(80.0, 80.0);
    let mut flags = [blue];
    let mut timers = LooseFlagTimers::default();

    auto_return_loose_flags(&mut flags, &mut timers);

    assert_eq!(timers.blue_frames, 1, "a loose flag must keep counting");
    assert_eq!(
        flags[0].position,
        Vec2::new(80.0, 80.0),
        "a flag inside the grace window must stay put"
    );
}

#[test]
fn auto_return_clears_the_timer_when_a_flag_is_recovered() {
    let mut red = flag(3, FlagTeam::Red, Vec2::new(500.0, 0.0));
    red.position = Vec2::new(200.0, 0.0);
    red.holder = Some(entity(9));
    let mut flags = [red];
    let mut timers = LooseFlagTimers {
        red_frames: 300,
        ..Default::default()
    };

    auto_return_loose_flags(&mut flags, &mut timers);

    assert_eq!(
        timers.red_frames, 0,
        "grabbing a loose flag must reset its abandonment timer"
    );
    assert_eq!(
        flags[0].position,
        Vec2::new(200.0, 0.0),
        "a recovered flag stays where its carrier holds it"
    );
}

#[test]
fn system_auto_returns_a_flag_left_loose_too_long() {
    let mut app = App::new();
    app.init_resource::<CaptureScore>();
    app.init_resource::<FlagStealScore>();
    app.init_resource::<FlagReturnScore>();
    app.init_resource::<CtfMatchResult>();
    app.init_resource::<Score>();
    app.init_resource::<OpponentScore>();
    app.init_resource::<NitroBoosts>();
    app.init_resource::<MatchClock>();
    app.insert_resource(LooseFlagTimers {
        red_frames: FLAG_RESET_FRAMES - 1,
        ..Default::default()
    });
    app.add_system(capture_the_flag_system);
    // Player far from every flag so nobody touches the loose red flag.
    app.world.spawn((
        test_player(),
        Transform::from_translation(Vec3::new(-2000.0, 0.0, 5.0)),
    ));
    app.world.spawn((
        CtfFlag {
            team: FlagTeam::Blue,
            home: Vec2::new(-1000.0, 0.0),
            holder: None,
        },
        Transform::from_translation(Vec3::new(-1000.0, 0.0, 2.0)),
    ));
    let red_home = Vec2::new(500.0, 0.0);
    let red_flag = app
        .world
        .spawn((
            CtfFlag {
                team: FlagTeam::Red,
                home: red_home,
                holder: None,
            },
            Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
        ))
        .id();

    app.update();

    let transform = app.world.get::<Transform>(red_flag).unwrap();
    assert_eq!(
        transform.translation.xy(),
        red_home,
        "a flag abandoned past the reset limit must auto-return to base"
    );
    assert_eq!(
        app.world.resource::<LooseFlagTimers>().red_frames,
        0,
        "the auto-return must clear the loose timer"
    );
}

#[test]
fn carry_timer_counts_up_a_held_flag() {
    let mut red = flag(1, FlagTeam::Red, Vec2::new(500.0, 0.0));
    red.holder = Some(entity(9));
    red.position = Vec2::new(120.0, -40.0);
    let flags = [red];
    let mut timers = FlagCarryTimers {
        red_frames: 41,
        ..Default::default()
    };

    advance_flag_carry_timers(&flags, &mut timers);

    assert_eq!(
        timers.red_frames, 42,
        "a flag in a holder's hands must keep counting up"
    );
}

#[test]
fn carry_timer_clears_an_unheld_flag() {
    // A flag sitting home (blue) and one knocked loose off its base (red) are
    // both unheld, so each clears to zero: only a flag in hand tires.
    let blue = flag(1, FlagTeam::Blue, Vec2::ZERO);
    let mut red = flag(2, FlagTeam::Red, Vec2::new(500.0, 0.0));
    red.position = Vec2::new(80.0, 80.0);
    let flags = [blue, red];
    let mut timers = FlagCarryTimers {
        blue_frames: 300,
        red_frames: 300,
    };

    advance_flag_carry_timers(&flags, &mut timers);

    assert_eq!(
        timers.blue_frames, 0,
        "a flag at home clears its carry clock"
    );
    assert_eq!(
        timers.red_frames, 0,
        "a flag knocked loose clears its carry clock"
    );
}

#[test]
fn system_counts_up_a_held_flags_carry_timer() {
    let mut app = App::new();
    app.init_resource::<CaptureScore>();
    app.init_resource::<FlagStealScore>();
    app.init_resource::<FlagReturnScore>();
    app.init_resource::<CtfMatchResult>();
    app.init_resource::<Score>();
    app.init_resource::<OpponentScore>();
    app.init_resource::<NitroBoosts>();
    app.init_resource::<MatchClock>();
    app.init_resource::<FlagCarryTimers>();
    app.add_system(capture_the_flag_system);

    // The human (blue) holds the red flag, parked far from the blue base so the
    // carry never scores: the flag stays in hand and its carry timer ticks.
    let player = app
        .world
        .spawn((
            test_player(),
            Transform::from_translation(Vec3::new(2000.0, 0.0, 5.0)),
        ))
        .id();
    app.world.spawn((
        CtfFlag {
            team: FlagTeam::Blue,
            home: Vec2::new(-1000.0, 0.0),
            holder: None,
        },
        Transform::from_translation(Vec3::new(-1000.0, 0.0, 2.0)),
    ));
    app.world.spawn((
        CtfFlag {
            team: FlagTeam::Red,
            home: Vec2::new(500.0, 0.0),
            holder: Some(player),
        },
        Transform::from_translation(Vec3::new(2000.0, 0.0, 2.0)),
    ));

    app.update();

    assert_eq!(
        app.world.resource::<FlagCarryTimers>().red_frames,
        1,
        "a held flag's carry timer must count up each frame"
    );
    assert_eq!(
        app.world.resource::<FlagCarryTimers>().blue_frames,
        0,
        "the home blue flag is unheld, so its carry timer stays at zero"
    );
}

fn app_with_clock(frames_remaining: u32) -> App {
    app_with_phased_clock(frames_remaining, MatchPhase::Regulation)
}

fn app_with_phased_clock(frames_remaining: u32, phase: MatchPhase) -> App {
    let mut app = App::new();
    app.init_resource::<CaptureScore>();
    app.init_resource::<FlagStealScore>();
    app.init_resource::<FlagReturnScore>();
    app.init_resource::<Score>();
    app.init_resource::<OpponentScore>();
    app.init_resource::<CtfMatchResult>();
    app.insert_resource(MatchClock {
        frames_remaining,
        phase,
    });
    app.add_system(expire_match_on_time_limit);
    app
}

#[test]
fn expiring_clock_ends_match_for_the_capture_leader() {
    let mut app = app_with_clock(1);
    app.insert_resource(CaptureScore {
        player: 2,
        opponents: 0,
    });

    app.update();

    assert!(app.world.resource::<MatchClock>().is_expired());
    assert_eq!(
        app.world.resource::<CtfMatchResult>().winner,
        Some(CtfMatchWinner::Player)
    );
}

#[test]
fn expiring_regulation_clock_with_level_scores_enters_sudden_death() {
    let mut app = app_with_clock(1);

    app.update();

    let clock = *app.world.resource::<MatchClock>();
    assert!(clock.is_sudden_death(), "a level round must go to overtime");
    assert_eq!(clock.frames_remaining, SUDDEN_DEATH_TIME_LIMIT_FRAMES);
    assert_eq!(
        app.world.resource::<CtfMatchResult>().winner,
        None,
        "overtime must not resolve immediately"
    );
}

#[test]
fn sudden_death_does_not_re_enter_itself() {
    let mut app = app_with_phased_clock(1, MatchPhase::SuddenDeath);

    app.update();

    let clock = *app.world.resource::<MatchClock>();
    assert!(clock.is_expired(), "overtime must run down, not refill");
    assert_eq!(
        app.world.resource::<CtfMatchResult>().winner,
        Some(CtfMatchWinner::Draw),
        "an overtime level on objectives, damage, and cash is the final fallback to a draw"
    );
}

#[test]
fn level_sudden_death_is_decided_by_the_heavier_wrecker() {
    let mut app = app_with_phased_clock(1, MatchPhase::SuddenDeath);
    app.insert_resource(Score {
        wrecks: 3,
        ..Score::default()
    });
    app.insert_resource(OpponentScore {
        wrecks: 1,
        ..OpponentScore::default()
    });

    app.update();

    assert_eq!(
        app.world.resource::<CtfMatchResult>().winner,
        Some(CtfMatchWinner::Player),
        "a deadlocked overtime goes to the team that wrecked more enemies"
    );
}

#[test]
fn level_sudden_death_is_decided_by_the_richer_team() {
    let mut app = app_with_phased_clock(1, MatchPhase::SuddenDeath);
    // Objectives and wrecks dead even, so only the banked cash can break it.
    app.insert_resource(Score {
        wrecks: 2,
        cash: 900,
        ..Score::default()
    });
    app.insert_resource(OpponentScore {
        wrecks: 2,
        cash: 350,
        ..OpponentScore::default()
    });

    app.update();

    assert_eq!(
        app.world.resource::<CtfMatchResult>().winner,
        Some(CtfMatchWinner::Player),
        "a deadlock level on objectives and damage goes to the team that banked more cash"
    );
}

#[test]
fn wreck_lead_still_decides_overtime_regardless_of_cash() {
    let mut app = app_with_phased_clock(1, MatchPhase::SuddenDeath);
    // The opponents wrecked more, so the cash arbiter is never consulted even
    // though the player ran the richer campaign.
    app.insert_resource(Score {
        wrecks: 1,
        cash: 5_000,
        ..Score::default()
    });
    app.insert_resource(OpponentScore {
        wrecks: 4,
        cash: 100,
        ..OpponentScore::default()
    });

    app.update();

    assert_eq!(
        app.world.resource::<CtfMatchResult>().winner,
        Some(CtfMatchWinner::Opponents),
        "the cash tie-break must never override a genuine wreck lead"
    );
}

#[test]
fn objective_lead_still_wins_overtime_regardless_of_wrecks() {
    let mut app = app_with_phased_clock(1, MatchPhase::SuddenDeath);
    app.insert_resource(CaptureScore {
        player: 1,
        opponents: 0,
    });
    app.insert_resource(Score {
        wrecks: 0,
        ..Score::default()
    });
    app.insert_resource(OpponentScore {
        wrecks: 9,
        ..OpponentScore::default()
    });

    app.update();

    assert_eq!(
        app.world.resource::<CtfMatchResult>().winner,
        Some(CtfMatchWinner::Player),
        "the wreck tie-break must never override a genuine objective lead"
    );
}

#[test]
fn expiring_sudden_death_clock_resolves_on_running_tallies() {
    let mut app = app_with_phased_clock(1, MatchPhase::SuddenDeath);
    app.insert_resource(FlagStealScore {
        player: 0,
        opponents: 1,
    });

    app.update();

    assert_eq!(
        app.world.resource::<CtfMatchResult>().winner,
        Some(CtfMatchWinner::Opponents),
        "a steal earned in overtime breaks the tie"
    );
}

#[test]
fn running_clock_keeps_the_match_open() {
    let mut app = app_with_clock(5);

    app.update();

    assert_eq!(app.world.resource::<MatchClock>().frames_remaining, 4);
    assert_eq!(app.world.resource::<CtfMatchResult>().winner, None);
}

#[test]
fn expired_clock_never_overrides_a_decided_winner() {
    let mut app = app_with_clock(1);
    app.insert_resource(CtfMatchResult {
        winner: Some(CtfMatchWinner::Player),
    });
    app.insert_resource(CaptureScore {
        player: 0,
        opponents: 3,
    });

    app.update();

    assert_eq!(
        app.world.resource::<CtfMatchResult>().winner,
        Some(CtfMatchWinner::Player),
        "a clinched win must not be rewritten by the timer"
    );
    assert_eq!(
        app.world.resource::<MatchClock>().frames_remaining,
        1,
        "a finished match must not keep burning clock"
    );
}

fn blue_collector(position: Vec2) -> CollectorState {
    CollectorState {
        entity: entity(1),
        team: FlagTeam::Blue,
        kind: CollectorKind::Player,
        position,
    }
}

fn red_collector(position: Vec2) -> CollectorState {
    CollectorState {
        entity: entity(2),
        team: FlagTeam::Red,
        kind: CollectorKind::Opponent,
        position,
    }
}

fn blue_teammate(position: Vec2) -> CollectorState {
    CollectorState {
        entity: entity(3),
        team: FlagTeam::Blue,
        kind: CollectorKind::Player,
        position,
    }
}

fn flag(entity_id: u32, team: FlagTeam, home: Vec2) -> FlagState {
    FlagState {
        entity: entity(entity_id),
        team,
        home,
        position: home,
        holder: None,
    }
}

fn advance_flags(flags: &mut [FlagState], collectors: &[CollectorState], score: &mut CaptureScore) {
    let mut result = CtfMatchResult::default();
    let mut steals = FlagStealScore::default();
    let mut returns = FlagReturnScore::default();
    advance_capture_the_flag(
        flags,
        collectors,
        score,
        &mut steals,
        &mut returns,
        &mut result,
    );
}

fn test_player() -> Player {
    Player {
        movement_speed: 0.0,
        rotation_speed: 0.0,
        engine_max_speed_multiplier: 0.0,
        forward_max_speed_base: 0.0,
        backward_max_speed_base: 0.0,
        wheels_turning_multiplier: 0.0,
    }
}

#[test]
fn recognises_flag_sprite_paths() {
    assert_eq!(
        flag_team_from_asset_path("arenas/church_ctf_1/blue-flag.png"),
        Some(FlagTeam::Blue)
    );
    assert_eq!(
        flag_team_from_asset_path("arenas/church_ctf_1/red-flag.png"),
        Some(FlagTeam::Red)
    );
    assert_eq!(
        flag_team_from_asset_path("arenas/church_ctf_1/hedge.png"),
        None
    );
}

#[test]
fn player_picks_up_red_flag_when_touching_it() {
    let mut flags = vec![
        flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0)),
        flag(11, FlagTeam::Red, Vec2::new(50.0, 0.0)),
    ];
    let collector = blue_collector(Vec2::ZERO);
    let mut score = CaptureScore::default();

    advance_flags(&mut flags, &[collector], &mut score);

    assert_eq!(flags[1].holder, Some(collector.entity));
    assert_eq!(flags[1].position, collector.position);
    assert_eq!(score, CaptureScore::default());
}

#[test]
fn player_steals_red_flag_for_steal_score() {
    let mut flags = vec![
        flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0)),
        flag(11, FlagTeam::Red, Vec2::new(50.0, 0.0)),
    ];
    let collector = blue_collector(Vec2::ZERO);
    let mut score = CaptureScore::default();
    let mut steals = FlagStealScore::default();
    let mut returns = FlagReturnScore::default();
    let mut result = CtfMatchResult::default();

    advance_capture_the_flag(
        &mut flags,
        &[collector],
        &mut score,
        &mut steals,
        &mut returns,
        &mut result,
    );

    assert_eq!(
        steals,
        FlagStealScore {
            player: 1,
            opponents: 0,
        }
    );
    assert_eq!(score, CaptureScore::default());
}

#[test]
fn nearest_teammate_claims_contested_enemy_flag() {
    let mut flags = vec![
        flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0)),
        flag(11, FlagTeam::Red, Vec2::ZERO),
    ];
    let far_collector = blue_collector(Vec2::new(90.0, 0.0));
    let near_collector = blue_teammate(Vec2::new(10.0, 0.0));
    let mut score = CaptureScore::default();

    advance_flags(&mut flags, &[far_collector, near_collector], &mut score);

    assert_eq!(flags[1].holder, Some(near_collector.entity));
    assert_eq!(flags[1].position, near_collector.position);
    assert_eq!(score, CaptureScore::default());
}

#[test]
fn player_scores_by_returning_enemy_flag_to_home_base() {
    let mut flags = vec![
        flag(10, FlagTeam::Blue, Vec2::ZERO),
        FlagState {
            holder: Some(entity(1)),
            ..flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0))
        },
    ];
    let collector = blue_collector(Vec2::new(10.0, 0.0));
    let mut score = CaptureScore::default();

    advance_flags(&mut flags, &[collector], &mut score);

    assert_eq!(score.player, 1);
    assert_eq!(score.opponents, 0);
    assert_eq!(flags[1].holder, None);
    assert_eq!(flags[1].position, flags[1].home);
}

#[test]
fn player_capture_at_limit_wins_the_match() {
    let mut flags = vec![
        flag(10, FlagTeam::Blue, Vec2::ZERO),
        FlagState {
            holder: Some(entity(1)),
            ..flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0))
        },
    ];
    let collector = blue_collector(Vec2::new(10.0, 0.0));
    let mut score = CaptureScore {
        player: CAPTURES_TO_WIN - 1,
        opponents: 0,
    };
    let mut steals = FlagStealScore::default();
    let mut returns = FlagReturnScore::default();
    let mut result = CtfMatchResult::default();

    advance_capture_the_flag(
        &mut flags,
        &[collector],
        &mut score,
        &mut steals,
        &mut returns,
        &mut result,
    );

    assert_eq!(score.player, CAPTURES_TO_WIN);
    assert_eq!(result.winner, Some(CtfMatchWinner::Player));
}

#[test]
fn finished_match_ignores_later_captures() {
    let mut flags = vec![
        flag(10, FlagTeam::Blue, Vec2::ZERO),
        FlagState {
            holder: Some(entity(1)),
            ..flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0))
        },
    ];
    let collector = blue_collector(Vec2::new(10.0, 0.0));
    let mut score = CaptureScore {
        player: CAPTURES_TO_WIN,
        opponents: 0,
    };
    let mut steals = FlagStealScore::default();
    let mut returns = FlagReturnScore::default();
    let mut result = CtfMatchResult {
        winner: Some(CtfMatchWinner::Player),
    };

    advance_capture_the_flag(
        &mut flags,
        &[collector],
        &mut score,
        &mut steals,
        &mut returns,
        &mut result,
    );

    assert_eq!(score.player, CAPTURES_TO_WIN);
    assert_eq!(result.winner, Some(CtfMatchWinner::Player));
}

#[test]
fn winning_capture_ends_same_frame_flag_interactions() {
    let mut flags = vec![
        flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0)),
        FlagState {
            holder: Some(entity(1)),
            position: Vec2::new(-500.0, 0.0),
            ..flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0))
        },
    ];
    let blue = blue_collector(Vec2::new(-500.0, 0.0));
    let teammate = blue_teammate(Vec2::new(500.0, 0.0));
    let mut score = CaptureScore {
        player: CAPTURES_TO_WIN - 1,
        opponents: CAPTURES_TO_WIN - 1,
    };
    let mut steals = FlagStealScore::default();
    let mut returns = FlagReturnScore::default();
    let mut result = CtfMatchResult::default();

    advance_capture_the_flag(
        &mut flags,
        &[blue, teammate],
        &mut score,
        &mut steals,
        &mut returns,
        &mut result,
    );

    assert_eq!(
        score,
        CaptureScore {
            player: CAPTURES_TO_WIN,
            opponents: CAPTURES_TO_WIN - 1,
        }
    );
    assert_eq!(result.winner, Some(CtfMatchWinner::Player));
    assert_eq!(flags[1].holder, None);
    assert_eq!(flags[1].position, flags[1].home);
}

#[test]
fn missing_holder_drops_flag_at_last_position() {
    let dropped_position = Vec2::new(125.0, 50.0);
    let mut flags = vec![FlagState {
        holder: Some(entity(99)),
        position: dropped_position,
        ..flag(10, FlagTeam::Blue, Vec2::ZERO)
    }];
    let mut score = CaptureScore::default();

    advance_flags(&mut flags, &[], &mut score);

    assert_eq!(flags[0].holder, None);
    assert_eq!(flags[0].position, dropped_position);
    assert_eq!(score, CaptureScore::default());
}

#[test]
fn teammate_returns_dropped_home_flag_same_frame() {
    let mut flags = vec![FlagState {
        holder: Some(entity(99)),
        position: Vec2::new(40.0, 0.0),
        ..flag(10, FlagTeam::Blue, Vec2::ZERO)
    }];
    let collector = blue_collector(Vec2::new(45.0, 0.0));
    let mut score = CaptureScore::default();

    advance_flags(&mut flags, &[collector], &mut score);

    assert_eq!(flags[0].holder, None);
    assert_eq!(flags[0].position, flags[0].home);
    assert_eq!(score, CaptureScore::default());
}

#[test]
fn opponent_scores_by_returning_blue_flag_to_red_base() {
    let mut flags = vec![
        FlagState {
            holder: Some(entity(2)),
            ..flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0))
        },
        flag(11, FlagTeam::Red, Vec2::ZERO),
    ];
    let collector = red_collector(Vec2::new(0.0, -10.0));
    let mut score = CaptureScore::default();

    advance_flags(&mut flags, &[collector], &mut score);

    assert_eq!(score.player, 0);
    assert_eq!(score.opponents, 1);
    assert_eq!(flags[0].holder, None);
    assert_eq!(flags[0].position, flags[0].home);
}

#[test]
fn cannot_score_while_own_flag_is_stolen() {
    let mut flags = vec![
        FlagState {
            holder: Some(entity(2)),
            position: Vec2::new(200.0, 0.0),
            ..flag(10, FlagTeam::Blue, Vec2::ZERO)
        },
        FlagState {
            holder: Some(entity(1)),
            ..flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0))
        },
    ];
    let collector = blue_collector(Vec2::ZERO);
    let mut score = CaptureScore::default();

    advance_flags(&mut flags, &[collector], &mut score);

    assert_eq!(score, CaptureScore::default());
    assert_eq!(flags[1].holder, Some(collector.entity));
}

#[test]
fn cannot_score_while_home_base_is_contested() {
    let mut flags = vec![
        flag(10, FlagTeam::Blue, Vec2::ZERO),
        FlagState {
            holder: Some(entity(1)),
            ..flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0))
        },
    ];
    let blue = blue_collector(Vec2::new(10.0, 0.0));
    let red = red_collector(Vec2::new(150.0, 0.0));
    let mut score = CaptureScore::default();

    advance_flags(&mut flags, &[blue, red], &mut score);

    assert_eq!(score, CaptureScore::default());
    assert_eq!(flags[1].holder, Some(blue.entity));
    assert_eq!(flags[1].position, blue.position);
}

#[test]
fn opponent_returns_stolen_red_flag_by_tagging_player() {
    let mut flags = vec![
        flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0)),
        FlagState {
            holder: Some(entity(1)),
            position: Vec2::new(20.0, 0.0),
            ..flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0))
        },
    ];
    let collector = red_collector(Vec2::ZERO);
    let mut score = CaptureScore::default();

    advance_flags(&mut flags, &[collector], &mut score);

    assert_eq!(score, CaptureScore::default());
    assert_eq!(flags[1].holder, None);
    assert_eq!(flags[1].position, flags[1].home);
}

#[test]
fn player_returns_stolen_blue_flag_by_tagging_opponent() {
    let mut flags = vec![
        FlagState {
            holder: Some(entity(2)),
            position: Vec2::new(-20.0, 0.0),
            ..flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0))
        },
        flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0)),
    ];
    let collector = blue_collector(Vec2::ZERO);
    let mut score = CaptureScore::default();

    advance_flags(&mut flags, &[collector], &mut score);

    assert_eq!(score, CaptureScore::default());
    assert_eq!(flags[0].holder, None);
    assert_eq!(flags[0].position, flags[0].home);
}

#[test]
fn player_returns_dropped_blue_flag_by_touching_it() {
    let mut flags = vec![
        FlagState {
            position: Vec2::new(-20.0, 0.0),
            ..flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0))
        },
        flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0)),
    ];
    let collector = blue_collector(Vec2::ZERO);
    let mut score = CaptureScore::default();

    advance_flags(&mut flags, &[collector], &mut score);

    assert_eq!(score, CaptureScore::default());
    assert_eq!(flags[0].holder, None);
    assert_eq!(flags[0].position, flags[0].home);
}

#[test]
fn player_returns_stolen_blue_flag_using_current_carrier_position() {
    let mut flags = vec![
        FlagState {
            holder: Some(entity(2)),
            position: Vec2::new(800.0, 0.0),
            ..flag(10, FlagTeam::Blue, Vec2::new(-500.0, 0.0))
        },
        flag(11, FlagTeam::Red, Vec2::new(500.0, 0.0)),
    ];
    let blue = blue_collector(Vec2::ZERO);
    let red = red_collector(Vec2::new(20.0, 0.0));
    let mut score = CaptureScore::default();

    advance_flags(&mut flags, &[blue, red], &mut score);

    assert_eq!(score, CaptureScore::default());
    assert_eq!(flags[0].holder, None);
    assert_eq!(flags[0].position, flags[0].home);
}

#[test]
fn system_tracks_player_capture_without_query_conflicts() {
    let mut app = App::new();
    app.init_resource::<CaptureScore>();
    app.init_resource::<FlagStealScore>();
    app.init_resource::<FlagReturnScore>();
    app.init_resource::<CtfMatchResult>();
    app.init_resource::<Score>();
    app.init_resource::<OpponentScore>();
    app.init_resource::<NitroBoosts>();
    app.init_resource::<MatchClock>();
    app.add_system(capture_the_flag_system);
    let player = app
        .world
        .spawn((
            test_player(),
            Transform::from_translation(Vec3::new(10.0, 0.0, 5.0)),
        ))
        .id();
    app.world.spawn((
        CtfFlag {
            team: FlagTeam::Blue,
            home: Vec2::ZERO,
            holder: None,
        },
        Transform::from_translation(Vec3::new(0.0, 0.0, 2.0)),
    ));
    app.world.spawn((
        CtfFlag {
            team: FlagTeam::Red,
            home: Vec2::new(500.0, 0.0),
            holder: Some(player),
        },
        Transform::from_translation(Vec3::new(10.0, 0.0, 2.0)),
    ));

    app.update();

    assert_eq!(
        *app.world.resource::<CaptureScore>(),
        CaptureScore {
            player: 1,
            opponents: 0,
        }
    );
    assert_eq!(app.world.resource::<Score>().cash, CAPTURE_CASH_BOUNTY);
    assert_eq!(app.world.resource::<OpponentScore>().cash, 0);
    assert_eq!(
        app.world.resource::<NitroBoosts>().player_frames,
        crate::gameplay::pickup::NITRO_BOOST_FRAMES
    );
    assert_eq!(app.world.resource::<NitroBoosts>().opponent_frames, 0);
}

#[test]
fn system_rewards_player_for_returning_home_flag() {
    let mut app = App::new();
    app.init_resource::<CaptureScore>();
    app.init_resource::<FlagStealScore>();
    app.init_resource::<FlagReturnScore>();
    app.init_resource::<CtfMatchResult>();
    app.init_resource::<Score>();
    app.init_resource::<OpponentScore>();
    app.init_resource::<NitroBoosts>();
    app.init_resource::<MatchClock>();
    app.add_system(capture_the_flag_system);
    app.world.spawn((
        test_player(),
        Transform::from_translation(Vec3::new(-20.0, 0.0, 5.0)),
    ));
    app.world.spawn((
        CtfFlag {
            team: FlagTeam::Blue,
            home: Vec2::new(-500.0, 0.0),
            holder: None,
        },
        Transform::from_translation(Vec3::new(-20.0, 0.0, 2.0)),
    ));
    app.world.spawn((
        CtfFlag {
            team: FlagTeam::Red,
            home: Vec2::new(500.0, 0.0),
            holder: None,
        },
        Transform::from_translation(Vec3::new(500.0, 0.0, 2.0)),
    ));

    app.update();

    assert_eq!(
        *app.world.resource::<FlagReturnScore>(),
        FlagReturnScore {
            player: 1,
            opponents: 0,
        }
    );
    assert_eq!(app.world.resource::<Score>().cash, FLAG_RETURN_CASH_BOUNTY);
    assert_eq!(app.world.resource::<Score>().returns, 1);
    assert_eq!(app.world.resource::<OpponentScore>().cash, 0);
    assert_eq!(
        app.world.resource::<NitroBoosts>().player_frames,
        crate::gameplay::pickup::NITRO_BOOST_FRAMES
    );
    assert_eq!(app.world.resource::<NitroBoosts>().opponent_frames, 0);
}

#[test]
fn system_uses_virtual_player_team_for_enemy_flags() {
    let mut app = App::new();
    app.init_resource::<CaptureScore>();
    app.init_resource::<FlagStealScore>();
    app.init_resource::<FlagReturnScore>();
    app.init_resource::<CtfMatchResult>();
    app.init_resource::<Score>();
    app.init_resource::<OpponentScore>();
    app.init_resource::<NitroBoosts>();
    app.init_resource::<MatchClock>();
    app.add_system(capture_the_flag_system);
    let virtual_player = app
        .world
        .spawn((
            VirtualPlayer {
                team: AiTeam::Blue,
                movement_speed: 0.0,
                rotation_speed: 0.0,
                waypoints: vec![],
                current_waypoint: 0,
                player_pursuit_radius: 0.0,
                pickup_pursuit_radius: 0.0,
                corner_throttle: 0.3,
            },
            Transform::from_translation(Vec3::ZERO),
        ))
        .id();
    let blue_flag = app
        .world
        .spawn((
            CtfFlag {
                team: FlagTeam::Blue,
                home: Vec2::new(-500.0, 0.0),
                holder: None,
            },
            Transform::from_translation(Vec3::new(-500.0, 0.0, 2.0)),
        ))
        .id();
    let red_flag = app
        .world
        .spawn((
            CtfFlag {
                team: FlagTeam::Red,
                home: Vec2::new(500.0, 0.0),
                holder: None,
            },
            Transform::from_translation(Vec3::new(10.0, 0.0, 2.0)),
        ))
        .id();

    app.update();

    assert_eq!(app.world.get::<CtfFlag>(blue_flag).unwrap().holder, None);
    assert_eq!(
        app.world.get::<CtfFlag>(red_flag).unwrap().holder,
        Some(virtual_player)
    );
    assert_eq!(
        *app.world.resource::<FlagStealScore>(),
        FlagStealScore {
            player: 1,
            opponents: 0,
        }
    );
    assert_eq!(app.world.resource::<Score>().cash, FLAG_STEAL_CASH_BOUNTY);
    assert_eq!(
        app.world.resource::<NitroBoosts>().player_frames,
        crate::gameplay::pickup::NITRO_BOOST_FRAMES
    );
    assert_eq!(app.world.resource::<NitroBoosts>().opponent_frames, 0);
}

#[test]
fn blue_virtual_player_capture_scores_for_player_team() {
    let mut app = App::new();
    app.init_resource::<CaptureScore>();
    app.init_resource::<FlagStealScore>();
    app.init_resource::<FlagReturnScore>();
    app.init_resource::<CtfMatchResult>();
    app.init_resource::<Score>();
    app.init_resource::<OpponentScore>();
    app.init_resource::<NitroBoosts>();
    app.init_resource::<MatchClock>();
    app.add_system(capture_the_flag_system);
    let virtual_player = app
        .world
        .spawn((
            VirtualPlayer {
                team: AiTeam::Blue,
                movement_speed: 0.0,
                rotation_speed: 0.0,
                waypoints: vec![],
                current_waypoint: 0,
                player_pursuit_radius: 0.0,
                pickup_pursuit_radius: 0.0,
                corner_throttle: 0.3,
            },
            Transform::from_translation(Vec3::new(-500.0, 0.0, 4.0)),
        ))
        .id();
    app.world.spawn((
        CtfFlag {
            team: FlagTeam::Blue,
            home: Vec2::new(-500.0, 0.0),
            holder: None,
        },
        Transform::from_translation(Vec3::new(-500.0, 0.0, 2.0)),
    ));
    app.world.spawn((
        CtfFlag {
            team: FlagTeam::Red,
            home: Vec2::new(500.0, 0.0),
            holder: Some(virtual_player),
        },
        Transform::from_translation(Vec3::new(-500.0, 0.0, 2.0)),
    ));

    app.update();

    assert_eq!(
        *app.world.resource::<CaptureScore>(),
        CaptureScore {
            player: 1,
            opponents: 0,
        }
    );
    assert_eq!(app.world.resource::<Score>().cash, CAPTURE_CASH_BOUNTY);
    assert_eq!(app.world.resource::<OpponentScore>().cash, 0);
    assert_eq!(
        app.world.resource::<NitroBoosts>().player_frames,
        crate::gameplay::pickup::NITRO_BOOST_FRAMES
    );
    assert_eq!(app.world.resource::<NitroBoosts>().opponent_frames, 0);
}

#[test]
fn system_records_match_winner_at_capture_limit() {
    let mut app = App::new();
    app.insert_resource(CaptureScore {
        player: CAPTURES_TO_WIN - 1,
        opponents: 0,
    });
    app.init_resource::<FlagStealScore>();
    app.init_resource::<FlagReturnScore>();
    app.init_resource::<CtfMatchResult>();
    app.init_resource::<Score>();
    app.init_resource::<OpponentScore>();
    app.init_resource::<NitroBoosts>();
    app.init_resource::<MatchClock>();
    app.add_system(capture_the_flag_system);
    let player = app
        .world
        .spawn((
            test_player(),
            Transform::from_translation(Vec3::new(10.0, 0.0, 5.0)),
        ))
        .id();
    app.world.spawn((
        CtfFlag {
            team: FlagTeam::Blue,
            home: Vec2::ZERO,
            holder: None,
        },
        Transform::from_translation(Vec3::new(0.0, 0.0, 2.0)),
    ));
    app.world.spawn((
        CtfFlag {
            team: FlagTeam::Red,
            home: Vec2::new(500.0, 0.0),
            holder: Some(player),
        },
        Transform::from_translation(Vec3::new(10.0, 0.0, 2.0)),
    ));

    app.update();

    assert_eq!(
        app.world.resource::<CtfMatchResult>().winner,
        Some(CtfMatchWinner::Player)
    );
}

#[test]
fn golden_goal_ends_overtime_on_a_player_capture() {
    let mut result = CtfMatchResult::default();

    award_golden_goal(
        true,
        CaptureScore {
            player: 1,
            opponents: 1,
        },
        CaptureScore {
            player: 2,
            opponents: 1,
        },
        &mut result,
    );

    assert_eq!(result.winner, Some(CtfMatchWinner::Player));
}

#[test]
fn golden_goal_ends_overtime_on_an_opponent_capture() {
    let mut result = CtfMatchResult::default();

    award_golden_goal(
        true,
        CaptureScore::default(),
        CaptureScore {
            player: 0,
            opponents: 1,
        },
        &mut result,
    );

    assert_eq!(result.winner, Some(CtfMatchWinner::Opponents));
}

#[test]
fn golden_goal_wins_below_the_regulation_capture_threshold() {
    // A single overtime capture decides it though it is far from
    // CAPTURES_TO_WIN, which a lone regulation capture never would be.
    let mut result = CtfMatchResult::default();

    award_golden_goal(
        true,
        CaptureScore::default(),
        CaptureScore {
            player: 1,
            opponents: 0,
        },
        &mut result,
    );

    assert_eq!(result.winner, Some(CtfMatchWinner::Player));
}

#[test]
fn golden_goal_is_inert_during_regulation() {
    let mut result = CtfMatchResult::default();

    award_golden_goal(
        false,
        CaptureScore::default(),
        CaptureScore {
            player: 1,
            opponents: 0,
        },
        &mut result,
    );

    assert_eq!(
        result.winner, None,
        "in regulation a lone capture only banks a point"
    );
}

#[test]
fn golden_goal_ignores_frames_without_a_new_capture() {
    let mut result = CtfMatchResult::default();

    award_golden_goal(
        true,
        CaptureScore {
            player: 2,
            opponents: 2,
        },
        CaptureScore {
            player: 2,
            opponents: 2,
        },
        &mut result,
    );

    assert_eq!(result.winner, None);
}

#[test]
fn golden_goal_never_overrides_a_decided_winner() {
    let mut result = CtfMatchResult {
        winner: Some(CtfMatchWinner::Opponents),
    };

    award_golden_goal(
        true,
        CaptureScore::default(),
        CaptureScore {
            player: 1,
            opponents: 0,
        },
        &mut result,
    );

    assert_eq!(result.winner, Some(CtfMatchWinner::Opponents));
}

#[test]
fn system_awards_golden_goal_capture_in_sudden_death() {
    let mut app = App::new();
    app.insert_resource(CaptureScore {
        player: 1,
        opponents: 1,
    });
    app.init_resource::<FlagStealScore>();
    app.init_resource::<FlagReturnScore>();
    app.init_resource::<CtfMatchResult>();
    app.init_resource::<Score>();
    app.init_resource::<OpponentScore>();
    app.init_resource::<NitroBoosts>();
    app.insert_resource(MatchClock {
        frames_remaining: SUDDEN_DEATH_TIME_LIMIT_FRAMES,
        phase: MatchPhase::SuddenDeath,
    });
    app.add_system(capture_the_flag_system);
    let player = app
        .world
        .spawn((
            test_player(),
            Transform::from_translation(Vec3::new(10.0, 0.0, 5.0)),
        ))
        .id();
    app.world.spawn((
        CtfFlag {
            team: FlagTeam::Blue,
            home: Vec2::ZERO,
            holder: None,
        },
        Transform::from_translation(Vec3::new(0.0, 0.0, 2.0)),
    ));
    app.world.spawn((
        CtfFlag {
            team: FlagTeam::Red,
            home: Vec2::new(500.0, 0.0),
            holder: Some(player),
        },
        Transform::from_translation(Vec3::new(10.0, 0.0, 2.0)),
    ));

    app.update();

    assert_eq!(
        app.world.resource::<CaptureScore>().player,
        2,
        "the overtime capture still tallies"
    );
    assert!(
        app.world.resource::<CaptureScore>().player < CAPTURES_TO_WIN,
        "the golden goal wins short of a regulation clinch"
    );
    assert_eq!(
        app.world.resource::<CtfMatchResult>().winner,
        Some(CtfMatchWinner::Player),
        "the first overtime capture wins outright"
    );
}

fn purse_app() -> App {
    let mut app = App::new();
    app.init_resource::<Score>();
    app.init_resource::<OpponentScore>();
    app.init_resource::<CaptureScore>();
    app.init_resource::<FlagStealScore>();
    app.init_resource::<FlagReturnScore>();
    app.init_resource::<CtfMatchResult>();
    app.init_resource::<MatchPursePaid>();
    app.init_resource::<MatchClock>();
    app.add_system(award_match_purse_on_resolution);
    app
}

#[test]
fn unresolved_match_banks_no_purse() {
    let mut app = purse_app();

    app.update();

    assert_eq!(app.world.resource::<Score>().cash, 0);
    assert_eq!(app.world.resource::<OpponentScore>().cash, 0);
    assert!(!app.world.resource::<MatchPursePaid>().0);
}

#[test]
fn resolved_match_banks_the_purse_and_latches_it() {
    let mut app = purse_app();
    // Winning by two captures keeps this a bare-purse win: the loser sat
    // clear of zero (no clean sheet) and clear of match point (no
    // nail-biter).
    app.insert_resource(CaptureScore {
        player: 3,
        opponents: 1,
    });
    app.insert_resource(CtfMatchResult {
        winner: Some(CtfMatchWinner::Player),
    });

    app.update();

    assert_eq!(app.world.resource::<Score>().cash, VICTORY_CASH_PURSE);
    assert!(
        app.world.resource::<MatchPursePaid>().0,
        "banking the purse must latch the flag"
    );
}

#[test]
fn a_resolved_match_pays_the_purse_only_once() {
    let mut app = purse_app();
    // Winning by two captures keeps this a bare-purse win: the loser sat
    // clear of zero (no clean sheet) and clear of match point (no
    // nail-biter).
    app.insert_resource(CaptureScore {
        player: 1,
        opponents: 3,
    });
    app.insert_resource(CtfMatchResult {
        winner: Some(CtfMatchWinner::Opponents),
    });

    app.update();
    app.update();
    app.update();

    assert_eq!(
        app.world.resource::<OpponentScore>().cash,
        VICTORY_CASH_PURSE,
        "the frozen post-match frames must not keep re-banking the purse"
    );
}

#[test]
fn a_resolved_clean_sheet_banks_the_bonus_through_the_system() {
    let mut app = purse_app();
    // The beaten opponents never captured: the resolution system must bank
    // the clean-sheet bonus on top of the victory purse.
    app.insert_resource(CaptureScore {
        player: 3,
        opponents: 0,
    });
    app.insert_resource(CtfMatchResult {
        winner: Some(CtfMatchWinner::Player),
    });

    app.update();

    assert_eq!(
        app.world.resource::<Score>().cash,
        VICTORY_CASH_PURSE + CLEAN_SHEET_CASH_BONUS
    );
}

#[test]
fn a_resolved_nail_biter_banks_the_bonus_through_the_system() {
    let mut app = purse_app();
    // The beaten opponents finished on match point: the resolution system
    // must bank the nail-biter bonus on top of the victory purse.
    app.insert_resource(CaptureScore {
        player: CAPTURES_TO_WIN,
        opponents: CAPTURES_TO_WIN - 1,
    });
    app.insert_resource(CtfMatchResult {
        winner: Some(CtfMatchWinner::Player),
    });

    app.update();

    assert_eq!(
        app.world.resource::<Score>().cash,
        VICTORY_CASH_PURSE + NAIL_BITER_CASH_BONUS
    );
}

#[test]
fn an_overtime_capture_banks_the_golden_goal_bonus_through_the_system() {
    let mut app = purse_app();
    // Won 2-1 with the clock still running in sudden death: a golden goal, so
    // the resolution system reads the overtime clock and banks the bonus.
    app.insert_resource(CaptureScore {
        player: 2,
        opponents: 1,
    });
    app.insert_resource(CtfMatchResult {
        winner: Some(CtfMatchWinner::Player),
    });
    app.insert_resource(MatchClock {
        frames_remaining: SUDDEN_DEATH_TIME_LIMIT_FRAMES,
        phase: MatchPhase::SuddenDeath,
    });

    app.update();

    assert_eq!(
        app.world.resource::<Score>().cash,
        VICTORY_CASH_PURSE + GOLDEN_GOAL_CASH_BONUS,
        "a capture that clinches a live overtime must bank the golden-goal bonus"
    );
}

#[test]
fn an_overtime_resolved_on_the_clock_banks_no_golden_goal_bonus() {
    let mut app = purse_app();
    // Sudden death with the clock already expired: the timeout path decided
    // the leader, so it is no golden goal and only the bare purse is banked.
    app.insert_resource(CaptureScore {
        player: 2,
        opponents: 1,
    });
    app.insert_resource(CtfMatchResult {
        winner: Some(CtfMatchWinner::Player),
    });
    app.insert_resource(MatchClock {
        frames_remaining: 0,
        phase: MatchPhase::SuddenDeath,
    });

    app.update();

    assert_eq!(
        app.world.resource::<Score>().cash,
        VICTORY_CASH_PURSE,
        "an overtime decided by the clock running out is no golden goal"
    );
}

#[test]
fn an_overtime_wreck_tiebreak_banks_the_demolition_bonus_through_the_system() {
    let mut app = purse_app();
    // A 1-1 objective deadlock that ran the overtime clock down and was settled by
    // the wreck tiebreak: the resolution system must read the expired overtime
    // clock and the level objectives and bank the demolition-decider bonus. The
    // loser sat clear of zero (no clean sheet) and of match point (no nail-biter),
    // isolating the demolition bonus.
    app.insert_resource(CaptureScore {
        player: 1,
        opponents: 1,
    });
    app.insert_resource(Score {
        wrecks: 4,
        ..Score::default()
    });
    app.insert_resource(CtfMatchResult {
        winner: Some(CtfMatchWinner::Player),
    });
    app.insert_resource(MatchClock {
        frames_remaining: 0,
        phase: MatchPhase::SuddenDeath,
    });

    app.update();

    assert_eq!(
        app.world.resource::<Score>().cash,
        VICTORY_CASH_PURSE + DEMOLITION_DECIDER_CASH_BONUS,
        "an overtime deadlock settled by the wreck tiebreak must bank the demolition bonus"
    );
}

#[test]
fn an_overtime_cash_tiebreak_banks_the_treasury_bonus_through_the_system() {
    let mut app = purse_app();
    // A 1-1 objective deadlock level on wrecks too falls through to the cash decider:
    // the player banked one more coin, so the resolution system reads the expired
    // overtime clock, the level objectives and wrecks, and banks the treasury-decider
    // bonus (never the demolition bonus, which the level wrecks rule out).
    app.insert_resource(CaptureScore {
        player: 1,
        opponents: 1,
    });
    app.insert_resource(Score {
        cash: 1,
        ..Score::default()
    });
    app.insert_resource(CtfMatchResult {
        winner: Some(CtfMatchWinner::Player),
    });
    app.insert_resource(MatchClock {
        frames_remaining: 0,
        phase: MatchPhase::SuddenDeath,
    });

    app.update();

    assert_eq!(
        app.world.resource::<Score>().cash,
        1 + VICTORY_CASH_PURSE + TREASURY_DECIDER_CASH_BONUS,
        "an overtime deadlock settled by the cash tiebreak must bank the treasury bonus"
    );
}
