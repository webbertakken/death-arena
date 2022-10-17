use bevy::app::PluginGroup;
use bevy::app::PluginGroupBuilder;

mod main;
mod player;

pub struct GameplayPlugins;

impl PluginGroup for GameplayPlugins {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group.add(player::PlayerPlugin::default());
    }
}
