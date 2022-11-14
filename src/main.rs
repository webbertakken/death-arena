#![allow(unused_variables)]
#![warn(clippy::nursery, clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::needless_pass_by_value,
    clippy::only_used_in_recursion
)]
use crate::menu::{MenuPlugins, MenuState};
use app::{init::InitPlugin, AppPlugins};
use bevy::prelude::*;

use crate::app::init::default_plugins::Configure;
// use bevy_inspector_egui::WorldInspectorPlugin;
use gameplay::GameplayPlugins;

pub mod app;
pub mod core;
mod gameplay;
mod menu;
pub mod ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppState {
    Menus,
    Loading,
    InGame,
}

fn main() {
    core::init();

    // Setup
    let mut game = App::new();
    game.add_plugin(InitPlugin);
    game.add_plugins(DefaultPlugins::configure());

    // State
    if cfg!(debug_assertions) {
        // Development
        game.add_state(AppState::Loading);
        game.add_state(MenuState::Hidden);
    } else {
        // Production
        game.add_state(AppState::Menus);
        game.add_state(MenuState::Main);
    }

    // Logic
    game.add_plugins(AppPlugins);
    game.add_plugins(MenuPlugins);
    game.add_plugins(GameplayPlugins);

    // Misc
    // game.add_plugin(WorldInspectorPlugin::new());

    // Run the app
    game.run();
}
