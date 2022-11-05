use crate::ui::UserInterfacePlugin;
use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use bevy::prelude::*;
use bevy_kira_audio::AudioPlugin;
use init::InitPlugin;

pub mod init;

pub struct AppPlugins;

impl PluginGroup for AppPlugins {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group.add(AudioPlugin::default());
        group.add(UserInterfacePlugin::default());
    }
}
