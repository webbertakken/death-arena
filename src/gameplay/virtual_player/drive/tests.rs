use super::*;
use crate::gameplay::combat::MAX_INTEGRITY;
use crate::gameplay::ctf::{CtfFlag, CtfMatchWinner, FlagTeam};
use crate::gameplay::virtual_player::VirtualPlayer;

fn app_with_system() -> App {
    let mut app = App::new();
    app.add_system(virtual_player_drive_system);
    app
}

fn spawn_player(app: &mut App, position: Vec3) -> Entity {
    app.world
        .spawn((
            Player {
                movement_speed: 0.0,
                rotation_speed: 0.0,
                engine_max_speed_multiplier: 0.0,
                forward_max_speed_base: 0.0,
                backward_max_speed_base: 0.0,
                wheels_turning_multiplier: 0.0,
            },
            Transform::from_translation(position),
        ))
        .id()
}

fn spawn_ai(app: &mut App, waypoints: Vec<Vec2>) -> Entity {
    spawn_ai_on_team(app, AiTeam::Red, waypoints)
}

/// Baseline pursuit radius for test fixtures: matches the all-rounder driving
/// personality, the neutral feel every behavioural assertion is measured
/// against.
const TEST_PURSUIT_RADIUS: f32 = 500.0;

/// Baseline pickup-scavenging radius for test fixtures: matches the all-rounder
/// driving personality and the former uniform global, the neutral greed every
/// pickup-behaviour assertion is measured against.
const TEST_PICKUP_PURSUIT_RADIUS: f32 = 450.0;

fn spawn_ai_on_team(app: &mut App, team: AiTeam, waypoints: Vec<Vec2>) -> Entity {
    spawn_ai_with_pursuit(app, team, waypoints, TEST_PURSUIT_RADIUS)
}

fn spawn_ai_with_pursuit(
    app: &mut App,
    team: AiTeam,
    waypoints: Vec<Vec2>,
    player_pursuit_radius: f32,
) -> Entity {
    app.world
        .spawn((
            VirtualPlayer {
                team,
                movement_speed: 500.0,
                rotation_speed: f32::to_radians(360.0),
                waypoints,
                current_waypoint: 0,
                player_pursuit_radius,
                pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                corner_throttle: 0.3,
            },
            Transform::from_translation(Vec3::new(0.0, 0.0, 4.0)),
        ))
        .id()
}

/// Spawns a Red driver with a bespoke pickup-scavenging radius (and the
/// baseline player-pursuit reach), so a test can pit a greedy personality
/// against a disciplined one on the same pickup.
fn spawn_ai_with_pickup_pursuit(
    app: &mut App,
    waypoints: Vec<Vec2>,
    pickup_pursuit_radius: f32,
) -> Entity {
    app.world
        .spawn((
            VirtualPlayer {
                team: AiTeam::Red,
                movement_speed: 500.0,
                rotation_speed: f32::to_radians(360.0),
                waypoints,
                current_waypoint: 0,
                player_pursuit_radius: TEST_PURSUIT_RADIUS,
                pickup_pursuit_radius,
                corner_throttle: 0.3,
            },
            Transform::from_translation(Vec3::new(0.0, 0.0, 4.0)),
        ))
        .id()
}

fn spawn_ai_at(app: &mut App, waypoints: Vec<Vec2>, translation: Vec3) -> Entity {
    app.world
        .spawn((
            VirtualPlayer {
                team: AiTeam::Red,
                movement_speed: 500.0,
                rotation_speed: f32::to_radians(360.0),
                waypoints,
                current_waypoint: 0,
                player_pursuit_radius: TEST_PURSUIT_RADIUS,
                pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                corner_throttle: 0.3,
            },
            Transform::from_translation(translation),
        ))
        .id()
}

fn spawn_flag(app: &mut App, team: FlagTeam, home: Vec2, position: Vec3, holder: Option<Entity>) {
    app.world.spawn((
        CtfFlag { team, home, holder },
        Transform::from_translation(position),
    ));
}

fn one_frame_ai_y(team: AiTeam, nitro: Option<fn(&mut NitroBoosts)>) -> f32 {
    let mut app = app_with_system();
    if let Some(trigger) = nitro {
        app.init_resource::<NitroBoosts>();
        trigger(&mut app.world.resource_mut::<NitroBoosts>());
    }
    let ai = spawn_ai_on_team(&mut app, team, vec![Vec2::new(0.0, 1000.0)]);

    app.update();

    app.world.get::<Transform>(ai).unwrap().translation.y
}

#[test]
fn a_committing_team_skips_a_cash_detour_to_race_the_flag() {
    use crate::gameplay::ctf::{MatchPhase, MATCH_TIME_LIMIT_FRAMES};

    // A red attacker at the origin facing +Y, the blue (enemy) flag dead
    // ahead and sitting at home so the car is assigned to go steal it. A cash
    // bag sits just ahead and to the right, inside the flag lane: a free grab
    // in normal play that a clock-racing team should leave on the track.
    fn attacker_x_after_one_frame(closing: bool) -> f32 {
        let mut app = app_with_system();
        // Red (opponents) trails, so in closing time it commits to the push.
        app.insert_resource(CaptureScore {
            player: 1,
            opponents: 0,
        });
        app.insert_resource(MatchClock {
            frames_remaining: if closing { 10 } else { MATCH_TIME_LIMIT_FRAMES },
            phase: MatchPhase::Regulation,
        });

        let attacker = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 2000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(0.0, 800.0),
            Vec3::new(0.0, 800.0, 0.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(0.0, -1000.0),
            Vec3::new(0.0, -1000.0, 0.0),
            None,
        );
        app.world.spawn((
            Pickup {
                kind: PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(40.0, 80.0, 2.0)),
        ));

        app.update();

        app.world.get::<Transform>(attacker).unwrap().translation.x
    }

    let detoured = attacker_x_after_one_frame(false);
    let committed = attacker_x_after_one_frame(true);

    assert!(
        detoured > 0.1,
        "normal play veers right toward the cash bag: {detoured}"
    );
    assert!(
        committed.abs() < 1e-3,
        "closing-time commitment drives straight at the flag, ignoring the cash: {committed}"
    );
}

#[test]
fn a_leading_team_also_skips_a_cash_detour_in_closing_time() {
    use crate::gameplay::ctf::{MatchPhase, MATCH_TIME_LIMIT_FRAMES};

    // The mirror of the committing-team detour test for the side that is
    // ahead. A lone red attacker, *leading* on captures, is assigned to steal
    // the blue flag dead ahead, with a cash bag just inside the flag lane. A
    // trailing team already leaves that bag (it commits); a leader running
    // down the clock should too, rather than greedily farming cash on a lead
    // it is about to win on. The lone car is never recalled to defend (the
    // lead-defence guard never pulls a team's last car), so the only thing
    // that can hold it off the bag is the broadened closing-time discipline.
    fn attacker_x_after_one_frame(closing: bool) -> f32 {
        let mut app = app_with_system();
        // Red (opponents) leads, so it never "commits"; only the discipline
        // that now also covers a protecting leader can leave the cash bag.
        app.insert_resource(CaptureScore {
            player: 0,
            opponents: 1,
        });
        app.insert_resource(MatchClock {
            frames_remaining: if closing { 10 } else { MATCH_TIME_LIMIT_FRAMES },
            phase: MatchPhase::Regulation,
        });

        let attacker = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 2000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(0.0, 800.0),
            Vec3::new(0.0, 800.0, 0.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(0.0, -1000.0),
            Vec3::new(0.0, -1000.0, 0.0),
            None,
        );
        app.world.spawn((
            Pickup {
                kind: PickupKind::Cash,
            },
            Transform::from_translation(Vec3::new(40.0, 80.0, 2.0)),
        ));

        app.update();

        app.world.get::<Transform>(attacker).unwrap().translation.x
    }

    let detoured = attacker_x_after_one_frame(false);
    let disciplined = attacker_x_after_one_frame(true);

    assert!(
        detoured > 0.1,
        "outside closing time even a leader veers right for the free cash bag: {detoured}"
    );
    assert!(
        disciplined.abs() < 1e-3,
        "in closing time a leader leaves the cash and races the flag too: {disciplined}"
    );
}

#[test]
fn a_leading_team_recalls_its_home_most_car_to_defend_in_closing_time() {
    use crate::gameplay::ctf::{MatchPhase, MATCH_TIME_LIMIT_FRAMES};

    // Red home sits at the origin and the blue (enemy) flag and base straight
    // ahead at +Y. A free red car drives forward to attack the enemy flag; a
    // car recalled to guard the lead instead heads back down its home lane
    // (toward the guard point at +Y 220), so a car sitting forward of that
    // point reverses. The home-most red car starts at (0, 500), forward of the
    // guard point but short of the flag, so the two intents pull opposite ways.
    fn home_most_dy(protecting: bool) -> f32 {
        let mut app = app_with_system();
        // Red (opponents) leads, so in closing time it protects that lead.
        app.insert_resource(CaptureScore {
            player: 0,
            opponents: 1,
        });
        app.insert_resource(MatchClock {
            frames_remaining: if protecting {
                10
            } else {
                MATCH_TIME_LIMIT_FRAMES
            },
            phase: MatchPhase::Regulation,
        });

        let home_most = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 2000.0)],
            Vec3::new(0.0, 500.0, 4.0),
        );
        // A second, more-forward red car keeps the team above the lone-car
        // guard and is never the home-most pick.
        spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 2000.0)],
            Vec3::new(0.0, 1500.0, 4.0),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::ZERO,
            Vec3::new(0.0, 0.0, 4.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(0.0, 1000.0),
            Vec3::new(0.0, 1000.0, 4.0),
            None,
        );

        app.update();

        app.world.get::<Transform>(home_most).unwrap().translation.y - 500.0
    }

    let attacking = home_most_dy(false);
    let defending = home_most_dy(true);

    assert!(
        attacking > 0.1,
        "outside closing time the leader's car pushes forward to attack: {attacking}"
    );
    assert!(
        defending < -0.1,
        "in closing time the leader recalls its home-most car to guard the lead: {defending}"
    );
}

#[test]
fn a_trailing_team_is_not_recalled_to_defend_in_closing_time() {
    use crate::gameplay::ctf::MatchPhase;

    // Red trails on captures, so in closing time it commits to attack rather
    // than protecting a lead it does not hold: its home-most car pushes on.
    let mut app = app_with_system();
    app.insert_resource(CaptureScore {
        player: 1,
        opponents: 0,
    });
    app.insert_resource(MatchClock {
        frames_remaining: 10,
        phase: MatchPhase::Regulation,
    });
    let home_most = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 2000.0)],
        Vec3::new(0.0, 500.0, 4.0),
    );
    spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 2000.0)],
        Vec3::new(0.0, 1500.0, 4.0),
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::ZERO,
        Vec3::new(0.0, 0.0, 4.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(0.0, 1000.0),
        Vec3::new(0.0, 1000.0, 4.0),
        None,
    );

    app.update();

    let dy = app.world.get::<Transform>(home_most).unwrap().translation.y - 500.0;
    assert!(
        dy > 0.1,
        "a trailing team commits forward, it is never recalled to camp: {dy}"
    );
}

#[test]
fn a_second_free_car_shields_the_flag_carriers_flank_from_a_second_pursuer() {
    // Red hauls the blue flag home, the carrier just south of centre. Two blue
    // chasers close in: the nearer (west) draws the primary block, so a second free
    // red car peels off to shield the carrier's flank against the second chaser
    // (south), interposing on the ram-range ring just south of the carrier. The
    // flank car sits between the carrier and its northern home base: shielding pulls
    // it south (dy < 0), while with only one chaser it has no flank to shield and
    // takes its home-defence fallback to the north (dy > 0). The opposite signs
    // isolate the new flank-shield behaviour, all within the arena bounds.
    fn flank_dy(second_pursuer: bool) -> f32 {
        let mut app = app_with_system();

        let carrier = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 2000.0)],
            Vec3::new(0.0, -100.0, 4.0),
        );
        // The primary blocker, nearest the western chaser's intercept.
        spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 2000.0)],
            Vec3::new(-160.0, -50.0, 4.0),
        );
        // The flank car under test, north of the carrier, between it and home.
        let flank = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 2000.0)],
            Vec3::new(0.0, 100.0, 4.0),
        );

        // Two blue chasers: west (closest) and, optionally, south (second).
        let west = spawn_ai_on_team(&mut app, AiTeam::Blue, vec![Vec2::ZERO]);
        app.world.get_mut::<Transform>(west).unwrap().translation = Vec3::new(-230.0, -100.0, 4.0);
        if second_pursuer {
            let south = spawn_ai_on_team(&mut app, AiTeam::Blue, vec![Vec2::ZERO]);
            app.world.get_mut::<Transform>(south).unwrap().translation =
                Vec3::new(0.0, -340.0, 4.0);
        }

        // Red home sits north; the blue flag is carried by the red carrier, so the
        // lone chaser's fallback (home defence) lane lies north of the flank car.
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(0.0, 500.0),
            Vec3::new(0.0, 500.0, 4.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(0.0, -500.0),
            Vec3::new(0.0, -100.0, 4.0),
            Some(carrier),
        );

        app.update();

        app.world.get::<Transform>(flank).unwrap().translation.y - 100.0
    }

    let lone_chaser = flank_dy(false);
    let second_chaser = flank_dy(true);

    assert!(
        lone_chaser > 0.1,
        "with a lone chaser the spare car takes its home-defence fallback to the north: {lone_chaser}"
    );
    assert!(
        second_chaser < -0.1,
        "a second chaser pulls a spare car back south to shield the carrier's flank: {second_chaser}"
    );
}

#[test]
fn moves_towards_a_distant_waypoint() {
    let mut app = app_with_system();
    // Facing +Y by default, waypoint straight ahead.
    let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.y > 0.0,
        "expected forward movement, y={}",
        transform.translation.y
    );
}

