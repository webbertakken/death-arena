use arena::ArenaPlugin;
use bevy::app::PluginGroup;
use bevy::app::PluginGroupBuilder;
use combat::CombatPlugin;
use ctf::CtfPlugin;

use pickup::PickupPlugin;
use player::PlayerPlugin;
use virtual_player::VirtualPlayerPlugin;

mod arena;
mod combat;
mod ctf;
mod main;
mod pickup;
mod player;
mod slipstream;
mod virtual_player;

pub struct GameplayPlugins;

impl PluginGroup for GameplayPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(PlayerPlugin)
            .add(VirtualPlayerPlugin)
            .add(PickupPlugin)
            .add(CombatPlugin)
            .add(CtfPlugin)
            .add(ArenaPlugin)
    }
}
