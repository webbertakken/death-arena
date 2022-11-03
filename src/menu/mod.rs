use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use bevy_kira_audio::AudioPlugin;
use main_menu::MainMenuPlugin;

mod main_menu;

pub struct MenuPlugins;

impl PluginGroup for MenuPlugins {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group.add(MainMenuPlugin::default());
    }
}
