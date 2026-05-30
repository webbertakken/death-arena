use arena::ArenaPlugin;
use bevy::app::PluginGroup;
use bevy::app::PluginGroupBuilder;

use pickup::PickupPlugin;
use player::PlayerPlugin;
use virtual_player::VirtualPlayerPlugin;

mod arena;
mod main;
mod pickup;
mod player;
mod virtual_player;

pub struct GameplayPlugins;

impl PluginGroup for GameplayPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(PlayerPlugin)
            .add(VirtualPlayerPlugin)
            .add(PickupPlugin)
            .add(ArenaPlugin)
    }
}
