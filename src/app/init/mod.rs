use bevy::app::{App, Plugin};
use bevy::asset::AssetServer;
use bevy::prelude::*;

pub mod default_plugins;

#[derive(Default)]
pub struct InitPlugin;

impl Plugin for InitPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(background_color())
            .add_startup_system(setup)
            .add_startup_system(|| info!("Initialised."));
    }
}

const fn background_color() -> ClearColor {
    ClearColor(Color::rgb(0.1, 0.1, 0.27))
}

const fn setup(commands: Commands, asset_server: Res<AssetServer>) {}
