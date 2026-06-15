//! Wall scrape: the classic Death Rally speed bleed a car suffers for grinding
//! the arena boundary instead of holding a clean racing line.
//!
//! Hug a wall and the contact scrubs your speed: a staple of the genre's feel and
//! a skill check that rewards keeping off the barriers. Modelled here as a small,
//! capped speed penalty a car carries while it sits inside a tight margin of an
//! arena wall, scaling from nothing at the edge of the margin to its full bite
//! flush against the boundary. The penalty is read by both movement systems, the
//! human's `car_movement_system` and the field's `virtual_player_drive_system`, so
//! the human and the AI scrape on the identical terms.
//!
//! The margin is deliberately tighter than the combat wall-crush band
//! ([`crate::gameplay::combat::WALL_CRUSH_MARGIN`]) and far inside the patrol
//! clearance every car normally keeps, so interior driving and base play are
//! untouched: only a car genuinely jammed against the boundary bleeds speed. That
//! also dovetails with the wall-crush press the AI already sets up
//! ([`crate::gameplay::virtual_player::ai::finish_off_wall_crush_aim`]): a victim
//! pinned flush against the wall both eats the crush damage and scrubs speed, so it
//! cannot peel off the boundary to escape, while the hunter charging in from the
//! open side stays clear of the margin.

use bevy::prelude::*;

/// Largest fraction of its speed a car loses while grinding flush against a wall.
///
/// A meaningful bleed, on the order of an engine sabotage's slow
/// ([`crate::gameplay::pickup::SABOTAGE_SPEED_MULTIPLIER`]), so hugging a barrier
/// visibly costs pace, yet never a stop: even pinned flush the car still rolls.
/// The penalty scales down with distance from the wall (see
/// [`wall_scrape_speed_multiplier`]), so this top rate is reached only flush
/// against the boundary.
pub const WALL_SCRAPE_MAX_SPEED_PENALTY: f32 = 0.15;

/// A scrape must be a real bleed yet never a stop, enforced at compile time so the
/// penalty can never drift into pinning a car motionless against the wall.
const _: () = assert!(WALL_SCRAPE_MAX_SPEED_PENALTY > 0.0 && WALL_SCRAPE_MAX_SPEED_PENALTY < 0.5);

/// A scrape must stay milder than limping on near-wrecked integrity, enforced at
/// compile time, so grinding a wall costs less pace than a battered engine and the
/// speed-penalty ordering stays coherent.
const _: () = assert!(
    1.0 - WALL_SCRAPE_MAX_SPEED_PENALTY > crate::gameplay::combat::MIN_INTEGRITY_SPEED_MULTIPLIER
);

/// How close to a wall a car must sit before the scrape begins to bite.
///
/// Kept tight, a clear step inside the combat wall-crush band
/// ([`crate::gameplay::combat::WALL_CRUSH_MARGIN`]) and far inside the clearance a
/// car keeps on its normal racing and patrol lines, so only a car truly jammed
/// against the boundary scrubs speed. A car merely near the wall, defending a base
/// or cutting a corner, keeps its full pace.
pub const WALL_SCRAPE_MARGIN: f32 = 60.0;

/// The scrape band must be a real but narrow corridor sitting inside the wall-crush
/// band, enforced at compile time, so a car is crush-vulnerable before it ever
/// scrapes and normal interior driving never bleeds speed.
const _: () = assert!(
    WALL_SCRAPE_MARGIN > 0.0 && WALL_SCRAPE_MARGIN < crate::gameplay::combat::WALL_CRUSH_MARGIN
);

/// Speed multiplier a car at `position` carries from scraping the nearest arena
/// wall, given the arena's `half_extents` (half its width and height, so the walls
/// lie at `±half_extents.x` and `±half_extents.y`).
///
/// Returns `1.0` (no scrape) unless the car sits within [`WALL_SCRAPE_MARGIN`] of a
/// wall on either axis. Inside the margin the penalty scales linearly with how deep
/// into it the car has pressed: nothing at the edge of the margin, the full
/// [`WALL_SCRAPE_MAX_SPEED_PENALTY`] flush against the boundary. A car wedged in a
/// corner scrapes the worse of its two axes (never more than the cap), so the
/// result is always in `1.0 - WALL_SCRAPE_MAX_SPEED_PENALTY ..= 1.0`.
///
/// The caller passes the car's current position (already confined within the
/// arena) and the arena half-extents, so the same reading drives the human and the
/// AI alike.
#[must_use]
pub fn wall_scrape_speed_multiplier(position: Vec2, half_extents: Vec2) -> f32 {
    let proximity =
        wall_proximity(position.x, half_extents.x).max(wall_proximity(position.y, half_extents.y));
    WALL_SCRAPE_MAX_SPEED_PENALTY.mul_add(-proximity, 1.0)
}