#[test]
fn carrying_the_enemy_flag_slows_a_virtual_player() {
    use crate::gameplay::ctf::FLAG_CARRIER_SPEED_MULTIPLIER;

    // Control: an empty-handed red patroller driving straight at a waypoint.
    let mut free_app = app_with_system();
    let free_ai = spawn_ai_on_team(&mut free_app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
    free_app.update();
    let free_y = free_app
        .world
        .get::<Transform>(free_ai)
        .unwrap()
        .translation
        .y;

    // Carrier: a red car hauling the blue flag runs home to its red base,
    // which sits straight ahead so the heading (and throttle) match the
    // control exactly. Only the flag-carry tax differs.
    let mut carrier_app = app_with_system();
    let carrier = spawn_ai_on_team(&mut carrier_app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
    spawn_flag(
        &mut carrier_app,
        FlagTeam::Red,
        Vec2::new(0.0, 1000.0),
        Vec3::new(0.0, 1000.0, 4.0),
        None,
    );
    spawn_flag(
        &mut carrier_app,
        FlagTeam::Blue,
        Vec2::new(0.0, -1000.0),
        Vec3::new(0.0, -1000.0, 4.0),
        Some(carrier),
    );
    carrier_app.update();
    let carrier_y = carrier_app
        .world
        .get::<Transform>(carrier)
        .unwrap()
        .translation
        .y;

    assert!(
        carrier_y > 0.0 && carrier_y < free_y,
        "free={free_y}, carrier={carrier_y}"
    );
    assert!(
        (carrier_y - free_y * FLAG_CARRIER_SPEED_MULTIPLIER).abs() <= 1e-3,
        "carrier should drive at the flag-carrier multiplier: free={free_y}, carrier={carrier_y}"
    );
}

#[test]
fn a_virtual_player_drafts_behind_a_car_running_ahead() {
    // Lone control: with no car ahead there is no wake to catch.
    let mut lone_app = app_with_system();
    let lone = spawn_ai_at(
        &mut lone_app,
        vec![Vec2::new(0.0, 2000.0)],
        Vec3::new(0.0, 0.0, 4.0),
    );
    lone_app.update();
    let lone_y = lone_app.world.get::<Transform>(lone).unwrap().translation.y;

    // Drafting: a leader sits directly ahead on the same heading, so the
    // trailing car catches its slipstream and covers more ground in the frame.
    let mut draft_app = app_with_system();
    spawn_ai_at(
        &mut draft_app,
        vec![Vec2::new(0.0, 2000.0)],
        Vec3::new(0.0, 200.0, 4.0),
    );
    let trailing = spawn_ai_at(
        &mut draft_app,
        vec![Vec2::new(0.0, 2000.0)],
        Vec3::new(0.0, 0.0, 4.0),
    );
    draft_app.update();
    let drafting_y = draft_app
        .world
        .get::<Transform>(trailing)
        .unwrap()
        .translation
        .y;

    assert!(
        drafting_y > lone_y,
        "a car tucked in behind a leader should be towed further: lone={lone_y}, \
             drafting={drafting_y}"
    );
}

#[test]
fn a_virtual_player_steers_into_a_wake_on_the_way_to_its_objective() {
    // Control: a lone car with its objective dead ahead holds a straight line, so
    // its lateral position never moves off zero.
    let mut straight_app = app_with_system();
    let straight = spawn_ai_at(
        &mut straight_app,
        vec![Vec2::new(0.0, 4000.0)],
        Vec3::new(0.0, 0.0, 4.0),
    );
    straight_app.update();
    let straight_x = straight_app
        .world
        .get::<Transform>(straight)
        .unwrap()
        .translation
        .x;

    // Seeking: a pace car runs the same way, ahead and off to one side, with the
    // objective far beyond it. The chaser should actively tuck across into the
    // wake rather than hold its straight line, ending the frame moved toward the
    // pace car's lane.
    let mut seek_app = app_with_system();
    spawn_ai_at(
        &mut seek_app,
        vec![Vec2::new(60.0, 4000.0)],
        Vec3::new(60.0, 250.0, 4.0),
    );
    let seeker = spawn_ai_at(
        &mut seek_app,
        vec![Vec2::new(0.0, 4000.0)],
        Vec3::new(0.0, 0.0, 4.0),
    );
    seek_app.update();
    let seeker_x = seek_app
        .world
        .get::<Transform>(seeker)
        .unwrap()
        .translation
        .x;

    assert!(
        straight_x.abs() <= 1e-3,
        "the lone control should hold its line, got x={straight_x}"
    );
    assert!(
        seeker_x > straight_x + 1e-3,
        "a car should steer across into a wake on its way to its objective: \
             straight={straight_x}, seeking={seeker_x}"
    );
}

#[test]
fn grinding_a_wall_scrubs_a_virtual_players_speed() {
    // Control: a car in the open centre driving straight at a waypoint dead
    // ahead, far from every wall so it keeps full pace.
    let mut open_app = app_with_system();
    let open = spawn_ai_at(
        &mut open_app,
        vec![Vec2::new(0.0, 2000.0)],
        Vec3::new(0.0, 0.0, 4.0),
    );
    open_app.update();
    let open_y = open_app.world.get::<Transform>(open).unwrap().translation.y;

    // Wall car: jammed up against the +X wall and driving straight up alongside
    // it (waypoint dead ahead on the same x), so heading and throttle match the
    // control exactly. Only the wall scrape differs.
    let wall_x = BOUNDS.x / 2.0 - 10.0;
    let mut wall_app = app_with_system();
    let wall = spawn_ai_at(
        &mut wall_app,
        vec![Vec2::new(wall_x, 2000.0)],
        Vec3::new(wall_x, 0.0, 4.0),
    );
    wall_app.update();
    let wall_y = wall_app.world.get::<Transform>(wall).unwrap().translation.y;

    let scrape = wall_scrape_speed_multiplier(Vec2::new(wall_x, 0.0), BOUNDS / 2.0);
    assert!(
        scrape < 1.0,
        "the fixture must actually press into the scrape margin, got {scrape}"
    );
    assert!(
        wall_y > 0.0 && wall_y < open_y,
        "open={open_y}, wall={wall_y}"
    );
    assert!(
        (wall_y - open_y * scrape).abs() <= 1e-3,
        "a wall-jammed car should drive at the scrape multiplier: \
             open={open_y}, wall={wall_y}, scrape={scrape}"
    );
}

#[test]
fn a_trailing_teams_catch_up_speeds_a_virtual_player() {
    use crate::gameplay::ctf::CAPTURES_TO_WIN;

    // Control: a level scoreline, so a red patroller earns no catch-up urge.
    let mut level_app = app_with_system();
    let level = spawn_ai_on_team(&mut level_app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
    level_app.update();
    let level_y = level_app
        .world
        .get::<Transform>(level)
        .unwrap()
        .translation
        .y;

    // Trailing: red is down by the largest live deficit, so its non-carrier
    // earns the full catch-up urge and covers more ground on the same heading.
    let mut trailing_app = app_with_system();
    trailing_app.insert_resource(CaptureScore {
        player: CAPTURES_TO_WIN - 1,
        opponents: 0,
    });
    let trailing = spawn_ai_on_team(&mut trailing_app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
    trailing_app.update();
    let trailing_y = trailing_app
        .world
        .get::<Transform>(trailing)
        .unwrap()
        .translation
        .y;

    let catch_up = comeback_speed_multiplier(0, CAPTURES_TO_WIN - 1, false);
    assert!(
        catch_up > 1.0,
        "the fixture must actually trail, got {catch_up}"
    );
    assert!(
        trailing_y > level_y,
        "level={level_y}, trailing={trailing_y}"
    );
    assert!(
        (trailing_y - level_y * catch_up).abs() <= 1e-3,
        "a trailing team's chaser should drive at the catch-up multiplier: \
             level={level_y}, trailing={trailing_y}, catch_up={catch_up}"
    );
}

#[test]
fn a_long_held_flag_tires_a_virtual_player_carrier() {
    use crate::gameplay::carry_fatigue::CARRY_FATIGUE_FULL_FRAMES;

    // A red carrier hauling the blue flag home to its red base dead ahead, so
    // heading and throttle are fixed; only the time on the flag differs. Both
    // runs pay the flat carry tax, so any extra slowdown is fatigue alone.
    fn carrier_y(carry_frames: Option<u32>) -> f32 {
        let mut app = app_with_system();
        let carrier = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(0.0, 1000.0),
            Vec3::new(0.0, 1000.0, 4.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(0.0, -1000.0),
            Vec3::new(0.0, -1000.0, 4.0),
            Some(carrier),
        );
        if let Some(frames) = carry_frames {
            app.insert_resource(FlagCarryTimers {
                blue_frames: frames,
                red_frames: 0,
            });
        }
        app.update();
        app.world.get::<Transform>(carrier).unwrap().translation.y
    }

    // Fresh grab: no carry timer means no fatigue, only the flat tax.
    let fresh_y = carrier_y(None);
    // Long hold: the flag has been carried to the full-fatigue horizon.
    let tired_y = carrier_y(Some(CARRY_FATIGUE_FULL_FRAMES));

    let fatigue = carry_fatigue_speed_multiplier(CARRY_FATIGUE_FULL_FRAMES);
    assert!(
        fatigue < 1.0,
        "the fixture must actually tire, got {fatigue}"
    );
    assert!(
        tired_y > 0.0 && tired_y < fresh_y,
        "fresh={fresh_y}, tired={tired_y}"
    );
    assert!(
        fresh_y.mul_add(-fatigue, tired_y).abs() <= 1e-3,
        "a long-held flag should scrub a carrier's pace on top of the tax: \
             fresh={fresh_y}, tired={tired_y}, fatigue={fatigue}"
    );
}

#[test]
fn a_flag_carrier_catches_no_slipstream() {
    // A red carrier running the blue flag home, measured with and without a
    // team-mate planted directly on its run-home line. A non-carrier in that
    // slot would be towed, but the flag spoils the draft, so the carrier covers
    // the identical ground either way: the slipstream can never speed a flag run.
    fn carrier_y(with_leader: bool) -> f32 {
        let mut app = app_with_system();
        let carrier = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::new(0.0, 1000.0),
            Vec3::new(0.0, 1000.0, 4.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(0.0, -1000.0),
            Vec3::new(0.0, -1000.0, 4.0),
            Some(carrier),
        );
        if with_leader {
            spawn_ai_at(
                &mut app,
                vec![Vec2::new(0.0, 1000.0)],
                Vec3::new(0.0, 200.0, 4.0),
            );
        }
        app.update();
        app.world.get::<Transform>(carrier).unwrap().translation.y
    }

    let alone = carrier_y(false);
    let with_leader = carrier_y(true);
    assert!(
        (alone - with_leader).abs() <= 1e-3,
        "a flag carrier must catch no slipstream: alone={alone}, with_leader={with_leader}"
    );
}

#[test]
fn a_battered_team_parks_its_home_most_car_in_the_pit() {
    // Red home sits at the origin and the blue (enemy) flag straight ahead
    // at +Y, so a healthy red car drives forward to attack it. One frame is
    // run twice: once healthy, once battered.
    fn run(opponent_integrity: f32) -> (f32, f32) {
        let mut app = app_with_system();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            opponent: opponent_integrity,
        });
        // The home-most car spawns exactly on its red base.
        let near = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
        // A distant second red car keeps the team above the lone-car guard.
        let far = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(0.0, 1500.0, 4.0),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::ZERO,
            Vec3::new(0.0, 0.0, 4.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(0.0, 1000.0),
            Vec3::new(0.0, 1000.0, 4.0),
            None,
        );

        app.update();

        let near_y = app.world.get::<Transform>(near).unwrap().translation.y;
        let far_y = app.world.get::<Transform>(far).unwrap().translation.y;
        (near_y, far_y)
    }

    let (healthy_near, _) = run(MAX_INTEGRITY);
    let (battered_near, battered_far) = run(20.0);

    assert!(
        healthy_near > 1.0,
        "a healthy home car should attack, not idle: {healthy_near}"
    );
    assert!(
        battered_near.abs() < 0.001,
        "a battered home-most car should park in its pit: {battered_near}"
    );
    assert!(
        (battered_far - 1500.0).abs() > 0.001,
        "the distant car keeps playing rather than retreating: {battered_far}"
    );
}

#[test]
fn a_battered_retreating_car_weaves_around_a_blocker_on_its_run_home() {
    // Red home sits at the origin. A battered red team sends its home-most car
    // back to pit-recover from straight above its base. With a stationary enemy
    // planted on the run home it weaves off the line to dodge a ram it can least
    // afford; with the lane clear it limps straight back. One frame loop is run
    // twice to compare the retreating car's sideways drift.
    fn run(with_blocker: bool) -> f32 {
        let mut app = app_with_system();
        app.insert_resource(VehicleIntegrity {
            player: MAX_INTEGRITY,
            opponent: 20.0,
        });
        let near = spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 0.0)],
            Vec3::new(0.0, 600.0, 4.0),
        );
        // A distant second red car keeps the team above the lone-car guard.
        spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 0.0)],
            Vec3::new(0.0, 1400.0, 4.0),
        );
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::ZERO,
            Vec3::new(0.0, 0.0, 4.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(0.0, 2000.0),
            Vec3::new(0.0, 2000.0, 4.0),
            None,
        );
        if with_blocker {
            // A stationary enemy dead on the run home, between the retreating
            // car and its base, so it stays a fixed roadblock every frame.
            app.world.spawn((
                VirtualPlayer {
                    team: AiTeam::Blue,
                    movement_speed: 0.0,
                    rotation_speed: 0.0,
                    waypoints: vec![Vec2::new(0.0, 300.0)],
                    current_waypoint: 0,
                    player_pursuit_radius: TEST_PURSUIT_RADIUS,
                    pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                    corner_throttle: 0.3,
                },
                Transform::from_translation(Vec3::new(0.0, 300.0, 4.0)),
            ));
        }

        for _ in 0..20 {
            app.update();
        }

        app.world.get::<Transform>(near).unwrap().translation.x
    }

    let weaved = run(true);
    let straight = run(false);

    assert!(
        straight.abs() < 0.001,
        "with the lane clear the limping car should track straight home: {straight}"
    );
    assert!(
        weaved.abs() > 1.0,
        "with an enemy on the line the limping car should weave off it: {weaved}"
    );
}

#[test]
fn a_lone_battered_car_keeps_playing_instead_of_retreating() {
    let mut app = app_with_system();
    app.insert_resource(VehicleIntegrity {
        player: MAX_INTEGRITY,
        opponent: 10.0,
    });
    // A single red car on its own base, with the enemy flag straight ahead.
    let lone = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::ZERO,
        Vec3::new(0.0, 0.0, 4.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(0.0, 1000.0),
        Vec3::new(0.0, 1000.0, 4.0),
        None,
    );

    app.update();

    let y = app.world.get::<Transform>(lone).unwrap().translation.y;
    assert!(
        y > 1.0,
        "a lone battered car must keep attacking rather than abandon the field: {y}"
    );
}

