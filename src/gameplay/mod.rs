use arena::ArenaPlugin;
use bevy::app::PluginGroup;
use bevy::app::PluginGroupBuilder;

use player::PlayerPlugin;

mod arena;
mod main;
mod player;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameState {
    Intro,
    Playing,
    Paused,
    Outro,
}

pub struct GameplayPlugins;

impl PluginGroup for GameplayPlugins {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group.add(PlayerPlugin::default());
        group.add(ArenaPlugin::default());
    }
}
