use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use bevy::prelude::*;
use bevy_kira_audio::AudioPlugin;
use setup::SetupPlugin;

mod setup;

pub struct AppPlugins;

impl PluginGroup for AppPlugins {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group.add(SetupPlugin::default());
    }
}
