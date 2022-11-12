use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

#[derive(Default)]
pub struct ConfigurationPlugin;

impl Plugin for ConfigurationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RapierConfiguration {
            gravity: Vec2::ZERO,
            ..Default::default()
        })
        .add_startup_system(setup_physics);
    }
}

const fn setup_physics() {}