#[test]
fn a_healthier_team_hunts_a_reeling_enemy() {
    // A red hunter sits at the origin facing +Y with its patrol waypoint
    // straight ahead, while a lone blue car sits straight behind it. The
    // blue team's wear is the variable: healthy blue and the red car drives
    // forward to its waypoint; reeling blue and the red car breaks off,
    // reversing to hunt the battered enemy down.
    fn run(player_integrity: f32) -> f32 {
        let mut app = app_with_system();
        app.insert_resource(VehicleIntegrity {
            player: player_integrity,
            opponent: MAX_INTEGRITY,
        });
        let hunter = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
        // A second red car keeps the team above the lone-car guard and sits
        // far from the prey so the origin car is the one chosen to hunt.
        spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(0.0, 1500.0, 4.0),
        );
        // The blue prey, straight behind the red hunter.
        app.world.spawn((
            VirtualPlayer {
                team: AiTeam::Blue,
                movement_speed: 500.0,
                rotation_speed: f32::to_radians(360.0),
                waypoints: vec![Vec2::new(0.0, -2000.0)],
                current_waypoint: 0,
                player_pursuit_radius: TEST_PURSUIT_RADIUS,
                pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                corner_throttle: 0.3,
            },
            Transform::from_translation(Vec3::new(0.0, -1000.0, 4.0)),
        ));

        app.update();

        app.world.get::<Transform>(hunter).unwrap().translation.y
    }

    let healthy = run(MAX_INTEGRITY);
    let reeling = run(20.0);

    assert!(
        healthy > 1.0,
        "against a healthy enemy the red car attacks forward: {healthy}"
    );
    assert!(
        reeling < -0.001,
        "against a reeling enemy the red car breaks off to hunt it down behind: {reeling}"
    );
}

#[test]
fn a_team_with_cars_to_spare_pincers_a_reeling_enemy() {
    // Three healthy red cars against a lone reeling blue prey straight behind
    // them. The two nearest red cars (B nearest, then A at the origin) both
    // break off to gang up on the kill, springing the combat pincer, while the
    // third (C, farthest) stays on the objective. A lone kill press would send
    // only B and leave A driving forward to its waypoint; A reversing to hunt
    // is the tell that the second hunter joined.
    let mut app = app_with_system();
    app.insert_resource(VehicleIntegrity {
        player: 20.0,
        opponent: MAX_INTEGRITY,
    });
    // A: at the origin facing +Y, waypoint ahead. The pincer partner.
    let car_a = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 600.0)]);
    // B: nearest the prey, the primary hunter.
    let car_b = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 600.0)],
        Vec3::new(0.0, -200.0, 4.0),
    );
    // C: farthest from the prey, stays on the objective driving to its waypoint.
    let car_c = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 600.0)],
        Vec3::new(0.0, 400.0, 4.0),
    );
    // The reeling blue prey, straight behind the red cars.
    app.world.spawn((
        VirtualPlayer {
            team: AiTeam::Blue,
            movement_speed: 500.0,
            rotation_speed: f32::to_radians(360.0),
            waypoints: vec![Vec2::new(0.0, -2000.0)],
            current_waypoint: 0,
            player_pursuit_radius: TEST_PURSUIT_RADIUS,
            pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
            corner_throttle: 0.3,
        },
        Transform::from_translation(Vec3::new(0.0, -500.0, 4.0)),
    ));

    app.update();

    let a_y = app.world.get::<Transform>(car_a).unwrap().translation.y;
    let b_y = app.world.get::<Transform>(car_b).unwrap().translation.y;
    let c_y = app.world.get::<Transform>(car_c).unwrap().translation.y;

    assert!(
        b_y < -200.0,
        "the primary hunter breaks off to chase the prey behind it: {b_y}"
    );
    assert!(
        a_y < 0.0,
        "the spare car joins the pincer, reversing to gang up rather than driving its route: {a_y}"
    );
    assert!(
        c_y > 400.0,
        "the farthest car stays on the objective, never abandoning the field: {c_y}"
    );
}

#[test]
fn a_reeling_team_does_not_over_commit_to_a_kill() {
    // Both teams are battered but level. Neither is the healthier side, so
    // the red car keeps attacking its waypoint rather than trading itself
    // into a mutual wreck chasing the blue car behind it.
    let mut app = app_with_system();
    app.insert_resource(VehicleIntegrity {
        player: 20.0,
        opponent: 20.0,
    });
    let red = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
    spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(0.0, 1500.0, 4.0),
    );
    app.world.spawn((
        VirtualPlayer {
            team: AiTeam::Blue,
            movement_speed: 500.0,
            rotation_speed: f32::to_radians(360.0),
            waypoints: vec![Vec2::new(0.0, -2000.0)],
            current_waypoint: 0,
            player_pursuit_radius: TEST_PURSUIT_RADIUS,
            pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
            corner_throttle: 0.3,
        },
        Transform::from_translation(Vec3::new(0.0, -1000.0, 4.0)),
    ));

    app.update();

    let y = app.world.get::<Transform>(red).unwrap().translation.y;
    assert!(
        y > 1.0,
        "a team that is no healthier than its enemy keeps playing the objective: {y}"
    );
}

#[test]
fn a_team_trailing_on_captures_hunts_the_reeling_leader_at_even_health() {
    // Both teams are equally battered, so durability alone keeps the red car
    // on its objective (see `a_reeling_team_does_not_over_commit_to_a_kill`).
    // The capture scoreline is the variable: once red trails blue it takes
    // the even-health gamble and breaks off to hunt the blue car behind it,
    // the AI mirror of the most-wanted comeback bounty.
    fn run(blue_captures: u32, red_captures: u32) -> f32 {
        let mut app = app_with_system();
        app.insert_resource(VehicleIntegrity {
            player: 20.0,
            opponent: 20.0,
        });
        app.insert_resource(CaptureScore {
            player: blue_captures,
            opponents: red_captures,
        });
        let red = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
        // A second red car keeps the team above the lone-car guard and sits
        // far from the prey so the origin car is the one chosen to hunt.
        spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(0.0, 1500.0, 4.0),
        );
        // The reeling blue prey, straight behind the red hunter.
        app.world.spawn((
            VirtualPlayer {
                team: AiTeam::Blue,
                movement_speed: 500.0,
                rotation_speed: f32::to_radians(360.0),
                waypoints: vec![Vec2::new(0.0, -2000.0)],
                current_waypoint: 0,
                player_pursuit_radius: TEST_PURSUIT_RADIUS,
                pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                corner_throttle: 0.3,
            },
            Transform::from_translation(Vec3::new(0.0, -1000.0, 4.0)),
        ));

        app.update();

        app.world.get::<Transform>(red).unwrap().translation.y
    }

    let level = run(0, 0);
    let trailing = run(2, 0);

    assert!(
        level > 1.0,
        "level on captures the even-health red car keeps to its objective: {level}"
    );
    assert!(
        trailing < -0.001,
        "trailing on captures the red car breaks off to hunt the leader down: {trailing}"
    );
}

#[test]
fn a_trailing_team_chases_a_worn_leader_only_in_closing_time() {
    use crate::gameplay::ctf::{MatchPhase, MATCH_TIME_LIMIT_FRAMES};

    // Both teams are worn past the normal reeling gate but short of the clutch
    // ceiling, so outside the closing stretch durability keeps the trailing red
    // car on its objective. Once the clock runs down, red chases the clutch
    // wreck that can swing the decider: the AI mirror of the combat clutch
    // bonus. Only the clock changes between the two runs.
    fn run(closing: bool) -> f32 {
        let mut app = app_with_system();
        // Worn enough to clear the closing-time clutch ceiling (0.45) yet above
        // the normal reeling gate (0.30): only the clutch window reaches it.
        app.insert_resource(VehicleIntegrity {
            player: 37.5,
            opponent: 37.5,
        });
        // Red trails blue, so the clutch comeback gamble is live for red.
        app.insert_resource(CaptureScore {
            player: 2,
            opponents: 0,
        });
        app.insert_resource(MatchClock {
            frames_remaining: if closing { 10 } else { MATCH_TIME_LIMIT_FRAMES },
            phase: MatchPhase::Regulation,
        });
        let red = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
        // A second red car keeps the team above the lone-car guard and sits far
        // from the prey so the origin car is the one chosen to hunt.
        spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 1000.0)],
            Vec3::new(0.0, 1500.0, 4.0),
        );
        // The worn blue prey, straight behind the red hunter.
        app.world.spawn((
            VirtualPlayer {
                team: AiTeam::Blue,
                movement_speed: 500.0,
                rotation_speed: f32::to_radians(360.0),
                waypoints: vec![Vec2::new(0.0, -2000.0)],
                current_waypoint: 0,
                player_pursuit_radius: TEST_PURSUIT_RADIUS,
                pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                corner_throttle: 0.3,
            },
            Transform::from_translation(Vec3::new(0.0, -1000.0, 4.0)),
        ));

        app.update();

        app.world.get::<Transform>(red).unwrap().translation.y
    }

    let regulation = run(false);
    let closing = run(true);

    assert!(
            regulation > 1.0,
            "outside closing time a worn leader is no target, so red keeps to its objective: {regulation}"
        );
    assert!(
        closing < -0.001,
        "in closing time red breaks off to chase the clutch wreck: {closing}"
    );
}

fn assert_arrive_radius_eq(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= f32::EPSILON,
        "actual={actual}, expected={expected}"
    );
}

#[test]
fn ram_targets_arrive_tighter_than_positional_ones() {
    // Ramming an enemy car means driving through it, so chase targets close
    // far tighter than the waypoint/positional boundary. Measured at the neutral
    // MIN_THROTTLE baseline so the commitment flex is held constant; the
    // tighter < wider invariant itself is enforced at compile time on the
    // PURSUIT_ARRIVE_RADIUS band.
    assert_arrive_radius_eq(
        arrive_radius_for_target(DrivingTarget::FinishWreck(Vec2::ZERO), MIN_THROTTLE),
        PURSUIT_ARRIVE_RADIUS,
    );
    assert_arrive_radius_eq(
        arrive_radius_for_target(DrivingTarget::Player(Vec2::ZERO), MIN_THROTTLE),
        PURSUIT_ARRIVE_RADIUS,
    );
    assert_arrive_radius_eq(
        arrive_radius_for_target(DrivingTarget::PatrolWaypoint(Vec2::ZERO), MIN_THROTTLE),
        WAYPOINT_ARRIVE_RADIUS,
    );
    assert_arrive_radius_eq(
        arrive_radius_for_target(DrivingTarget::EnemyFlag(Vec2::ZERO), MIN_THROTTLE),
        WAYPOINT_ARRIVE_RADIUS,
    );
}

#[test]
fn a_hunter_drives_through_to_ram_a_reeling_enemy_at_close_range() {
    // A red hunter sits at the origin facing +Y with a patrol waypoint
    // straight ahead; a reeling blue car sits just 60 units behind it, well
    // inside the wide waypoint arrive radius yet outside true ram range. The
    // hunter must keep driving back into the prey to land the wreck rather
    // than coasting to an idle at the arrive boundary, short of contact.
    let mut app = app_with_system();
    app.insert_resource(VehicleIntegrity {
        player: 20.0,
        opponent: MAX_INTEGRITY,
    });
    let hunter = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
    // A distant second red car keeps the team above the lone-car guard and
    // sits far from the prey so the origin car is the chosen hunter.
    spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(0.0, 1500.0, 4.0),
    );
    // The reeling blue prey, a close 60 units behind the red hunter: nearer
    // than WAYPOINT_ARRIVE_RADIUS but further than PURSUIT_ARRIVE_RADIUS.
    app.world.spawn((
        VirtualPlayer {
            team: AiTeam::Blue,
            movement_speed: 500.0,
            rotation_speed: f32::to_radians(360.0),
            waypoints: vec![Vec2::new(0.0, -2000.0)],
            current_waypoint: 0,
            player_pursuit_radius: TEST_PURSUIT_RADIUS,
            pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
            corner_throttle: 0.3,
        },
        Transform::from_translation(Vec3::new(0.0, -60.0, 4.0)),
    ));

    app.update();

    let y = app.world.get::<Transform>(hunter).unwrap().translation.y;
    assert!(
        y < -0.001,
        "a hunter must drive through to ram a close reeling enemy, not idle short of it: {y}"
    );
}

#[test]
fn a_hunter_shoves_a_wall_pinned_prey_into_the_boundary() {
    // A reeling blue prey hugs the +x wall (x = 920, inside the crush band);
    // the red hunter sits directly below it on the open side, facing +Y. Aiming
    // straight at the prey would carry the hunter due north (no sideways drift);
    // aiming past it into the wall instead bends the charge toward +x, so a
    // rightward nudge is the tell that the kill press is setting up a wall crush.
    let mut app = app_with_system();
    app.insert_resource(VehicleIntegrity {
        player: 20.0,
        opponent: MAX_INTEGRITY,
    });
    let hunter_start_x = 920.0;
    let hunter = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(hunter_start_x, -800.0, 4.0),
    );
    // A distant second red car keeps the team above the lone-car guard and sits
    // far from the prey so the lower car is the chosen hunter.
    spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(0.0, 3000.0, 4.0),
    );
    // The reeling blue prey, pinned against the +x wall straight above the hunter.
    app.world.spawn((
        VirtualPlayer {
            team: AiTeam::Blue,
            movement_speed: 500.0,
            rotation_speed: f32::to_radians(360.0),
            waypoints: vec![Vec2::new(hunter_start_x, -2000.0)],
            current_waypoint: 0,
            player_pursuit_radius: TEST_PURSUIT_RADIUS,
            pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
            corner_throttle: 0.3,
        },
        Transform::from_translation(Vec3::new(hunter_start_x, -300.0, 4.0)),
    ));

    app.update();

    let x = app.world.get::<Transform>(hunter).unwrap().translation.x;
    assert!(
        x > hunter_start_x + 1e-3,
        "a hunter must bend its charge toward the wall to crush a pinned prey, \
             not drive straight at it: {x}"
    );
}

