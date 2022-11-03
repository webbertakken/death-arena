use crate::AppState;
use arena::ArenaPlugin;
use bevy::app::PluginGroup;
use bevy::app::PluginGroupBuilder;
use bevy::prelude::SystemSet;
use bevy_kira_audio::AudioPlugin;
use player::PlayerPlugin;

mod arena;
mod main;
mod player;

pub struct GameplayPlugins;

impl PluginGroup for GameplayPlugins {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group.add(PlayerPlugin::default());
        group.add(ArenaPlugin::default());
    }
}
