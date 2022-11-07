use crate::app::generic::GenericSystemsPlugin;
use crate::app::lights::LightingPlugin;
use crate::ui::UserInterfacePlugin;
use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use bevy::prelude::*;
use bevy_kira_audio::AudioPlugin;
use init::InitPlugin;

pub mod generic;
pub mod init;
pub mod lights;

pub struct AppPlugins;

impl PluginGroup for AppPlugins {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group.add(AudioPlugin::default());
        group.add(LightingPlugin::default());
        group.add(GenericSystemsPlugin::default());
        group.add(UserInterfacePlugin::default());
    }
}
