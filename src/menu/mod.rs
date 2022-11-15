use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

use dealer::DealerMenuPlugin;
use game_selection::GameSelectionPlugin;
use garage::GarageMenuPlugin;
use main::MainMenuPlugin;
use settings::SettingsMenuPlugin;

mod dealer;
mod game_selection;
mod garage;
mod main;
mod settings;

#[allow(dead_code)]
#[derive(Clone, Eq, PartialEq, Debug, Hash)]
pub enum MenuState {
    Main,
    Settings,
    SettingsAudio,
    SettingsVideo,
    SettingsControls,
    Garage,
    GarageCarSelection,
    Dealer,
    GameSelection,
    Credits,
    InGame,
    Paused,
    Hidden,
}

pub struct MenuPlugins;

impl PluginGroup for MenuPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(MainMenuPlugin::default())
            .add(GarageMenuPlugin::default())
            .add(SettingsMenuPlugin::default())
            .add(DealerMenuPlugin::default())
            .add(GameSelectionPlugin::default())
    }
}
