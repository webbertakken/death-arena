use bevy::app::PluginGroup;
use bevy::app::PluginGroupBuilder;

mod world;

pub struct EnvironmentPlugins;

impl PluginGroup for EnvironmentPlugins {
    fn build(&mut self, group: &mut PluginGroupBuilder) {
        group.add(world::WorldPlugin::default());
    }
}