#[test]
fn a_hunter_cuts_off_a_reeling_prey_fleeing_across_its_path() {
    // A reeling blue prey sits straight ahead of the red hunter but is fleeing to
    // the right (facing +x) at half the hunter's speed, out in the open with no
    // wall to crush against. Aiming at the spot it occupies now would send the
    // hunter due north; leading it to where it is heading bends the charge toward
    // +x to cut it off. A rightward drift is the tell that the kill press is
    // heading the runner off rather than tail-chasing it.
    let mut app = app_with_system();
    app.insert_resource(VehicleIntegrity {
        player: 20.0,
        opponent: MAX_INTEGRITY,
    });
    let hunter = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
    // A distant second red car keeps the team above the lone-car guard (yet short
    // of the three needed to spare a pincer partner) and sits far from the prey so
    // the origin car is the chosen hunter.
    spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(0.0, 1500.0, 4.0),
    );
    // The reeling blue prey, straight ahead of the hunter, fleeing rightward (+x)
    // at half the hunter's top speed so the interception is comfortably solvable.
    app.world.spawn((
        VirtualPlayer {
            team: AiTeam::Blue,
            movement_speed: 250.0,
            rotation_speed: f32::to_radians(360.0),
            waypoints: vec![Vec2::new(2000.0, 300.0)],
            current_waypoint: 0,
            player_pursuit_radius: TEST_PURSUIT_RADIUS,
            pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
            corner_throttle: 0.3,
        },
        Transform::from_translation(Vec3::new(0.0, 300.0, 4.0))
            .with_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)),
    ));

    app.update();

    let transform = app.world.get::<Transform>(hunter).unwrap();
    assert!(
        transform.translation.x > 1e-3,
        "a hunter must bend its charge toward where the prey is fleeing, \
             not drive straight at the spot it has left: {}",
        transform.translation.x
    );
    assert!(
        transform.translation.y > 0.0,
        "the hunter still drives forward onto the prey while cutting it off: {}",
        transform.translation.y
    );
}

#[test]
fn reverses_towards_a_waypoint_behind_the_car() {
    let mut app = app_with_system();
    let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, -1000.0)]);

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.y < 0.0,
        "expected reverse movement, y={}",
        transform.translation.y
    );
}

#[test]
fn finished_match_stops_virtual_players() {
    let mut app = app_with_system();
    app.insert_resource(CtfMatchResult {
        winner: Some(CtfMatchWinner::Player),
    });
    let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert_eq!(transform.translation, Vec3::new(0.0, 0.0, 4.0));
}

#[test]
fn advances_waypoint_once_arrived() {
    let mut app = app_with_system();
    // Start already on top of the first waypoint so it should advance.
    let ai = spawn_ai(&mut app, vec![Vec2::ZERO, Vec2::new(500.0, 0.0)]);

    app.update();

    let vp = app.world.get::<VirtualPlayer>(ai).unwrap();
    assert_eq!(vp.current_waypoint, 1);
}

#[test]
fn stays_within_arena_bounds() {
    let mut app = app_with_system();
    let edge = Vec3::new(BOUNDS.x / 2.0, BOUNDS.y / 2.0, 4.0);
    let ai = app
        .world
        .spawn((
            VirtualPlayer {
                team: AiTeam::Red,
                movement_speed: 5000.0,
                rotation_speed: f32::to_radians(360.0),
                waypoints: vec![Vec2::new(BOUNDS.x, BOUNDS.y)],
                current_waypoint: 0,
                player_pursuit_radius: TEST_PURSUIT_RADIUS,
                pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                corner_throttle: 0.3,
            },
            Transform::from_translation(edge),
        ))
        .id();

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(transform.translation.x <= BOUNDS.x / 2.0 + 1e-3);
    assert!(transform.translation.y <= BOUNDS.y / 2.0 + 1e-3);
}

#[test]
fn idle_ai_without_waypoints_does_not_panic() {
    let mut app = app_with_system();
    let ai = spawn_ai(&mut app, vec![]);

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert_eq!(transform.translation, Vec3::new(0.0, 0.0, 4.0));
}

#[test]
fn pursues_nearby_pickup_before_patrol_waypoint() {
    let mut app = app_with_system();
    let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Cash,
        },
        Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
    ));

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.x > 0.0,
        "expected opponent to turn towards pickup, x={}",
        transform.translation.x
    );
}

#[test]
fn pursues_nearby_player_before_patrol_waypoint() {
    let mut app = app_with_system();
    let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    spawn_player(&mut app, Vec3::new(200.0, 0.0, 5.0));

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.x > 0.0,
        "expected opponent to turn towards player, x={}",
        transform.translation.x
    );
}

#[test]
fn an_eager_personality_hunts_a_player_a_cautious_one_leaves_alone() {
    // Same player, same patrol route, two different driving personalities. The
    // human sits 300 units to the right; the patrol waypoint is far up the
    // y-axis the car already faces. The drive system must honour each car's own
    // pursuit radius, not a shared global, so eagerness is a genuine
    // personality trait rather than uniform across the roster.
    let player = Vec3::new(300.0, 0.0, 5.0);

    // Cautious technician-style car: 200-unit reach falls short of the player,
    // so it stays disciplined and keeps lapping its patrol route.
    let mut cautious_app = app_with_system();
    let cautious = spawn_ai_with_pursuit(
        &mut cautious_app,
        AiTeam::Red,
        vec![Vec2::new(0.0, 1000.0)],
        200.0,
    );
    spawn_player(&mut cautious_app, player);
    cautious_app.update();
    let cautious_transform = cautious_app.world.get::<Transform>(cautious).unwrap();
    assert!(
        cautious_transform.translation.x.abs() < 1e-4,
        "a cautious driver leaves a player beyond its reach alone, x={}",
        cautious_transform.translation.x
    );
    assert!(
        cautious_transform.translation.y > 0.0,
        "a cautious driver keeps lapping its patrol route, y={}",
        cautious_transform.translation.y
    );

    // Eager sprinter-style car: 400-unit reach covers the same player, so it
    // breaks off the route to run the player down.
    let mut eager_app = app_with_system();
    let eager = spawn_ai_with_pursuit(
        &mut eager_app,
        AiTeam::Red,
        vec![Vec2::new(0.0, 1000.0)],
        400.0,
    );
    spawn_player(&mut eager_app, player);
    eager_app.update();
    let eager_transform = eager_app.world.get::<Transform>(eager).unwrap();
    assert!(
        eager_transform.translation.x > 0.0,
        "an eager driver runs down a player within its reach, x={}",
        eager_transform.translation.x
    );
}

#[test]
fn a_greedy_personality_scavenges_a_pickup_a_disciplined_one_ignores() {
    // Same pickup, same patrol route, two different driving personalities. A
    // cash bag sits 480 units to the right, just outside the former uniform
    // 450-unit reach; the patrol waypoint is far up the y-axis the car already
    // faces. The drive system must honour each car's own pickup-scavenging
    // radius, not a shared global, so greed is a genuine personality trait
    // rather than uniform across the roster.
    fn spawn_cash(app: &mut App, position: Vec3) {
        app.world.spawn((
            Pickup {
                kind: PickupKind::Cash,
            },
            Transform::from_translation(position),
        ));
    }
    let pickup = Vec3::new(480.0, 0.0, 2.0);

    // Disciplined technician-style car: 380-unit greed falls short of the bag,
    // so it stays on its line and keeps lapping its patrol route.
    let mut disciplined_app = app_with_system();
    let disciplined =
        spawn_ai_with_pickup_pursuit(&mut disciplined_app, vec![Vec2::new(0.0, 1000.0)], 380.0);
    spawn_cash(&mut disciplined_app, pickup);
    disciplined_app.update();
    let disciplined_transform = disciplined_app.world.get::<Transform>(disciplined).unwrap();
    assert!(
        disciplined_transform.translation.x.abs() < 1e-4,
        "a disciplined driver leaves a pickup beyond its reach alone, x={}",
        disciplined_transform.translation.x
    );
    assert!(
        disciplined_transform.translation.y > 0.0,
        "a disciplined driver keeps lapping its patrol route, y={}",
        disciplined_transform.translation.y
    );

    // Greedy sprinter-style car: 520-unit greed covers the same bag, so it
    // breaks off the route to scavenge it.
    let mut greedy_app = app_with_system();
    let greedy = spawn_ai_with_pickup_pursuit(&mut greedy_app, vec![Vec2::new(0.0, 1000.0)], 520.0);
    spawn_cash(&mut greedy_app, pickup);
    greedy_app.update();
    let greedy_transform = greedy_app.world.get::<Transform>(greedy).unwrap();
    assert!(
        greedy_transform.translation.x > 0.0,
        "a greedy driver breaks off to scavenge a pickup within its reach, x={}",
        greedy_transform.translation.x
    );
}

#[test]
fn blue_virtual_player_does_not_chase_human_teammate() {
    let mut app = app_with_system();
    let ai = spawn_ai_on_team(&mut app, AiTeam::Blue, vec![Vec2::new(0.0, 1000.0)]);
    spawn_player(&mut app, Vec3::new(200.0, 0.0, 5.0));

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.x.abs() < 1e-4,
        "expected blue teammate to stay on patrol, x={}",
        transform.translation.x
    );
    assert!(
        transform.translation.y > 0.0,
        "expected blue teammate to keep moving, y={}",
        transform.translation.y
    );
}

#[test]
fn blue_virtual_player_leaves_player_claimed_pickup_alone() {
    let mut app = app_with_system();
    let ai = spawn_ai_on_team(&mut app, AiTeam::Blue, vec![Vec2::new(0.0, 1000.0)]);
    spawn_player(&mut app, Vec3::new(180.0, 0.0, 5.0));
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Cash,
        },
        Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
    ));

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.x.abs() < 1e-4,
        "expected teammate to leave player-claimed pickup alone, x={}",
        transform.translation.x
    );
    assert!(
        transform.translation.y > 0.0,
        "expected teammate to keep patrolling, y={}",
        transform.translation.y
    );
}

#[test]
fn blue_virtual_player_does_not_claim_pickup_when_player_is_closer() {
    let pickup = PickupTarget {
        position: Vec2::new(200.0, 0.0),
        priority: 100,
    };

    let yields = virtual_player_yields_player_pickup_claim(
        AiTeam::Blue,
        Some(Vec2::new(180.0, 0.0)),
        pickup,
        Vec2::ZERO,
    );

    assert!(yields);
}

#[test]
fn red_virtual_player_claims_pickup_even_when_player_is_closer() {
    let pickup = PickupTarget {
        position: Vec2::new(200.0, 0.0),
        priority: 100,
    };

    let yields = virtual_player_yields_player_pickup_claim(
        AiTeam::Red,
        Some(Vec2::new(180.0, 0.0)),
        pickup,
        Vec2::ZERO,
    );

    assert!(!yields);
}

#[test]
fn red_virtual_player_contests_player_claimed_pickup() {
    let mut app = app_with_system();
    let ai = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
    spawn_player(&mut app, Vec3::new(180.0, 0.0, 5.0));
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Cash,
        },
        Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
    ));

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.x > 0.0,
        "expected opponent to contest player-claimed pickup, x={}",
        transform.translation.x
    );
}

#[test]
fn pickup_stays_higher_priority_than_player_chase() {
    let mut app = app_with_system();
    let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    spawn_player(&mut app, Vec3::new(200.0, 0.0, 5.0));
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Repair,
        },
        Transform::from_translation(Vec3::new(-200.0, 0.0, 2.0)),
    ));

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.x < 0.0,
        "expected opponent to prioritise pickup, x={}",
        transform.translation.x
    );
}

#[test]
fn pursues_richer_pickup_before_closer_pickup() {
    let mut app = app_with_system();
    let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Repair,
        },
        Transform::from_translation(Vec3::new(-25.0, 0.0, 2.0)),
    ));
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Cash,
        },
        Transform::from_translation(Vec3::new(150.0, 0.0, 2.0)),
    ));

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.x > 0.0,
        "expected opponent to turn towards richer pickup, x={}",
        transform.translation.x
    );
}

#[test]
fn pursues_nitro_before_cash_for_race_pressure() {
    let mut app = app_with_system();
    let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Cash,
        },
        Transform::from_translation(Vec3::new(-25.0, 0.0, 2.0)),
    ));
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Nitro,
        },
        Transform::from_translation(Vec3::new(150.0, 0.0, 2.0)),
    ));

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.x > 0.0,
        "expected opponent to prioritise nitro pressure, x={}",
        transform.translation.x
    );
}

#[test]
fn only_one_virtual_player_pursues_a_shared_pickup() {
    let mut app = app_with_system();
    let first_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    let second_ai = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(0.0, 100.0, 4.0),
    );
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Cash,
        },
        Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
    ));

    app.update();

    let first_transform = app.world.get::<Transform>(first_ai).unwrap();
    let second_transform = app.world.get::<Transform>(second_ai).unwrap();

    assert!(
        first_transform.translation.x > 0.0,
        "expected first opponent to claim pickup, x={}",
        first_transform.translation.x
    );
    assert!(
        second_transform.translation.x.abs() < 1e-4,
        "expected second opponent to keep patrol line, x={}",
        second_transform.translation.x
    );
    assert!(
        second_transform.translation.y > 100.0,
        "expected second opponent to keep moving, y={}",
        second_transform.translation.y
    );
}

#[test]
fn nearby_teammates_spread_out_before_patrolling() {
    let mut app = app_with_system();
    let left_ai = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(0.0, 0.0, 4.0),
    );
    let right_ai = spawn_ai_at(
        &mut app,
        vec![Vec2::new(40.0, 1000.0)],
        Vec3::new(40.0, 0.0, 4.0),
    );

    app.update();

    let left_transform = app.world.get::<Transform>(left_ai).unwrap();
    let right_transform = app.world.get::<Transform>(right_ai).unwrap();

    assert!(
        left_transform.translation.x < 0.0,
        "expected left teammate to steer away, x={}",
        left_transform.translation.x
    );
    assert!(
        right_transform.translation.x > 40.0,
        "expected right teammate to steer away, x={}",
        right_transform.translation.x
    );
}

#[test]
fn closest_virtual_player_claims_shared_pickup_even_if_spawned_later() {
    let mut app = app_with_system();
    let far_ai = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(0.0, 0.0, 4.0),
    );
    let close_ai = spawn_ai_at(
        &mut app,
        vec![Vec2::new(120.0, 1000.0)],
        Vec3::new(120.0, 0.0, 4.0),
    );
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Cash,
        },
        Transform::from_translation(Vec3::new(220.0, 0.0, 2.0)),
    ));

    app.update();

    let far_transform = app.world.get::<Transform>(far_ai).unwrap();
    let close_transform = app.world.get::<Transform>(close_ai).unwrap();

    assert!(
        far_transform.translation.x.abs() < 1e-4,
        "expected farther opponent to keep patrol line, x={}",
        far_transform.translation.x
    );
    assert!(
        far_transform.translation.y > 0.0,
        "expected farther opponent to keep moving, y={}",
        far_transform.translation.y
    );
    assert!(
        close_transform.translation.x > 120.0,
        "expected closer opponent to claim pickup, x={}",
        close_transform.translation.x
    );
}

