use crate::{App, Plugin, Res};
use bevy::prelude::*;

mod camera;
mod car;
mod movement;
mod sfx;

#[derive(Component)]
pub struct Player {
    /// linear speed in meters per second
    movement_speed: f32,
    /// rotation speed in radians per second
    rotation_speed: f32,
}

#[derive(Default)]
pub struct PlayerPlugin;

pub struct SpawnTimer(Timer);

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SpawnTimer(Timer::from_seconds(2.0, true)))
            .add_startup_system(camera::setup)
            .add_startup_system(setup)
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

pub const fn setup(commands: Commands, asset_server: Res<AssetServer>) {}
