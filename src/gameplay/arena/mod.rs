use crate::gameplay::arena::scene::Scene;
use crate::gameplay::arena::scene_loader::SceneLoader;

use crate::{App, AppState, Plugin};
use bevy::prelude::*;

use bevy_common_assets::json::JsonAssetPlugin;

use scene_loader::SceneState;

mod loader;
mod music;
mod scene;
mod scene_loader;

#[derive(Default)]
pub struct ArenaPlugin;

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        // Always
        app.add_plugin(JsonAssetPlugin::<Scene>::new(&["2dtf"]));

        // Pre loading
        app.init_resource::<SceneState>()
            .add_asset::<Scene>()
            .init_asset_loader::<SceneLoader>();

        // Loading
        app.add_system_set(
            SystemSet::on_enter(AppState::Loading)
                .with_system(|| info!("Enter: Loading"))
                .with_system(scene_loader::load),
        );
        app.add_system_set(
            SystemSet::on_update(AppState::Loading)
                .with_system(scene_loader::load_sprites_from_scene)
                .with_system(scene_loader::move_to_next_state),
        );
        app.add_system_set(
            SystemSet::on_exit(AppState::Loading).with_system(|| info!("Exit: Loading")),
        );

        // Enter Gameplay (does not work with the current hierarchy)
        app.add_system_set(
            SystemSet::on_enter(AppState::InGame)
                .with_system(entering_in_game)
                .with_system(loader::setup)
                .with_system(music::setup),
        );

        // Every frame
        app.add_system_set(SystemSet::on_update(AppState::InGame).with_system(debug));

        // Exit Gameplay
        // app.add_system_set(SystemSet::on_exit(AppState::InGame));
    }
}

const fn debug() {}

fn entering_in_game() {
    info!("Entering: InGame");
}
