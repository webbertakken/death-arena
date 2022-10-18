use crate::{EnvironmentPlugins, PluginGroup};
use bevy::app::PluginGroupBuilder;
use bevy_kira_audio::AudioPlugin;
use dotenvy::dotenv;

pub fn init() {
    dotenv().ok();
}

pub struct CorePlugins;

impl PluginGroup for CorePlugins {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group.add(AudioPlugin);
    }
}
