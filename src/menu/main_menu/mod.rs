use crate::menu::MenuState;
use crate::{App, Input, KeyCode, Plugin, Query, Res, Transform, Vec3};
use bevy::prelude::*;
use bevy_kira_audio::prelude::*;

mod ui;

#[derive(Component)]
pub enum ButtonAction {
    MainMenu,
    Career,
    Multiplayer,
    Restart,
    Quit,
}

#[derive(Default)]
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        // Enter
        app.add_system_set(SystemSet::on_enter(MenuState::Main).with_system(ui::show));

        // Update
        app.add_system_set(SystemSet::on_update(MenuState::Main).with_system(on_update_main_menu));

        // Exit
        app.add_system_set(SystemSet::on_exit(MenuState::Main).with_system(ui::hide));
    }
}

pub fn on_update_main_menu(commands: Commands, asset_server: Res<AssetServer>) {
    // log::info!("Hello, update!");
}