#[test]
fn second_virtual_player_claims_next_pickup_when_closest_ai_is_busy() {
    let mut app = app_with_system();
    let close_ai = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(0.0, 0.0, 4.0),
    );
    let far_ai = spawn_ai_at(
        &mut app,
        vec![Vec2::new(300.0, 1000.0)],
        Vec3::new(300.0, 0.0, 4.0),
    );
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Nitro,
        },
        Transform::from_translation(Vec3::new(-150.0, 0.0, 2.0)),
    ));
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Cash,
        },
        Transform::from_translation(Vec3::new(80.0, 0.0, 2.0)),
    ));

    app.update();

    let close_transform = app.world.get::<Transform>(close_ai).unwrap();
    let far_transform = app.world.get::<Transform>(far_ai).unwrap();

    assert!(
        close_transform.translation.x < 0.0,
        "expected closest opponent to take high-value nitro, x={}",
        close_transform.translation.x
    );
    assert!(
        far_transform.translation.x < 300.0,
        "expected second opponent to claim remaining cash, x={}",
        far_transform.translation.x
    );
}

#[test]
fn only_one_virtual_player_intercepts_home_flag_threat() {
    let first = CtfTargetCandidate {
        entity: Entity::from_raw(1),
        team: AiTeam::Red,
        position: Vec2::new(350.0, 0.0),
        target: DrivingTarget::DefendHomeBase(Vec2::new(360.0, 0.0)),
        home_base: Vec2::new(500.0, 0.0),
        carries_enemy_flag: false,
    };
    let second = CtfTargetCandidate {
        entity: Entity::from_raw(2),
        team: AiTeam::Red,
        position: Vec2::new(0.0, 0.0),
        target: DrivingTarget::DefendHomeBase(Vec2::new(360.0, 0.0)),
        home_base: Vec2::new(500.0, 0.0),
        carries_enemy_flag: false,
    };
    let assignments = assign_ctf_targets(
        &[first, second],
        &[FlagTarget {
            team: AiTeam::Red,
            home: Vec2::new(500.0, 0.0),
            position: Vec2::new(500.0, 0.0),
            holder: None,
        }],
    );

    assert_eq!(
        assignments,
        vec![
            (
                Entity::from_raw(1),
                Some(DrivingTarget::DefendHomeBase(Vec2::new(360.0, 0.0)))
            ),
            (
                Entity::from_raw(2),
                Some(DrivingTarget::DefendHomeBase(Vec2::new(500.0, 0.0)))
            ),
        ]
    );
}

#[test]
fn spare_defender_guards_home_flag_lane() {
    let attacker = CtfTargetCandidate {
        entity: Entity::from_raw(1),
        team: AiTeam::Red,
        position: Vec2::new(-300.0, 0.0),
        target: DrivingTarget::EnemyFlag(Vec2::new(-500.0, 0.0)),
        home_base: Vec2::new(500.0, 0.0),
        carries_enemy_flag: false,
    };
    let spare = CtfTargetCandidate {
        entity: Entity::from_raw(2),
        team: AiTeam::Red,
        position: Vec2::new(450.0, 0.0),
        target: DrivingTarget::EnemyFlag(Vec2::new(-500.0, 0.0)),
        home_base: Vec2::new(500.0, 0.0),
        carries_enemy_flag: false,
    };
    let assignments = assign_ctf_targets(
        &[attacker, spare],
        &[
            FlagTarget {
                team: AiTeam::Red,
                home: Vec2::new(500.0, 0.0),
                position: Vec2::new(500.0, 0.0),
                holder: None,
            },
            FlagTarget {
                team: AiTeam::Blue,
                home: Vec2::new(-500.0, 0.0),
                position: Vec2::new(-500.0, 0.0),
                holder: None,
            },
        ],
    );

    assert_eq!(
        assignments,
        vec![
            (
                Entity::from_raw(1),
                Some(DrivingTarget::EnemyFlag(Vec2::new(-500.0, 0.0)))
            ),
            (
                Entity::from_raw(2),
                Some(DrivingTarget::DefendHomeBase(Vec2::new(280.0, 0.0)))
            ),
        ]
    );
}

#[test]
fn equal_distance_ctf_role_assignment_uses_position_tiebreakers() {
    let left = CtfTargetCandidate {
        entity: Entity::from_raw(1),
        team: AiTeam::Red,
        position: Vec2::new(-50.0, 0.0),
        target: DrivingTarget::EnemyFlag(Vec2::ZERO),
        home_base: Vec2::new(500.0, 0.0),
        carries_enemy_flag: false,
    };
    let right = CtfTargetCandidate {
        entity: Entity::from_raw(2),
        team: AiTeam::Red,
        position: Vec2::new(50.0, 0.0),
        target: DrivingTarget::EnemyFlag(Vec2::ZERO),
        home_base: Vec2::new(500.0, 0.0),
        carries_enemy_flag: false,
    };
    let flags = [FlagTarget {
        team: AiTeam::Red,
        home: Vec2::new(500.0, 0.0),
        position: Vec2::new(500.0, 0.0),
        holder: None,
    }];

    let assignments = assign_ctf_targets(&[right, left], &flags);

    assert_eq!(
        assignments,
        vec![
            (
                Entity::from_raw(2),
                Some(DrivingTarget::DefendHomeBase(Vec2::new(500.0, 0.0)))
            ),
            (
                Entity::from_raw(1),
                Some(DrivingTarget::EnemyFlag(Vec2::ZERO))
            ),
        ]
    );
}

#[test]
fn pursues_blue_flag_before_pickup_or_patrol_waypoint() {
    let mut app = app_with_system();
    let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(-200.0, 0.0, 2.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(500.0, 0.0, 2.0),
        None,
    );
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Cash,
        },
        Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
    ));

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.x < 0.0,
        "expected opponent to turn towards blue flag, x={}",
        transform.translation.x
    );
}

#[test]
fn flag_carrier_ignores_pickup_behind_route_home() {
    let mut app = app_with_system();
    let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(0.0, 0.0, 2.0),
        Some(ai),
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(500.0, 0.0, 2.0),
        None,
    );
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Cash,
        },
        Transform::from_translation(Vec3::new(-200.0, 0.0, 2.0)),
    ));

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.x > 0.0,
        "expected flag carrier to turn towards home base, x={}",
        transform.translation.x
    );
}

#[test]
fn defends_red_flag_when_player_is_about_to_steal_it() {
    let mut app = app_with_system();
    let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    spawn_player(&mut app, Vec3::new(250.0, 0.0, 5.0));
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(-400.0, 0.0, 2.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(500.0, 0.0, 2.0),
        None,
    );

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.x > 0.0,
        "expected opponent to defend the threatened red flag, x={}",
        transform.translation.x
    );
}

#[test]
fn blue_virtual_player_pursues_red_flag() {
    let mut app = app_with_system();
    let ai = spawn_ai_on_team(&mut app, AiTeam::Blue, vec![Vec2::new(0.0, 1000.0)]);
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(-500.0, 0.0, 2.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(200.0, 0.0, 2.0),
        None,
    );

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.x > 0.0,
        "expected blue opponent to turn towards red flag, x={}",
        transform.translation.x
    );
}

#[test]
fn attacker_detours_for_closer_pickup_along_blue_flag_push() {
    let mut app = app_with_system();
    let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(-400.0, 0.0, 2.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(500.0, 0.0, 2.0),
        None,
    );
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Cash,
        },
        Transform::from_translation(Vec3::new(-100.0, 0.0, 2.0)),
    ));

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.x < 0.0,
        "expected attacker to stay on the flag-side pickup lane, x={}",
        transform.translation.x
    );
}

#[test]
fn flag_carrier_returns_to_red_base() {
    let mut app = app_with_system();
    let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(0.0, 0.0, 2.0),
        Some(ai),
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(500.0, 0.0, 2.0),
        None,
    );

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.x > 0.0,
        "expected flag carrier to turn towards red base, x={}",
        transform.translation.x
    );
}

#[test]
fn flag_carrier_stages_outside_contested_red_base() {
    let mut app = app_with_system();
    let ai = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(500.0, 0.0, 4.0),
    );
    spawn_player(&mut app, Vec3::new(500.0, 120.0, 5.0));
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(0.0, 0.0, 2.0),
        Some(ai),
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(500.0, 0.0, 2.0),
        None,
    );

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.y < 0.0,
        "expected flag carrier to stage away from red-base contest, y={}",
        transform.translation.y
    );
}

#[test]
fn teammate_clears_contested_red_base_for_flag_carrier() {
    let mut app = app_with_system();
    let carrier = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(300.0, 0.0, 4.0),
    );
    let defender = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(560.0, 0.0, 4.0),
    );
    spawn_player(&mut app, Vec3::new(430.0, 0.0, 5.0));
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(300.0, 0.0, 2.0),
        Some(carrier),
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(500.0, 0.0, 2.0),
        None,
    );

    app.update();

    let carrier_transform = app.world.get::<Transform>(carrier).unwrap();
    let defender_transform = app.world.get::<Transform>(defender).unwrap();
    assert!(
        carrier_transform.translation.x > 300.0,
        "expected carrier to keep pushing home, x={}",
        carrier_transform.translation.x
    );
    assert!(
        defender_transform.translation.x < 560.0,
        "expected teammate to clear base contester, x={}",
        defender_transform.translation.x
    );
}

#[test]
fn flag_carrier_intercepts_stolen_home_flag_before_scoring() {
    let mut app = app_with_system();
    let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    let player = spawn_player(&mut app, Vec3::new(-800.0, 0.0, 5.0));
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(0.0, 0.0, 2.0),
        Some(ai),
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(-800.0, 0.0, 2.0),
        Some(player),
    );

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.x < 0.0,
        "expected flag carrier to intercept stolen home flag, x={}",
        transform.translation.x
    );
}

#[test]
fn teammate_defends_stolen_home_flag_before_flag_carrier() {
    let mut app = app_with_system();
    let carrier = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    let defender = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(0.0, 50.0, 4.0),
    );
    let player = spawn_player(&mut app, Vec3::new(-800.0, 0.0, 5.0));
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(0.0, 0.0, 2.0),
        Some(carrier),
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(-800.0, 0.0, 2.0),
        Some(player),
    );

    app.update();

    let carrier_transform = app.world.get::<Transform>(carrier).unwrap();
    let defender_transform = app.world.get::<Transform>(defender).unwrap();
    assert!(
        carrier_transform.translation.x > 0.0,
        "flag carrier should wait on the scoring route, x={}",
        carrier_transform.translation.x
    );
    assert!(
        defender_transform.translation.x < 0.0,
        "free teammate should chase the stolen home flag, x={}",
        defender_transform.translation.x
    );
}

#[test]
fn teammate_escorts_flag_carrier_before_pickup_or_patrol_waypoint() {
    let mut app = app_with_system();
    let carrier = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    let escort = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(-200.0, 0.0, 2.0),
        Some(carrier),
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(500.0, 0.0, 2.0),
        None,
    );
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Cash,
        },
        Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
    ));

    app.update();

    let transform = app.world.get::<Transform>(escort).unwrap();
    assert!(
        transform.translation.x > 0.0,
        "expected escort to lead the carrier towards home, x={}",
        transform.translation.x
    );
}

#[test]
fn teammate_blocks_nearby_flag_carrier_pursuer() {
    let mut app = app_with_system();
    let carrier = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(-120.0, 0.0, 4.0),
    );
    let blocker = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    spawn_player(&mut app, Vec3::new(-240.0, 0.0, 5.0));
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(-120.0, 0.0, 2.0),
        Some(carrier),
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(500.0, 0.0, 2.0),
        None,
    );

    app.update();

    let blocker_transform = app.world.get::<Transform>(blocker).unwrap();
    assert!(
        blocker_transform.translation.x < 0.0,
        "expected teammate to block the pursuer, x={}",
        blocker_transform.translation.x
    );
}

#[test]
fn only_one_teammate_escorts_flag_carrier() {
    let mut app = app_with_system();
    let carrier = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    let first_escort = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    let second_escort = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(-200.0, 0.0, 2.0),
        Some(carrier),
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(500.0, 0.0, 2.0),
        None,
    );

    app.update();

    let first_transform = app.world.get::<Transform>(first_escort).unwrap();
    let second_transform = app.world.get::<Transform>(second_escort).unwrap();

    assert!(
        first_transform.translation.x > 0.0,
        "expected first teammate to lead the carrier home, x={}",
        first_transform.translation.x
    );
    assert!(
        second_transform.translation.x > 0.0,
        "expected spare teammate to defend the red base, x={}",
        second_transform.translation.x
    );
}

#[test]
fn defender_intercepts_stolen_red_flag_before_enemy_flag() {
    let mut app = app_with_system();
    let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    let player = spawn_player(&mut app, Vec3::new(300.0, 0.0, 5.0));
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(-200.0, 0.0, 2.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(200.0, 0.0, 2.0),
        Some(player),
    );

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.x > 0.0,
        "expected defender to cut off the stolen red flag, x={}",
        transform.translation.x
    );
}

#[test]
fn spare_defender_screens_stolen_home_flag_route() {
    let flag_hunter = CtfTargetCandidate {
        entity: Entity::from_raw(1),
        team: AiTeam::Red,
        position: Vec2::new(-700.0, 0.0),
        target: DrivingTarget::StolenHomeFlag(Vec2::new(-660.0, 0.0)),
        home_base: Vec2::new(500.0, 0.0),
        carries_enemy_flag: false,
    };
    let route_screen = CtfTargetCandidate {
        entity: Entity::from_raw(2),
        team: AiTeam::Red,
        position: Vec2::new(-100.0, 0.0),
        target: DrivingTarget::StolenHomeFlag(Vec2::new(-660.0, 0.0)),
        home_base: Vec2::new(500.0, 0.0),
        carries_enemy_flag: false,
    };
    let home_guard = CtfTargetCandidate {
        entity: Entity::from_raw(3),
        team: AiTeam::Red,
        position: Vec2::new(450.0, 0.0),
        target: DrivingTarget::StolenHomeFlag(Vec2::new(-660.0, 0.0)),
        home_base: Vec2::new(500.0, 0.0),
        carries_enemy_flag: false,
    };
    let assignments = assign_ctf_targets(
        &[flag_hunter, route_screen, home_guard],
        &[
            FlagTarget {
                team: AiTeam::Red,
                home: Vec2::new(500.0, 0.0),
                position: Vec2::new(-800.0, 0.0),
                holder: Some(Entity::from_raw(42)),
            },
            FlagTarget {
                team: AiTeam::Blue,
                home: Vec2::new(-500.0, 0.0),
                position: Vec2::new(-500.0, 0.0),
                holder: None,
            },
        ],
    );

    assert_eq!(
        assignments,
        vec![
            (
                Entity::from_raw(1),
                Some(DrivingTarget::StolenHomeFlag(Vec2::new(-660.0, 0.0)))
            ),
            (
                Entity::from_raw(2),
                Some(DrivingTarget::StolenHomeFlagRouteGuard(Vec2::new(
                    -150.0, 0.0
                )))
            ),
            (
                Entity::from_raw(3),
                Some(DrivingTarget::DefendHomeBase(Vec2::new(280.0, 0.0)))
            ),
        ]
    );
}

