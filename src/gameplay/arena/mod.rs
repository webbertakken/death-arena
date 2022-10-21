use crate::{App, Input, KeyCode, Plugin, Query, Res, Transform, Vec3};
use bevy::prelude::*;
use bevy::{math::Vec3Swizzles, time::FixedTimestep};
use bevy_kira_audio::prelude::*;
use std::time::Duration;

use crate::gameplay::main::{BOUNDS, TIME_STEP};
use crate::gameplay::player;
use crate::gameplay::player::Player;
mod loader;
mod music;
mod objects;

#[derive(Default)]
pub struct ArenaPlugin;

#[derive(Component)]
pub struct SnapToPlayer;

#[derive(Component)]
pub struct RotateToPlayer {
    /// rotation speed in radians per second
    rotation_speed: f32,
}

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(loader::setup)
            .add_startup_system(objects::setup)
            .add_startup_system(music::setup)
            .add_system_set(
                SystemSet::new()
                    .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                    .with_system(objects::snap_to_player_system)
                    .with_system(objects::rotate_to_player_system),
            );
    }
}
