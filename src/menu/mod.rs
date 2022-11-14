use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

use main_menu::MainMenuPlugin;

mod main_menu;

#[allow(dead_code)]
#[derive(Clone, Eq, PartialEq, Debug, Hash)]
pub enum MenuState {
    Main,
    Garage,
    Dealer,
    ArenaSelection,
    InGame,
    Paused,
    Hidden,
}

pub struct MenuPlugins;

impl PluginGroup for MenuPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>().add(MainMenuPlugin::default())
    }
}
