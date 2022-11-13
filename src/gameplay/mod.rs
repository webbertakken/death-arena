use arena::ArenaPlugin;
use bevy::app::PluginGroup;
use bevy::app::PluginGroupBuilder;

use player::PlayerPlugin;

mod arena;
mod main;
mod player;

pub struct GameplayPlugins;

impl PluginGroup for GameplayPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(PlayerPlugin::default())
            .add(ArenaPlugin::default())
    }
}
