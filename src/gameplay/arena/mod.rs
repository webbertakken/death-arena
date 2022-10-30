use crate::gameplay::arena::scene::Scene;
use crate::gameplay::main::{BOUNDS, TIME_STEP};
use crate::gameplay::player::Player;
use crate::gameplay::{player, GameplayPlugins};
use crate::{App, AppState, Input, KeyCode, Plugin, Query, Res, Transform, Vec3};
use bevy::prelude::*;
use bevy::{math::Vec3Swizzles, time::FixedTimestep};
use bevy_asset_loader::prelude::*;
use bevy_common_assets::json::JsonAssetPlugin;
use bevy_kira_audio::prelude::*;
use std::time::Duration;

mod loader;
mod music;
mod objects;
mod scene;

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
        // Always
        app.add_plugin(JsonAssetPlugin::<Scene>::new(&["2dtf"]))
            .add_startup_system(loader::setup)
            .add_startup_system(objects::setup)
            .add_startup_system(music::setup);

        // Enter Gameplay (does not work with the current hierarchy)
        app.add_system_set(SystemSet::on_enter(AppState::InGame));

        // Every frame
        app.add_system_set(
            SystemSet::on_update(AppState::InGame)
                .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                .with_system(objects::snap_to_player_system)
                .with_system(objects::rotate_to_player_system),
        );

        // Exit Gameplay
        app.add_system_set(SystemSet::on_exit(AppState::InGame));
    }
}
