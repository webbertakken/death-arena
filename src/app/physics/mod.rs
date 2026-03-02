use crate::app::physics::collider::ColliderData;
use crate::app::physics::collider_loader::ColliderLoader;

use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

pub mod collider;
pub mod collider_loader;

#[derive(Default)]
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<ColliderData>()
            .init_asset_loader::<ColliderLoader>()
            .add_systems(Startup, setup_physics);
    }
}

fn setup_physics(mut rapier_config: Query<&mut RapierConfiguration>) {
    for mut config in &mut rapier_config {
        config.gravity = Vect::ZERO;
    }
}
