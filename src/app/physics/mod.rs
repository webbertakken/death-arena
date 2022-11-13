use crate::app::physics::collider::ColliderData;
use crate::app::physics::collider_loader::ColliderLoader;
use bevy::asset::{AssetLoader, BoxedFuture, LoadContext, LoadedAsset};
use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

pub mod collider;
pub mod collider_loader;

#[derive(Default)]
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RapierConfiguration {
            gravity: Vec2::ZERO,
            ..Default::default()
        })
        .add_asset::<ColliderData>()
        .init_asset_loader::<ColliderLoader>()
        .add_startup_system(setup_physics);
    }
}

const fn setup_physics() {}
