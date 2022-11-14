use crate::app::generic::GenericSystemsPlugin;
use crate::app::lights::LightingPlugin;
use crate::app::physics::PhysicsPlugin;
use crate::ui::UserInterfacePlugin;
use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use bevy_kira_audio::AudioPlugin;
use bevy_rapier2d::prelude::*;

pub mod generic;
pub mod init;
pub mod lights;
pub mod physics;

pub struct AppPlugins;

impl PluginGroup for AppPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(AudioPlugin::default())
            .add(LightingPlugin::default())
            .add(GenericSystemsPlugin::default())
            .add(UserInterfacePlugin::default())
            .add(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
            .add(RapierDebugRenderPlugin::default())
            .add(PhysicsPlugin::default())
    }
}
