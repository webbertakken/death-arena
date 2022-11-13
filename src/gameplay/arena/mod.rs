use crate::gameplay::arena::scene::Scene;
use crate::gameplay::arena::scene_loader::SceneLoader;
use crate::gameplay::main::TIME_STEP;

use crate::{App, AppState, Plugin};
use bevy::prelude::*;
use bevy::time::FixedTimestep;

use bevy_common_assets::json::JsonAssetPlugin;

use scene_loader::SceneState;

mod loader;
mod music;
mod objects;
mod scene;
mod scene_loader;

#[derive(Default)]
pub struct ArenaPlugin;

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
                .with_system(objects::setup)
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
