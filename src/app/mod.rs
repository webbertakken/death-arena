use crate::app::configuration::ConfigurationPlugin;
use crate::app::generic::GenericSystemsPlugin;
use crate::app::lights::LightingPlugin;
use crate::ui::UserInterfacePlugin;
use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use bevy_kira_audio::AudioPlugin;
use bevy_rapier2d::prelude::*;
use init::InitPlugin;

pub mod configuration;
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
        group.add(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0));
        group.add(RapierDebugRenderPlugin::default());
        group.add(ConfigurationPlugin::default());
    }
}
