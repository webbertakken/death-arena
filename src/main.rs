#![allow(dead_code, unused_variables, unused_imports)]
#![warn(clippy::nursery, clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::unused_self,
    clippy::needless_pass_by_value
)]
use crate::menu::{MenuPlugins, MenuState};
use app::{init::InitPlugin, AppPlugins};
use bevy::prelude::*;
use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_inspector_egui::WorldInspectorPlugin;
use bevy_kira_audio::prelude::*;
use gameplay::GameplayPlugins;
use iyes_loopless::prelude::*;

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
    game.add_plugins(DefaultPlugins);

    // State
    game.add_state(AppState::Menus);
    game.add_state(MenuState::Main);

    // Logic
    game.add_plugins(AppPlugins);
    game.add_plugins(MenuPlugins);
    game.add_plugins(GameplayPlugins);

    // Misc
    game.add_plugin(WorldInspectorPlugin::new());

    // Run the app
    game.run();
}
