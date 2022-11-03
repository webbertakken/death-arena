#![allow(dead_code, unused_variables, unused_imports)]
use crate::menu::MenuPlugins;
use app::{init::InitPlugin, AppPlugins};
use bevy::prelude::*;
use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_inspector_egui::WorldInspectorPlugin;
use bevy_kira_audio::prelude::*;
use gameplay::GameplayPlugins;
use iyes_loopless::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AppState {
    Menus,
    Loading,
    InGame,
}

mod app;
mod core;
mod gameplay;
mod menu;

fn main() {
    core::init();

    // Setup
    let mut game = App::new();
    game.add_plugin(InitPlugin);
    game.add_plugins(DefaultPlugins);

    // State
    game.add_loopless_state(AppState::Menus);

    // Logic
    game.add_plugins(AppPlugins);
    game.add_plugins(MenuPlugins);
    game.add_plugins(GameplayPlugins);

    // Misc
    game.add_plugin(WorldInspectorPlugin::new());

    // Run the app
    game.run();
}

fn enter_game(mut app_state: ResMut<State<AppState>>) {
    app_state.set(AppState::InGame).unwrap();
    // ^ this can fail if we are already in the target state
    // or if another state change is already queued
}
