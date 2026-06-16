//! Arena rotation: the single source of truth for which arenas a match can load.
//!
//! Death Arena spins up a fresh match on a randomly chosen arena, the classic
//! arcade "next track" roll. That roll lives here so there is exactly one place
//! that decides the playable arenas: [`super::scene_loader::load`] reads it the
//! moment a match starts loading, picks an arena with [`select_arena`], and loads
//! that scene. The list is deliberately a private-to-module constant rather than
//! scattered string literals, so adding an arena is a single edit here and the
//! choice provably reaches the loader instead of being silently ignored.
//!
//! Only validated arenas belong in [`ARENAS`]: a malformed scene file (one the
//! [`Scene`](super::scene::Scene) schema cannot deserialise) would not crash the
//! build, it would hand the loader a handle that never resolves and ship an empty
//! arena to the live demo. The `every_listed_arena_asset_exists_and_parses` test
//! guards that by deserialising every listed arena, and `scripts/
//! check_asset_paths.sh` independently asserts each path is a file that ships in
//! `assets/`.

/// Every arena scene a match can be played on, as Bevy asset paths under
/// `assets/`.
///
/// The rotation the match-start roll draws from. A single, validated entry today;
/// adding an arena is one line here and it immediately joins the rotation, because
/// [`super::scene_loader::load`] selects from exactly this list (no second,
/// hand-synced copy can drift out of step with it).
pub const ARENAS: [&str; 1] = ["textures/church-ctf.2dtf"];

/// The rotation must never be empty, so [`select_arena`]'s modulo can never divide
/// by zero, enforced at compile time.
const _: () = assert!(!ARENAS.is_empty());

/// Picks the arena a match loads from [`ARENAS`], wrapping `roll` across the list.
///
/// `roll` is the raw random draw made at match start; reducing it modulo the
/// rotation length keeps the pick in bounds for any value, so the caller never has
/// to know how many arenas there are. With a single arena every roll resolves to
/// it; the wrap only starts choosing once the rotation grows.
#[must_use]
pub const fn select_arena(roll: usize) -> &'static str {
    ARENAS[roll % ARENAS.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::arena::scene::Scene;

    #[test]
    fn the_rotation_is_never_empty() {
        // The match-start roll divides by this length; an empty rotation would
        // panic at runtime, so the rotation must always offer at least one arena.
        assert!(!ARENAS.is_empty());
    }

    #[test]
    fn every_arena_targets_a_scene_file() {
        for path in ARENAS {
            let extension = std::path::Path::new(path).extension();
            assert!(
                extension.is_some_and(|ext| ext.eq_ignore_ascii_case("2dtf")),
                "arena path {path} is not a .2dtf scene file"
            );
        }
    }

    #[test]
    fn selection_stays_within_the_rotation_for_any_roll() {
        // Any raw random draw, however large, must resolve to a listed arena
        // rather than indexing out of bounds.
        for roll in 0..=ARENAS.len() * 4 {
            assert!(
                ARENAS.contains(&select_arena(roll)),
                "roll {roll} resolved outside the rotation"
            );
        }
        assert!(ARENAS.contains(&select_arena(usize::MAX)));
    }

    #[test]
    fn selection_indexes_the_rotation_by_modulo() {
        for roll in [0_usize, 1, 2, 7, 100, usize::MAX] {
            assert_eq!(select_arena(roll), ARENAS[roll % ARENAS.len()]);
        }
    }

    #[test]
    fn every_listed_arena_asset_exists_and_parses() {
        // The rotation must only list arenas the loader can actually deserialise:
        // a malformed scene ships an empty arena rather than crashing, so prove
        // each listed file is on disk and parses as a Scene exactly as the loader
        // does at runtime.
        for path in ARENAS {
            let full = format!("{}/assets/{path}", env!("CARGO_MANIFEST_DIR"));
            let bytes = std::fs::read(&full)
                .unwrap_or_else(|error| panic!("arena asset {full} must exist: {error}"));
            let _scene: Scene = serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                panic!("arena asset {full} must parse as a Scene: {error}")
            });
        }
    }
}