#[test]
fn defender_intercepts_current_carrier_for_held_home_flag() {
    let mut app = app_with_system();
    let ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    let player = spawn_player(&mut app, Vec3::new(250.0, 0.0, 5.0));
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(-200.0, 0.0, 2.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(-250.0, 0.0, 2.0),
        Some(player),
    );

    app.update();

    let transform = app.world.get::<Transform>(ai).unwrap();
    assert!(
        transform.translation.x > 0.0,
        "expected defender to cut off the current carrier, x={}",
        transform.translation.x
    );
}

#[test]
fn only_one_virtual_player_intercepts_stolen_red_flag() {
    let mut app = app_with_system();
    let first_ai = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(0.0, -50.0, 4.0),
    );
    let second_ai = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(0.0, 50.0, 4.0),
    );
    let player = spawn_player(&mut app, Vec3::new(-800.0, 0.0, 5.0));
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(-500.0, 0.0, 2.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(-800.0, 0.0, 2.0),
        Some(player),
    );

    app.update();

    let first_transform = app.world.get::<Transform>(first_ai).unwrap();
    let second_transform = app.world.get::<Transform>(second_ai).unwrap();
    assert!(
        first_transform.translation.x < 0.0,
        "first opponent should hunt the flag carrier, x={}",
        first_transform.translation.x
    );
    assert!(
        second_transform.translation.x < 0.0,
        "second opponent should screen the stolen-flag route, x={}",
        second_transform.translation.x
    );
}

#[test]
fn only_one_virtual_player_pursues_a_shared_enemy_flag() {
    let mut app = app_with_system();
    let first_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    let second_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(-200.0, 0.0, 2.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(500.0, 0.0, 2.0),
        None,
    );
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Cash,
        },
        Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
    ));

    app.update();

    let first_transform = app.world.get::<Transform>(first_ai).unwrap();
    let second_transform = app.world.get::<Transform>(second_ai).unwrap();

    assert!(
        first_transform.translation.x < 0.0,
        "expected first opponent to claim the blue flag, x={}",
        first_transform.translation.x
    );
    assert!(
        second_transform.translation.x > 0.0,
        "expected second opponent to race for another objective, x={}",
        second_transform.translation.x
    );
}

#[test]
fn spare_attacker_defends_home_base_when_enemy_flag_is_claimed() {
    let mut app = app_with_system();
    let first_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    let second_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(-200.0, 0.0, 2.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(500.0, 0.0, 2.0),
        None,
    );

    app.update();

    let first_transform = app.world.get::<Transform>(first_ai).unwrap();
    let second_transform = app.world.get::<Transform>(second_ai).unwrap();

    assert!(
        first_transform.translation.x < 0.0,
        "expected first opponent to claim the blue flag, x={}",
        first_transform.translation.x
    );
    assert!(
        second_transform.translation.x > 0.0,
        "expected spare opponent to defend the red base, x={}",
        second_transform.translation.x
    );
}

#[test]
fn extra_spare_virtual_player_blocks_midfield_lane() {
    let attacker = CtfTargetCandidate {
        entity: Entity::from_raw(1),
        team: AiTeam::Red,
        position: Vec2::new(-300.0, 0.0),
        target: DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)),
        home_base: Vec2::new(500.0, 0.0),
        carries_enemy_flag: false,
    };
    let close_spare = CtfTargetCandidate {
        entity: Entity::from_raw(2),
        team: AiTeam::Red,
        position: Vec2::new(450.0, 0.0),
        target: DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)),
        home_base: Vec2::new(500.0, 0.0),
        carries_enemy_flag: false,
    };
    let far_spare = CtfTargetCandidate {
        entity: Entity::from_raw(3),
        team: AiTeam::Red,
        position: Vec2::new(0.0, 0.0),
        target: DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)),
        home_base: Vec2::new(500.0, 0.0),
        carries_enemy_flag: false,
    };
    let assignments = assign_ctf_targets(
        &[attacker, close_spare, far_spare],
        &[
            FlagTarget {
                team: AiTeam::Red,
                home: Vec2::new(500.0, 0.0),
                position: Vec2::new(500.0, 0.0),
                holder: None,
            },
            FlagTarget {
                team: AiTeam::Blue,
                home: Vec2::new(-500.0, 0.0),
                position: Vec2::new(-400.0, 0.0),
                holder: None,
            },
        ],
    );

    assert_eq!(
        assignments,
        vec![
            (
                Entity::from_raw(1),
                Some(DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)))
            ),
            (
                Entity::from_raw(2),
                Some(DrivingTarget::DefendHomeBase(Vec2::new(280.0, 0.0)))
            ),
            (
                Entity::from_raw(3),
                Some(DrivingTarget::MidfieldInterceptor(Vec2::ZERO))
            ),
        ]
    );
}

#[test]
fn fourth_spare_virtual_player_flanks_enemy_flag() {
    let attacker = CtfTargetCandidate {
        entity: Entity::from_raw(1),
        team: AiTeam::Red,
        position: Vec2::new(-360.0, 0.0),
        target: DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)),
        home_base: Vec2::new(500.0, 0.0),
        carries_enemy_flag: false,
    };
    let close_home_guard = CtfTargetCandidate {
        entity: Entity::from_raw(2),
        team: AiTeam::Red,
        position: Vec2::new(450.0, 0.0),
        target: DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)),
        home_base: Vec2::new(500.0, 0.0),
        carries_enemy_flag: false,
    };
    let midfield_guard = CtfTargetCandidate {
        entity: Entity::from_raw(3),
        team: AiTeam::Red,
        position: Vec2::ZERO,
        target: DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)),
        home_base: Vec2::new(500.0, 0.0),
        carries_enemy_flag: false,
    };
    let flanker = CtfTargetCandidate {
        entity: Entity::from_raw(4),
        team: AiTeam::Red,
        position: Vec2::new(-400.0, -160.0),
        target: DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)),
        home_base: Vec2::new(500.0, 0.0),
        carries_enemy_flag: false,
    };
    let assignments = assign_ctf_targets(
        &[attacker, close_home_guard, midfield_guard, flanker],
        &[
            FlagTarget {
                team: AiTeam::Red,
                home: Vec2::new(500.0, 0.0),
                position: Vec2::new(500.0, 0.0),
                holder: None,
            },
            FlagTarget {
                team: AiTeam::Blue,
                home: Vec2::new(-500.0, 0.0),
                position: Vec2::new(-400.0, 0.0),
                holder: None,
            },
        ],
    );

    assert_eq!(
        assignments,
        vec![
            (
                Entity::from_raw(1),
                Some(DrivingTarget::EnemyFlag(Vec2::new(-400.0, 0.0)))
            ),
            (
                Entity::from_raw(2),
                Some(DrivingTarget::DefendHomeBase(Vec2::new(280.0, 0.0)))
            ),
            (
                Entity::from_raw(3),
                Some(DrivingTarget::MidfieldInterceptor(Vec2::ZERO))
            ),
            (
                Entity::from_raw(4),
                Some(DrivingTarget::EnemyFlagFlank(Vec2::new(-400.0, -220.0)))
            ),
        ]
    );
}

#[test]
fn closest_virtual_player_claims_shared_enemy_flag() {
    let mut app = app_with_system();
    let far_ai = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(0.0, 0.0, 4.0),
    );
    let close_ai = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(-300.0, 0.0, 4.0),
    );
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(-400.0, 0.0, 2.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(500.0, 0.0, 2.0),
        None,
    );

    app.update();

    let far_transform = app.world.get::<Transform>(far_ai).unwrap();
    let close_transform = app.world.get::<Transform>(close_ai).unwrap();

    assert!(
        far_transform.translation.x > 0.0,
        "far opponent should defend the red base, x={}",
        far_transform.translation.x
    );
    assert!(
        close_transform.translation.x < -300.0,
        "closest opponent should claim the blue flag, x={}",
        close_transform.translation.x
    );
}

#[test]
fn pickup_detour_still_reserves_enemy_flag_attack_role() {
    let mut app = app_with_system();
    let first_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    let second_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(-400.0, 0.0, 2.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(500.0, 0.0, 2.0),
        None,
    );
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Cash,
        },
        Transform::from_translation(Vec3::new(-100.0, 0.0, 2.0)),
    ));

    app.update();

    let first_transform = app.world.get::<Transform>(first_ai).unwrap();
    let second_transform = app.world.get::<Transform>(second_ai).unwrap();

    assert!(
        first_transform.translation.x < 0.0,
        "expected attacker to detour towards pickup on the flag lane, x={}",
        first_transform.translation.x
    );
    assert!(
        second_transform.translation.x > 0.0,
        "expected spare opponent to defend once attack lane is reserved, x={}",
        second_transform.translation.x
    );
}

#[test]
fn spare_defender_detours_for_pickup_on_home_lane() {
    let mut app = app_with_system();
    let first_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    let second_ai = spawn_ai(&mut app, vec![Vec2::new(0.0, 1000.0)]);
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(-400.0, 0.0, 2.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(500.0, 0.0, 2.0),
        None,
    );
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Cash,
        },
        Transform::from_translation(Vec3::new(200.0, 0.0, 2.0)),
    ));

    app.update();

    let first_transform = app.world.get::<Transform>(first_ai).unwrap();
    let second_transform = app.world.get::<Transform>(second_ai).unwrap();

    assert!(
        first_transform.translation.x < 0.0,
        "expected attacker to keep the blue flag role, x={}",
        first_transform.translation.x
    );
    assert!(
        second_transform.translation.x > 0.0,
        "expected spare defender to detour through the home-lane pickup, x={}",
        second_transform.translation.x
    );
}

#[test]
fn pricing_lifts_a_sabotage_for_the_team_whose_flag_is_stolen() {
    use crate::gameplay::virtual_player::ai::CTF_WIDE_DETOUR_MIN_PRIORITY;

    // Red's flag is being hauled off by an enemy. A flag is only ever held by
    // an enemy, so the same event makes Blue the carrier running it home: the
    // robbed team (Red) prices the sabotage to chase the thief, the carrier
    // team (Blue) to cover its own getaway, and the chase outranks the getaway.
    let flags = [
        FlagTarget {
            team: AiTeam::Red,
            home: Vec2::new(500.0, 0.0),
            position: Vec2::new(0.0, 0.0),
            holder: Some(Entity::from_raw(7)),
        },
        FlagTarget {
            team: AiTeam::Blue,
            home: Vec2::new(-500.0, 0.0),
            position: Vec2::new(-500.0, 0.0),
            holder: None,
        },
    ];
    let stolen = flag_stolen_state(&flags);
    assert!(stolen.for_team(AiTeam::Red), "an enemy holds Red's flag");
    assert!(
        !stolen.for_team(AiTeam::Blue),
        "Blue's own flag is safe at home"
    );

    let sabotage = ArenaPickup {
        position: Vec2::new(50.0, 0.0),
        kind: PickupKind::Sabotage,
    };
    let robbed = price_pickup_for_team(sabotage, None, AiTeam::Red, stolen).priority;
    let carrier = price_pickup_for_team(sabotage, None, AiTeam::Blue, stolen).priority;

    assert!(
        robbed >= CTF_WIDE_DETOUR_MIN_PRIORITY,
        "the robbed team must value the sabotage enough to chase the thief: {robbed}"
    );
    assert!(
        carrier >= CTF_WIDE_DETOUR_MIN_PRIORITY,
        "the carrier team must value the sabotage enough to cover its getaway: {carrier}"
    );
    assert!(
            robbed > carrier,
            "chasing the thief must still outrank covering our own run: robbed={robbed}, carrier={carrier}"
        );
}

#[test]
fn pricing_lifts_a_sabotage_for_the_team_carrying_the_enemy_flag() {
    use crate::gameplay::pickup::collect::SABOTAGE_FLAG_GETAWAY_VIRTUAL_PLAYER_PRIORITY;
    use crate::gameplay::virtual_player::ai::CTF_WIDE_DETOUR_MIN_PRIORITY;

    // Red hauls Blue's flag home; Red's own flag sits safe at base. So Red is
    // the carrier-team that values the sabotage as getaway cover, while Blue is
    // the robbed team that values it to chase the thief.
    let flags = [
        FlagTarget {
            team: AiTeam::Blue,
            home: Vec2::new(-500.0, 0.0),
            position: Vec2::new(0.0, 0.0),
            holder: Some(Entity::from_raw(7)),
        },
        FlagTarget {
            team: AiTeam::Red,
            home: Vec2::new(500.0, 0.0),
            position: Vec2::new(500.0, 0.0),
            holder: None,
        },
    ];
    let stolen = flag_stolen_state(&flags);
    assert!(stolen.for_team(AiTeam::Blue), "an enemy holds Blue's flag");
    assert!(!stolen.for_team(AiTeam::Red), "Red's flag is safe at home");

    let sabotage = ArenaPickup {
        position: Vec2::new(50.0, 0.0),
        kind: PickupKind::Sabotage,
    };
    let carrier_team = price_pickup_for_team(sabotage, None, AiTeam::Red, stolen).priority;
    let robbed_team = price_pickup_for_team(sabotage, None, AiTeam::Blue, stolen).priority;

    assert_eq!(
        carrier_team, SABOTAGE_FLAG_GETAWAY_VIRTUAL_PLAYER_PRIORITY,
        "the team running the enemy flag home prices the sabotage as getaway cover: {carrier_team}"
    );
    assert!(
        carrier_team >= CTF_WIDE_DETOUR_MIN_PRIORITY,
        "getaway cover must justify pulling an escort off a committed run: {carrier_team}"
    );
    assert!(
        carrier_team > PickupKind::Sabotage.virtual_player_priority(),
        "covering our own carrier must beat the flat sabotage value: {carrier_team}"
    );
    assert!(
        robbed_team > carrier_team,
        "defending the robbed team's own steal still outranks getaway cover: \
             robbed={robbed_team}, carrier={carrier_team}"
    );
}

