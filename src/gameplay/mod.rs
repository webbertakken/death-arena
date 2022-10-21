use bevy::app::PluginGroup;
use bevy::app::PluginGroupBuilder;
use bevy_kira_audio::AudioPlugin;

mod arena;
mod main;
mod player;

pub struct GameplayPlugins;

impl PluginGroup for GameplayPlugins {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group.add(AudioPlugin);
        group.add(player::PlayerPlugin::default());
        group.add(arena::ArenaPlugin::default());
    }
}
