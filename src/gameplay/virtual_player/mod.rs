use crate::{App, Plugin};
use bevy::prelude::*;

pub mod ai;
mod discipline;
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
    /// World-space radius within which this driver breaks off to scavenge a
    /// trackside pickup. Set from the car's driving personality so each opponent
    /// plays the pickup contest with its own greed: an impulsive sprinter detours
    /// for loot from further out than a disciplined technician that stays on its
    /// line. A behavioural trait, not a power stat: a wider radius scavenges more
    /// eagerly but abandons the patrol lap and the flag objective sooner, so it
    /// trades discipline for greed rather than being a strict upgrade.
    pub pickup_pursuit_radius: f32,
    /// Throttle floor this driver keeps when the target is off to the side, i.e.
    /// how hard it stays on the gas through a corner. Set from the car's driving
    /// personality so each opponent takes a turn with its own commitment: a
    /// reckless sprinter barrels through on a wide line, a disciplined technician
    /// eases off for a tight one. A behavioural trait, not a power stat: more
    /// corner throttle covers ground faster but sweeps a wider arc that overshoots
    /// the apex, so it trades line precision for corner speed rather than being a
    /// strict upgrade.
    pub corner_throttle: f32,
}

#[derive(Default)]
pub struct VirtualPlayerPlugin;

impl Plugin for VirtualPlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<drive::PlayerVelocity>()
            .add_startup_system(spawn::setup)
            .add_system(
                drive::track_player_velocity_system.before(drive::virtual_player_drive_system),
            )
            .add_system(drive::virtual_player_drive_system);
    }
}
