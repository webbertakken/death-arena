use crate::{App, Plugin};
use bevy::prelude::*;

pub mod ai;
mod drive;
mod spawn;

/// An AI-controlled opponent car that patrols the arena.
///
/// Driving stats mirror [`crate::gameplay::player::Player`] so opponents feel
/// like the human car; the brain lives in [`ai`] and is applied by the drive
/// system.
#[derive(Component)]
pub struct VirtualPlayer {
    /// Capture-the-flag team this virtual player belongs to.
    pub team: ai::AiTeam,
    /// Linear speed in metres per second.
    pub movement_speed: f32,
    /// Rotation speed in radians per second.
    pub rotation_speed: f32,
    /// Cyclic patrol route in world space.
    pub waypoints: Vec<Vec2>,
    /// Index into `waypoints` the car is currently driving towards.
    pub current_waypoint: usize,
    /// World-space radius within which this driver peels off to hunt the human
    /// player. Set from the car's driving personality so each opponent hunts with
    /// its own eagerness: an aggressive sprinter runs the player down from further
    /// out than a disciplined technician that stays glued to its line.
    pub player_pursuit_radius: f32,
}

#[derive(Default)]
pub struct VirtualPlayerPlugin;

impl Plugin for VirtualPlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(spawn::setup)
            .add_system(drive::virtual_player_drive_system);
    }
}
