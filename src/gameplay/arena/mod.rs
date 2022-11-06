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
        app.add_plugin(JsonAssetPlugin::<Scene>::new(&["2dtf"]));

        app.add_system_set(
            SystemSet::new()
                .with_run_criteria(FixedTimestep::step(f64::from(TIME_STEP)))
                .with_system(objects::snap_to_player_system)
                .with_system(objects::rotate_to_player_system),
        );

        // Enter Gameplay (does not work with the current hierarchy)
        app.add_system_set(
            SystemSet::on_enter(AppState::InGame)
                .with_system(entering_in_game)
                .with_system(loader::setup)
                .with_system(objects::setup)
                .with_system(music::setup),
        );

        // Every frame
        app.add_system_set(SystemSet::on_update(AppState::InGame).with_system(debug));

        // Exit Gameplay
        // app.add_system_set(SystemSet::on_exit(AppState::InGame));
    }
}

fn debug() {
    info!("ingame update");
    // info!("Scene: {:?}", scene);
}

fn entering_in_game() {
    info!("Entering: InGame");
}
