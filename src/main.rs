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
use gameplay::GameplayPlugins;

pub mod app;
pub mod core;
mod gameplay;
mod menu;
pub mod ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, States)]
pub enum AppState {
    #[default]
    Menus,
    Loading,
    InGame,
}

fn main() {
    core::init();

    // Setup
    let mut game = App::new();
    game.add_plugins(InitPlugin);
    game.add_plugins(DefaultPlugins::configure());

    // State
    if cfg!(debug_assertions) {
        // Development
        game.init_state::<AppState>();
        game.insert_state(AppState::Loading);
        game.init_state::<MenuState>();
        game.insert_state(MenuState::Dealer);
    } else {
        // Production
        game.init_state::<AppState>();
        game.init_state::<MenuState>();
    }

    // Logic
    game.add_plugins(AppPlugins);
    game.add_plugins(MenuPlugins);
    game.add_plugins(GameplayPlugins);

    // Run the app
    game.run();
}
