use crate::gameplay::arena::scene::Scene;
use crate::gameplay::arena::scene_loader::SceneLoader;

use crate::{App, AppState, Plugin};
use bevy::prelude::*;

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
        app.add_plugins(JsonAssetPlugin::<Scene>::new(&["2dtf"]));
        app.add_systems(
            FixedUpdate,
            (
                objects::snap_to_player_system,
                objects::rotate_to_player_system,
            ),
        );

        // Pre loading
        app.init_resource::<SceneState>()
            .init_asset::<Scene>()
            .init_asset_loader::<SceneLoader>();

        // Loading
        app.add_systems(
            OnEnter(AppState::Loading),
            (|| info!("Enter: Loading"), scene_loader::load),
        );
        app.add_systems(
            Update,
            (
                scene_loader::load_sprites_from_scene,
                scene_loader::move_to_next_state,
            )
                .run_if(in_state(AppState::Loading)),
        );
        app.add_systems(OnExit(AppState::Loading), || info!("Exit: Loading"));

        // Enter Gameplay (does not work with the current hierarchy)
        app.add_systems(
            OnEnter(AppState::InGame),
            (
                entering_in_game,
                loader::setup,
                objects::setup,
                music::setup,
            ),
        );

        // Every frame
        app.add_systems(Update, debug.run_if(in_state(AppState::InGame)));
    }
}

fn debug() {}

fn entering_in_game() {
    info!("Entering: InGame");
}
