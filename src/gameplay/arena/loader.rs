use bevy::prelude::*;

/// Marker for the arena root entity spawned for the current match.
#[derive(Component)]
pub struct Arena;

/// Spawns the arena root marker when a match begins.
///
/// The arena scene itself, its sprites, colliders and flags, is chosen and loaded
/// earlier, during [`crate::AppState::Loading`], by [`super::scene_loader::load`]
/// (which rolls the rotation via [`super::selection::select_arena`]). This system
/// only marks the arena root for the in-game world; it does not re-load the scene,
/// so the choice the loader made is the one that plays.
pub fn setup(mut commands: Commands) {
    commands.spawn((Arena, Name::new("Arena")));
}
