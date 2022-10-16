use bevy::app::PluginGroup;
use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

mod hello_world;
mod player;

pub struct GameplayPlugins;

impl PluginGroup for GameplayPlugins {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group.add(hello_world::HelloWorldPlugin::default());
        group.add(player::PlayerPlugin::default());
    }
}
