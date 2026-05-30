use crate::{App, Plugin};
use bevy::prelude::*;

mod camera;
mod car;
mod movement;
mod sfx;

#[cfg(test)]
mod movement_tests;

#[derive(Component)]
pub struct Player {
    /// linear speed in meters per second
    pub movement_speed: f32,
    /// rotation speed in radians per second
    pub rotation_speed: f32,
    /// engine max speed multiplier (0.5 to 1.0)
    pub engine_max_speed_multiplier: f32,
    /// forward max speed base
    pub forward_max_speed_base: f32,
    /// backward max speed base
    pub backward_max_speed_base: f32,
    /// wheels turning multiplier
    pub wheels_turning_multiplier: f32,
}

#[derive(Default)]
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(camera::setup)
            .add_startup_system(car::setup)
            .add_startup_system(sfx::setup)
            .add_system_set(
                SystemSet::new()
                    .with_system(movement::car_movement_system)
                    .with_system(camera::camera_follows_player_system)
                    .with_system(sfx::engine_revving_system),
            )
            .add_system(bevy::window::close_on_esc);
    }
}
