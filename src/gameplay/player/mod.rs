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

#[derive(Resource)]
pub struct SpawnTimer(Timer);

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SpawnTimer(Timer::from_seconds(2.0, TimerMode::Repeating)))
            .add_systems(
                Startup,
                (camera::setup, setup, car::setup, sfx::setup),
            )
            .add_systems(
                Update,
                (
                    movement::car_movement_system,
                    camera::camera_follows_player_system,
                    sfx::engine_revving_system,
                ),
            );
    }
}

pub fn setup(commands: Commands, asset_server: Res<AssetServer>) {}