/// How deeply (`0.0`..=`1.0`) a coordinate has pressed into the scrape margin of
/// its wall: `0.0` at or beyond [`WALL_SCRAPE_MARGIN`] from the wall, rising to
/// `1.0` flush against (or, defensively, past) the boundary at `±half`.
fn wall_proximity(coordinate: f32, half: f32) -> f32 {
    let distance_to_wall = (half - coordinate.abs()).max(0.0);
    if distance_to_wall >= WALL_SCRAPE_MARGIN {
        0.0
    } else {
        (1.0 - distance_to_wall / WALL_SCRAPE_MARGIN).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A roomy arena whose walls sit well clear of the origin, matching the feel of
    /// the real bounds (the centre is far from every wall).
    const HALF_EXTENTS: Vec2 = Vec2::new(1000.0, 600.0);

    fn assert_near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= f32::EPSILON,
            "actual={actual}, expected={expected}"
        );
    }

    /// The scrape multiplier for a car sitting `distance_from_wall` units inside the
    /// +X wall, the axis every proximity assertion probes.
    fn scrape_at_distance(distance_from_wall: f32) -> f32 {
        let position = Vec2::new(HALF_EXTENTS.x - distance_from_wall, 0.0);
        wall_scrape_speed_multiplier(position, HALF_EXTENTS)
    }

    #[test]
    fn a_car_in_the_open_keeps_full_speed() {
        assert_near(wall_scrape_speed_multiplier(Vec2::ZERO, HALF_EXTENTS), 1.0);
    }

    #[test]
    fn a_car_just_outside_the_margin_keeps_full_speed() {
        // One unit clear of the scrape margin on the +X wall: no bleed yet.
        assert_near(scrape_at_distance(WALL_SCRAPE_MARGIN + 1.0), 1.0);
    }

    #[test]
    fn a_car_flush_against_a_wall_takes_the_full_penalty() {
        assert_near(scrape_at_distance(0.0), 1.0 - WALL_SCRAPE_MAX_SPEED_PENALTY);
    }

    #[test]
    fn the_scrape_bites_harder_the_closer_to_the_wall() {
        let shallow = scrape_at_distance(WALL_SCRAPE_MARGIN * 0.75);
        let deep = scrape_at_distance(WALL_SCRAPE_MARGIN * 0.25);
        assert!(
            deep < shallow && shallow < 1.0,
            "a deeper press should scrub more: shallow={shallow}, deep={deep}"
        );
    }

    #[test]
    fn the_scrape_fades_smoothly_to_nothing_at_the_margin_edge() {
        let just_inside = scrape_at_distance(WALL_SCRAPE_MARGIN - 1.0);
        let just_outside = scrape_at_distance(WALL_SCRAPE_MARGIN + 1.0);
        assert_near(just_outside, 1.0);
        assert!(
            just_inside < just_outside && just_outside - just_inside < 0.01,
            "the scrape must fade to nothing at the margin edge, no cliff: \
             inside={just_inside}, outside={just_outside}"
        );
    }

    #[test]
    fn the_far_wall_is_scraped_just_like_the_near_one() {
        let near = wall_scrape_speed_multiplier(Vec2::new(HALF_EXTENTS.x, 0.0), HALF_EXTENTS);
        let far = wall_scrape_speed_multiplier(Vec2::new(-HALF_EXTENTS.x, 0.0), HALF_EXTENTS);
        assert_near(near, far);
    }

    #[test]
    fn a_corner_scrapes_the_worse_of_its_two_axes() {
        // Pressed flush on +Y but only halfway into the +X margin: the worse axis
        // (the flush +Y wall) sets the penalty, and a corner never exceeds the cap.
        let half_margin = WALL_SCRAPE_MARGIN * 0.5;
        let position = Vec2::new(HALF_EXTENTS.x - half_margin, HALF_EXTENTS.y);
        assert_near(
            wall_scrape_speed_multiplier(position, HALF_EXTENTS),
            1.0 - WALL_SCRAPE_MAX_SPEED_PENALTY,
        );
    }

    #[test]
    fn a_car_past_the_wall_is_clamped_to_the_full_penalty() {
        // Defensive: a position nudged past the boundary still scrapes at the cap,
        // never beyond it, so the multiplier can never run below its floor.
        let position = Vec2::new(HALF_EXTENTS.x + 50.0, 0.0);
        assert_near(
            wall_scrape_speed_multiplier(position, HALF_EXTENTS),
            1.0 - WALL_SCRAPE_MAX_SPEED_PENALTY,
        );
    }
}
