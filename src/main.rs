#![allow(dead_code, unused_variables, unused_imports)]
use bevy::prelude::*;
use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_inspector_egui::WorldInspectorPlugin;
use bevy_kira_audio::prelude::*;
use gameplay::GameplayPlugins;
use iyes_loopless::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AppState {
    MainMenu,
    // Career,
    Loading,
    InGame,
}

mod core;
mod gameplay;

fn main() {
    core::init();
    App::new()
        // Core setup
        .insert_resource(WindowDescriptor {
            width: 1400.0,
            height: 800.0,
            title: "Death Arena".to_string(),
            canvas: Some("#game".to_owned()),
            fit_canvas_to_parent: true,
            ..default()
        })
        // State transitions
        .add_loading_state(
            LoadingState::new(AppState::Loading).continue_to_state(AppState::InGame), // .with_collection(),
        )
        // App states
        // .add_loopless_state(AppState::MainMenu)
        .add_state(AppState::Loading)
        // .add_state(AppState::InGame)
        // Plugins
        .add_plugins(DefaultPlugins)
        .add_plugins(GameplayPlugins)
        .add_plugin(WorldInspectorPlugin::new())
        .run();
}

fn enter_game(mut app_state: ResMut<State<AppState>>) {
    app_state.set(AppState::InGame).unwrap();
    // ^ this can fail if we are already in the target state
    // or if another state change is already queued
}