/// Drives one frame with the lone Red defender facing straight down its
/// home-defence route (`-x`) while Red's flag is being carried off and the
/// round is in closing-time discipline, then reports the defender's `y`
/// drift. A pickup of `kind` sits off the route at `(50, 80)`: only a pickup
/// worth the wide closing-time detour pulls the defender off the line, so a
/// positive `y` means it broke off for the pickup.
fn disciplined_defender_detour_y(kind: PickupKind) -> f32 {
    use crate::gameplay::ctf::{MatchPhase, CLOSING_TIME_FRAMES};
    use std::f32::consts::FRAC_PI_2;

    let mut app = app_with_system();
    app.insert_resource(MatchClock {
        frames_remaining: CLOSING_TIME_FRAMES,
        phase: MatchPhase::Regulation,
    });

    let defender = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(200.0, 0.0, 4.0),
    );
    // Face the defender west, straight along its intercept route, so any +y
    // motion is a genuine detour and not steering slack off the spawn facing.
    app.world.get_mut::<Transform>(defender).unwrap().rotation = Quat::from_rotation_z(FRAC_PI_2);

    let carrier = spawn_player(&mut app, Vec3::new(0.0, 0.0, 5.0));
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(-500.0, 0.0, 2.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(0.0, 0.0, 2.0),
        Some(carrier),
    );
    app.world.spawn((
        Pickup { kind },
        Transform::from_translation(Vec3::new(50.0, 80.0, 2.0)),
    ));

    app.update();

    app.world.get::<Transform>(defender).unwrap().translation.y
}

#[test]
fn stolen_flag_pulls_a_disciplined_defender_onto_a_sabotage() {
    let sabotage_y = disciplined_defender_detour_y(PickupKind::Sabotage);
    let cash_y = disciplined_defender_detour_y(PickupKind::Cash);

    assert!(
            sabotage_y > 0.0,
            "a defender must break off onto the sabotage to slow the thief carrying its flag, y={sabotage_y}"
        );
    assert!(
        cash_y.abs() < 1e-3,
        "a disciplined defender must leave a cash bag and hold its intercept route, y={cash_y}"
    );
}

/// Drives one frame with a Red escort facing east along its escort route while
/// a Red carrier hauls the Blue flag home (Red's own flag safe) in closing-time
/// discipline, then reports the escort's `y` drift. A pickup of `kind` sits off
/// the route at `(300, 80)`: only a pickup worth the wide closing-time detour
/// pulls the escort off the line, so a positive `y` means it broke off for it.
/// A flat-priced sabotage stays on its narrow lane and is dropped in closing
/// time, so only the getaway-priced sabotage (our carrier is running) detours.
fn disciplined_escort_detour_y(kind: PickupKind) -> f32 {
    disciplined_escort_detour_y_with_integrity(kind, None)
}

fn disciplined_escort_detour_y_with_integrity(
    kind: PickupKind,
    integrity: Option<VehicleIntegrity>,
) -> f32 {
    use crate::gameplay::ctf::{MatchPhase, CLOSING_TIME_FRAMES};
    use std::f32::consts::FRAC_PI_2;

    let mut app = app_with_system();
    app.insert_resource(MatchClock {
        frames_remaining: CLOSING_TIME_FRAMES,
        phase: MatchPhase::Regulation,
    });
    if let Some(integrity) = integrity {
        app.insert_resource(integrity);
    }

    let escort = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(200.0, 0.0, 4.0),
    );
    // Face the escort east, straight along its escort route toward home, so any
    // +y motion is a genuine detour and not steering slack off the spawn facing.
    app.world.get_mut::<Transform>(escort).unwrap().rotation = Quat::from_rotation_z(-FRAC_PI_2);

    let carrier = spawn_ai_at(
        &mut app,
        vec![Vec2::new(0.0, 1000.0)],
        Vec3::new(400.0, 0.0, 4.0),
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(500.0, 0.0),
        Vec3::new(500.0, 0.0, 2.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(-500.0, 0.0),
        Vec3::new(400.0, 0.0, 2.0),
        Some(carrier),
    );
    app.world.spawn((
        Pickup { kind },
        Transform::from_translation(Vec3::new(300.0, 80.0, 2.0)),
    ));

    app.update();

    app.world.get::<Transform>(escort).unwrap().translation.y
}

#[test]
fn carried_flag_pulls_a_disciplined_escort_onto_a_sabotage() {
    let sabotage_y = disciplined_escort_detour_y(PickupKind::Sabotage);
    let cash_y = disciplined_escort_detour_y(PickupKind::Cash);

    assert!(
            sabotage_y > 0.0,
            "an escort must break off onto the sabotage to cover its carrier's run home, y={sabotage_y}"
        );
    assert!(
        cash_y.abs() < 1e-3,
        "a disciplined escort must leave a cash bag and hold its escort route, y={cash_y}"
    );
}

#[test]
fn carried_flag_pulls_a_disciplined_escort_onto_a_shield() {
    // The defensive mirror of the getaway sabotage: while a teammate runs the
    // enemy flag home (fragile, double ram bleed) a healthy escort-team would
    // normally leave a flat-priced shield, but the getaway lift makes it break
    // off to armour the run even under closing-time discipline.
    let shield_y = disciplined_escort_detour_y(PickupKind::Shield);
    let cash_y = disciplined_escort_detour_y(PickupKind::Cash);

    assert!(
        shield_y > 0.0,
        "an escort must break off onto the shield to armour its carrier's run home, y={shield_y}"
    );
    assert!(
        cash_y.abs() < 1e-3,
        "a disciplined escort must leave a cash bag and hold its escort route, y={cash_y}"
    );
}

#[test]
fn carried_flag_pulls_a_worn_disciplined_escort_onto_a_repair() {
    // The third leg of the getaway tripod: while a teammate runs the enemy flag
    // home, a worn escort-team tops up the integrity buffer the gauntlet will
    // burn. Unlike the shield/sabotage getaway lifts, a repair heals nothing on
    // a full team, so the team is held to half durability (0.5, above the
    // pit-retreat band) where a bare repair is worth only 110, below the
    // closing-time wide-detour bar. The getaway top-up lifts it over that bar
    // while a cash bag stays left.
    let worn = || {
        Some(VehicleIntegrity {
            player: 100.0,
            opponent: 50.0,
        })
    };
    let repair_y = disciplined_escort_detour_y_with_integrity(PickupKind::Repair, worn());
    let cash_y = disciplined_escort_detour_y_with_integrity(PickupKind::Cash, worn());

    assert!(
            repair_y > 0.0,
            "a worn escort must break off onto the repair to top up its carrier's run home, y={repair_y}"
        );
    assert!(
        cash_y.abs() < 1e-3,
        "a disciplined escort must leave a cash bag and hold its escort route, y={cash_y}"
    );
}

#[test]
fn nitro_boost_increases_virtual_player_distance() {
    let normal_y = one_frame_ai_y(AiTeam::Red, None);
    let boosted_y = one_frame_ai_y(AiTeam::Red, Some(NitroBoosts::trigger_opponent));

    assert!(
        boosted_y > normal_y,
        "normal={normal_y}, boosted={boosted_y}"
    );
}

#[test]
fn player_team_nitro_boosts_blue_virtual_players() {
    let normal_y = one_frame_ai_y(AiTeam::Blue, None);
    let boosted_y = one_frame_ai_y(AiTeam::Blue, Some(NitroBoosts::trigger_player));

    assert!(
        boosted_y > normal_y,
        "normal={normal_y}, boosted={boosted_y}"
    );
}

#[test]
fn opponent_nitro_does_not_boost_blue_virtual_players() {
    let normal_y = one_frame_ai_y(AiTeam::Blue, None);
    let opponent_boosted_y = one_frame_ai_y(AiTeam::Blue, Some(NitroBoosts::trigger_opponent));

    assert!(
        (opponent_boosted_y - normal_y).abs() < 1e-4,
        "normal={normal_y}, opponent_boosted={opponent_boosted_y}"
    );
}

fn one_frame_ai_y_with_integrity(team: AiTeam, integrity: VehicleIntegrity) -> f32 {
    let mut app = app_with_system();
    app.insert_resource(integrity);
    let ai = spawn_ai_on_team(&mut app, team, vec![Vec2::new(0.0, 1000.0)]);

    app.update();

    app.world.get::<Transform>(ai).unwrap().translation.y
}

#[test]
fn battered_integrity_reduces_opponent_distance() {
    let healthy_y = one_frame_ai_y(AiTeam::Red, None);
    let wrecked_y = one_frame_ai_y_with_integrity(
        AiTeam::Red,
        VehicleIntegrity {
            player: 100.0,
            opponent: 0.0,
        },
    );

    assert!(
        wrecked_y > 0.0 && wrecked_y < healthy_y,
        "healthy={healthy_y}, wrecked={wrecked_y}"
    );
}

fn one_frame_ai_y_with_stun(team: AiTeam, stuns: WreckStuns) -> f32 {
    let mut app = app_with_system();
    app.insert_resource(stuns);
    let ai = spawn_ai_on_team(&mut app, team, vec![Vec2::new(0.0, 1000.0)]);

    app.update();

    app.world.get::<Transform>(ai).unwrap().translation.y
}

#[test]
fn a_wreck_spin_out_reduces_opponent_distance() {
    let healthy_y = one_frame_ai_y(AiTeam::Red, None);
    let mut stuns = WreckStuns::default();
    stuns.trigger_opponent();
    let stunned_y = one_frame_ai_y_with_stun(AiTeam::Red, stuns);

    assert!(
        stunned_y > 0.0 && stunned_y < healthy_y,
        "a spun-out opponent should crawl forward: healthy={healthy_y}, stunned={stunned_y}"
    );
}

#[test]
fn an_opponent_spin_out_does_not_slow_blue_virtual_players() {
    let healthy_y = one_frame_ai_y(AiTeam::Blue, None);
    let mut stuns = WreckStuns::default();
    stuns.trigger_opponent();
    let blue_y = one_frame_ai_y_with_stun(AiTeam::Blue, stuns);

    assert!(
        (blue_y - healthy_y).abs() < 1e-4,
        "the opponents' spin-out must not slow blue cars: healthy={healthy_y}, blue={blue_y}"
    );
}

fn one_frame_ai_y_with_surge(team: AiTeam, surges: WreckSurges) -> f32 {
    let mut app = app_with_system();
    app.insert_resource(surges);
    let ai = spawn_ai_on_team(&mut app, team, vec![Vec2::new(0.0, 1000.0)]);

    app.update();

    app.world.get::<Transform>(ai).unwrap().translation.y
}

#[test]
fn a_fresh_kill_surge_increases_opponent_distance() {
    let healthy_y = one_frame_ai_y(AiTeam::Red, None);
    let mut surges = WreckSurges::default();
    surges.trigger_opponent();
    let surging_y = one_frame_ai_y_with_surge(AiTeam::Red, surges);

    assert!(
        surging_y > healthy_y,
        "a fresh-kill surge should speed an opponent up: healthy={healthy_y}, surging={surging_y}"
    );
}

#[test]
fn a_player_team_surge_does_not_speed_red_virtual_players() {
    let healthy_y = one_frame_ai_y(AiTeam::Red, None);
    let mut surges = WreckSurges::default();
    surges.trigger_player();
    let red_y = one_frame_ai_y_with_surge(AiTeam::Red, surges);

    assert!(
        (red_y - healthy_y).abs() < 1e-4,
        "the player team's surge must not speed red cars: healthy={healthy_y}, red={red_y}"
    );
}

#[test]
fn opponent_wear_does_not_slow_blue_virtual_players() {
    let healthy_y = one_frame_ai_y(AiTeam::Blue, None);
    let opponent_wrecked_y = one_frame_ai_y_with_integrity(
        AiTeam::Blue,
        VehicleIntegrity {
            player: 100.0,
            opponent: 0.0,
        },
    );

    assert!(
        (opponent_wrecked_y - healthy_y).abs() < 1e-4,
        "healthy={healthy_y}, opponent_wrecked={opponent_wrecked_y}"
    );
}

fn one_frame_ai_y_with_sabotage(team: AiTeam, effects: SabotageEffects) -> f32 {
    let mut app = app_with_system();
    app.insert_resource(effects);
    let ai = spawn_ai_on_team(&mut app, team, vec![Vec2::new(0.0, 1000.0)]);

    app.update();

    app.world.get::<Transform>(ai).unwrap().translation.y
}

#[test]
fn sabotaging_the_opponent_reduces_its_distance() {
    let healthy_y = one_frame_ai_y(AiTeam::Red, None);
    let mut effects = SabotageEffects::default();
    effects.sabotage_opponent();
    let sabotaged_y = one_frame_ai_y_with_sabotage(AiTeam::Red, effects);

    assert!(
        sabotaged_y > 0.0 && sabotaged_y < healthy_y,
        "a sabotaged opponent should crawl forward: healthy={healthy_y}, sabotaged={sabotaged_y}"
    );
}

#[test]
fn sabotaging_the_opponent_does_not_slow_blue_virtual_players() {
    let healthy_y = one_frame_ai_y(AiTeam::Blue, None);
    let mut effects = SabotageEffects::default();
    effects.sabotage_opponent();
    let blue_y = one_frame_ai_y_with_sabotage(AiTeam::Blue, effects);

    assert!(
        (blue_y - healthy_y).abs() < 1e-4,
        "sabotaging red must not slow blue cars: healthy={healthy_y}, blue={blue_y}"
    );
}

fn attacker_x_after_frames(integrity: Option<VehicleIntegrity>, frames: u32) -> f32 {
    let mut app = app_with_system();
    if let Some(integrity) = integrity {
        app.insert_resource(integrity);
    }
    // Red attacker facing +Y, enemy (blue) flag up the lane, own flag behind.
    let ai = spawn_ai_on_team(&mut app, AiTeam::Red, vec![Vec2::new(0.0, 1000.0)]);
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(0.0, -600.0),
        Vec3::new(0.0, -600.0, 1.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(0.0, 600.0),
        Vec3::new(0.0, 600.0, 1.0),
        None,
    );
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Repair,
        },
        Transform::from_translation(Vec3::new(80.0, 150.0, 2.0)),
    ));

    for _ in 0..frames {
        app.update();
    }

    app.world.get::<Transform>(ai).unwrap().translation.x
}

#[test]
fn battered_attacker_peels_off_for_a_repair_on_the_flag_lane() {
    let healthy_x = attacker_x_after_frames(None, 15);
    let wrecked_x = attacker_x_after_frames(
        Some(VehicleIntegrity {
            player: 100.0,
            opponent: 0.0,
        }),
        15,
    );

    assert!(
        healthy_x.abs() < 1.0,
        "a pristine attacker should hold the flag lane, x={healthy_x}"
    );
    assert!(
            wrecked_x > 5.0,
            "a wrecked attacker should peel off toward the repair, healthy={healthy_x}, wrecked={wrecked_x}"
        );
}

