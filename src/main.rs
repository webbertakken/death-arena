#![warn(clippy::nursery, clippy::pedantic)]
// Never-ship lints (clippy::restriction, not covered by nursery/pedantic): a
// stray debug print runs every frame inside an ECS system, flooding the console
// and bypassing Bevy's structured logging, and a placeholder panic hard-crashes
// the WASM canvas. Denied so the existing clippy gate catches them at the door.
#![deny(
    clippy::dbg_macro,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::todo,
    clippy::unimplemented
)]
#![allow(
    clippy::module_name_repetitions,
    clippy::needless_pass_by_value,
    clippy::only_used_in_recursion
)]
use crate::menu::{MenuPlugins, MenuState};
use app::{init::InitPlugin, AppPlugins};
use bevy::prelude::*;

use crate::app::init::default_plugins::Configure;
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
        game.add_state(MenuState::Dealer);
    } else {
        // Production
        game.add_state(AppState::Menus);
        game.add_state(MenuState::Main);
    }

    // Logic
    game.add_plugins(AppPlugins);
    game.add_plugins(MenuPlugins);
    game.add_plugins(GameplayPlugins);

    // Run the app
    game.run();
}
