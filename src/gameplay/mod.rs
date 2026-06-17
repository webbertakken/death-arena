use arena::ArenaPlugin;
use bevy::app::PluginGroup;
use bevy::app::PluginGroupBuilder;
use combat::CombatPlugin;
use ctf::CtfPlugin;

use pickup::PickupPlugin;
use player::PlayerPlugin;
use virtual_player::VirtualPlayerPlugin;

mod arena;
mod carry_fatigue;
mod chase_resolve;
mod combat;
mod comeback;
mod ctf;
mod flag_escort;
mod flag_rally;
mod front_runner;
mod main;
mod pickup;
mod player;
mod slipstream;
mod virtual_player;
mod wall_scrape;

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
