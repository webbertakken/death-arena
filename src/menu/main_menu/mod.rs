use crate::{App, Input, KeyCode, Plugin, Query, Res, Transform, Vec3};
use bevy::prelude::*;
use bevy_kira_audio::prelude::*;

#[derive(Default)]
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(setup);
        app.add_system(update);
    }
}

pub fn setup(commands: Commands, asset_server: Res<AssetServer>) {
    log::info!("Setup: Main menu");
}

pub fn update(commands: Commands, asset_server: Res<AssetServer>) {
    // log::info!("Hello, update!");
}