/// Mirror of [`attacker_x_after_frames`] for a Blue (player-team) attacker, so
/// repair pursuit can be checked against the attacker's *own* team wear.
fn blue_attacker_x_after_frames(integrity: Option<VehicleIntegrity>, frames: u32) -> f32 {
    let mut app = app_with_system();
    if let Some(integrity) = integrity {
        app.insert_resource(integrity);
    }
    // Blue attacker facing +Y, enemy (red) flag up the lane, own flag behind.
    let ai = spawn_ai_on_team(&mut app, AiTeam::Blue, vec![Vec2::new(0.0, 1000.0)]);
    spawn_flag(
        &mut app,
        FlagTeam::Blue,
        Vec2::new(0.0, -600.0),
        Vec3::new(0.0, -600.0, 1.0),
        None,
    );
    spawn_flag(
        &mut app,
        FlagTeam::Red,
        Vec2::new(0.0, 600.0),
        Vec3::new(0.0, 600.0, 1.0),
        None,
    );
    app.world.spawn((
        Pickup {
            kind: crate::gameplay::pickup::PickupKind::Repair,
        },
        Transform::from_translation(Vec3::new(80.0, 150.0, 2.0)),
    ));

    for _ in 0..frames {
        app.update();
    }

    app.world.get::<Transform>(ai).unwrap().translation.x
}

#[test]
fn healthy_attacker_holds_lane_when_only_the_enemy_is_wrecked() {
    // Blue is pristine, Red is wrecked. A repair is worthless to a full team
    // (durability is capped), so a healthy attacker must keep pushing the flag
    // rather than detour for a patch-up it cannot use: repairs are valued by
    // your OWN wear, never the enemy's.
    let healthy_x = blue_attacker_x_after_frames(
        Some(VehicleIntegrity {
            player: 100.0,
            opponent: 0.0,
        }),
        15,
    );

    assert!(
            healthy_x.abs() < 1.0,
            "a pristine attacker should hold the flag lane even when the enemy is wrecked, x={healthy_x}"
        );
}

#[test]
fn battered_blue_attacker_still_peels_off_for_a_repair() {
    // The mirror case: when the Blue attacker's own team is wrecked it must
    // chase the repair, proving per-team pricing scales repair pursuit for
    // either side.
    let wrecked_x = blue_attacker_x_after_frames(
        Some(VehicleIntegrity {
            player: 0.0,
            opponent: 100.0,
        }),
        15,
    );

    assert!(
        wrecked_x > 5.0,
        "a wrecked blue attacker should peel off toward the repair, x={wrecked_x}"
    );
}

/// One drive frame for a Red chaser with the given cornering commitment, the
/// human player sitting dead ahead at `player_distance`. Returns how far up the
/// +Y line the chaser advanced: zero means it idled (eased off its run-down),
/// positive means it kept closing.
fn run_down_advance_y(corner_throttle: f32, player_distance: f32) -> f32 {
    let mut app = app_with_system();
    spawn_player(&mut app, Vec3::new(0.0, player_distance, 4.0));
    let chaser = app
        .world
        .spawn((
            VirtualPlayer {
                team: AiTeam::Red,
                movement_speed: 500.0,
                rotation_speed: f32::to_radians(360.0),
                waypoints: Vec::new(),
                current_waypoint: 0,
                player_pursuit_radius: TEST_PURSUIT_RADIUS,
                pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                corner_throttle,
            },
            Transform::from_translation(Vec3::new(0.0, 0.0, 4.0)),
        ))
        .id();

    app.update();

    app.world.get::<Transform>(chaser).unwrap().translation.y
}

#[test]
fn the_all_rounder_runs_a_foe_down_at_the_baseline_depth() {
    assert!(
        (pursuit_arrive_radius(MIN_THROTTLE) - PURSUIT_ARRIVE_RADIUS).abs() <= f32::EPSILON,
        "a driver cornering on the neutral floor must run a foe down at the exact baseline"
    );
}

#[test]
fn a_reckless_driver_commits_a_deeper_run_down() {
    let reckless = pursuit_arrive_radius(0.45);
    let baseline = pursuit_arrive_radius(MIN_THROTTLE);
    let disciplined = pursuit_arrive_radius(0.20);
    assert!(
        reckless < baseline && baseline < disciplined,
        "commitment must deepen the run-down: reckless={reckless}, baseline={baseline}, \
             disciplined={disciplined}"
    );
}

#[test]
fn every_run_down_depth_stays_inside_ram_range() {
    // Across the whole personality commitment band the run-down depth stays a
    // positive distance well inside ram range, so even the shyest chaser is
    // still trading paint when it idles and the keenest never noses clean past
    // its foe.
    for throttle in [0.15, 0.20, 0.30, 0.38, 0.42, 0.45, 0.50] {
        let radius = pursuit_arrive_radius(throttle);
        assert!(
            radius > 0.0 && radius < RAM_RADIUS && radius < WAYPOINT_ARRIVE_RADIUS,
            "run-down radius {radius} for throttle {throttle} left the safe band"
        );
    }
}

#[test]
fn run_down_depth_is_clamped_for_out_of_band_commitment() {
    // A degenerate throttle can never invert the radius or send it past the
    // clamp, so even an absurd personality still idles on contact.
    assert!(
        (pursuit_arrive_radius(10.0) - PURSUIT_ARRIVE_RADIUS_MIN).abs() <= f32::EPSILON,
        "an extreme reckless throttle must clamp to the tightest run-down"
    );
    assert!(
        (pursuit_arrive_radius(-10.0) - PURSUIT_ARRIVE_RADIUS_MAX).abs() <= f32::EPSILON,
        "an extreme disciplined throttle must clamp to the shallowest run-down"
    );
}

#[test]
fn a_reckless_chaser_drives_deeper_than_a_disciplined_one_eases_off() {
    // The human sits at a distance that falls between the two drivers' run-down
    // depths: the reckless chaser is still inside its (tighter) arrive radius so
    // it keeps closing, while the disciplined one has already reached its (wider)
    // one and eases off. Same speed, same line; only commitment differs.
    let distance = 30.0;
    let reckless = run_down_advance_y(0.45, distance);
    let disciplined = run_down_advance_y(0.20, distance);
    assert!(
        reckless > 0.0,
        "a reckless chaser must keep closing at distance {distance}, advanced {reckless}"
    );
    assert!(
        disciplined.abs() <= f32::EPSILON,
        "a disciplined chaser must ease off at distance {distance}, advanced {disciplined}"
    );
}

#[test]
fn the_first_velocity_sample_is_a_neutral_zero() {
    // No previous position to difference against, so a freshly spawned (or
    // freshly respawned) human reads as stationary rather than as a teleport.
    assert_eq!(
        player_velocity_estimate(None, Vec2::new(123.0, -45.0)),
        Vec2::ZERO,
        "the first sample must estimate a neutral zero velocity"
    );
}

#[test]
fn velocity_is_the_position_delta_over_the_fixed_frame() {
    let previous = Vec2::new(10.0, -20.0);
    let current = Vec2::new(10.0 + 5.0, -20.0 - 3.0);
    let expected = Vec2::new(5.0, -3.0) / TIME_STEP;
    assert!(
        player_velocity_estimate(Some(previous), current).distance(expected) <= 1e-3,
        "velocity must be the per-frame delta divided by the fixed time step"
    );
}

#[test]
fn the_tracker_samples_then_differences_the_humans_movement() {
    // First frame: no previous sample, so the estimate is zero but the position
    // is banked. Second frame: the human has moved, so the estimate is the delta
    // over the fixed step. Driving the system twice mirrors the real loop.
    let mut app = App::new();
    app.init_resource::<PlayerVelocity>();
    app.add_system(track_player_velocity_system);
    let human = spawn_player(&mut app, Vec3::new(100.0, 200.0, 5.0));

    app.update();
    let after_first = *app.world.resource::<PlayerVelocity>();
    assert_eq!(
        after_first.velocity,
        Vec2::ZERO,
        "the first sample must read the human as stationary"
    );
    assert_eq!(after_first.previous_position, Some(Vec2::new(100.0, 200.0)));

    app.world.get_mut::<Transform>(human).unwrap().translation =
        Vec3::new(100.0 + 8.0, 200.0 - 2.0, 5.0);
    app.update();

    let after_second = *app.world.resource::<PlayerVelocity>();
    let expected = Vec2::new(8.0, -2.0) / TIME_STEP;
    assert!(
        after_second.velocity.distance(expected) <= 1e-3,
        "the second sample must be the movement delta over the frame: {:?}",
        after_second.velocity
    );
}

#[test]
fn the_tracker_resets_when_the_human_is_absent() {
    // A despawned human clears the previous position, so its return never reads
    // as one giant teleport-velocity spike.
    let mut app = App::new();
    app.insert_resource(PlayerVelocity {
        previous_position: Some(Vec2::new(50.0, 50.0)),
        velocity: Vec2::new(999.0, 999.0),
    });
    app.add_system(track_player_velocity_system);

    app.update();

    let tracker = *app.world.resource::<PlayerVelocity>();
    assert_eq!(
        tracker.velocity,
        Vec2::ZERO,
        "an absent human reads as still"
    );
    assert_eq!(
        tracker.previous_position, None,
        "an absent human must clear the previous sample"
    );
}

#[test]
fn a_defender_leads_a_juking_human_thief_to_the_ring_crossing() {
    // A red home defender already sitting on the static body-block point of its
    // home flag. A human thief sweeps in toward the flag, juking hard to one
    // side. Without a tracked human velocity the defender holds the body-block
    // (it is already there); once the human's velocity is known it shifts to cut
    // the thief off where it will actually breach the defensive ring, the same
    // lead it already runs against a virtual thief.
    //
    // Red home (and flag) sit at the origin, so the static block is 140 straight
    // up the human's bearing and the defender starts exactly there. The juking
    // human sits at (0, 300) inside the home-flag threat radius, sweeping
    // down-left, so its ring crossing lies off to negative x.
    fn defender_x(with_velocity: bool) -> f32 {
        let mut app = app_with_system();
        let defender = app
            .world
            .spawn((
                VirtualPlayer {
                    team: AiTeam::Red,
                    movement_speed: 500.0,
                    rotation_speed: f32::to_radians(360.0),
                    waypoints: vec![Vec2::new(0.0, 2000.0)],
                    current_waypoint: 0,
                    player_pursuit_radius: TEST_PURSUIT_RADIUS,
                    pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                    corner_throttle: 0.3,
                },
                Transform {
                    translation: Vec3::new(0.0, 140.0, 4.0),
                    rotation: Quat::from_rotation_z(std::f32::consts::PI),
                    ..default()
                },
            ))
            .id();
        spawn_flag(
            &mut app,
            FlagTeam::Red,
            Vec2::ZERO,
            Vec3::new(0.0, 0.0, 4.0),
            None,
        );
        spawn_flag(
            &mut app,
            FlagTeam::Blue,
            Vec2::new(0.0, 1000.0),
            Vec3::new(0.0, 1000.0, 4.0),
            None,
        );
        spawn_player(&mut app, Vec3::new(0.0, 300.0, 5.0));
        if with_velocity {
            app.insert_resource(PlayerVelocity {
                previous_position: Some(Vec2::new(0.0, 300.0)),
                velocity: Vec2::new(-150.0, -300.0),
            });
        }

        for _ in 0..30 {
            app.update();
        }

        app.world.get::<Transform>(defender).unwrap().translation.x
    }

    let held = defender_x(false);
    let led = defender_x(true);

    assert!(
        held.abs() < 1e-3,
        "with no tracked velocity the defender holds the body-block: {held}"
    );
    assert!(
        led < -1.0,
        "knowing the human's velocity, the defender shifts to cut off the ring crossing: {led}"
    );
}

#[test]
fn a_kill_press_heads_a_fleeing_human_off_at_the_pass() {
    // A healthy red hunter against a reeling human team. The human flees
    // laterally across the hunter's nose. Without a tracked human velocity the
    // hunter tail-chases the spot the human currently sits; once that velocity
    // is known it leads the human to where it is heading, the same interception
    // it already runs against a fleeing virtual prey.
    fn hunter_x(with_velocity: bool) -> f32 {
        let mut app = app_with_system();
        app.insert_resource(VehicleIntegrity {
            player: 20.0,
            opponent: MAX_INTEGRITY,
        });
        // The hunter at the origin already facing the prey (-Y), so the lateral
        // lead reads as a clean sideways drift rather than a U-turn.
        let hunter = app
            .world
            .spawn((
                VirtualPlayer {
                    team: AiTeam::Red,
                    movement_speed: 500.0,
                    rotation_speed: f32::to_radians(360.0),
                    waypoints: vec![Vec2::new(0.0, 2000.0)],
                    current_waypoint: 0,
                    player_pursuit_radius: TEST_PURSUIT_RADIUS,
                    pickup_pursuit_radius: TEST_PICKUP_PURSUIT_RADIUS,
                    corner_throttle: 0.3,
                },
                Transform {
                    translation: Vec3::new(0.0, 0.0, 4.0),
                    rotation: Quat::from_rotation_z(std::f32::consts::PI),
                    ..default()
                },
            ))
            .id();
        // A second red car keeps the team above the lone-hunter guard and sits
        // far from the prey so the origin car is the chosen hunter.
        spawn_ai_at(
            &mut app,
            vec![Vec2::new(0.0, 2000.0)],
            Vec3::new(0.0, 1500.0, 4.0),
        );
        // The reeling human prey, fleeing to the right across the hunter's nose.
        spawn_player(&mut app, Vec3::new(0.0, -300.0, 5.0));
        if with_velocity {
            app.insert_resource(PlayerVelocity {
                previous_position: Some(Vec2::new(0.0, -300.0)),
                velocity: Vec2::new(300.0, 0.0),
            });
        }

        for _ in 0..20 {
            app.update();
        }

        app.world.get::<Transform>(hunter).unwrap().translation.x
    }

    let tail_chased = hunter_x(false);
    let led = hunter_x(true);

    assert!(
        tail_chased.abs() < 1e-3,
        "with no tracked velocity the hunter tail-chases straight at the human: {tail_chased}"
    );
    assert!(
        led > 1.0,
        "knowing the human's velocity, the hunter leads it, veering to cut it off: {led}"
    );
}
