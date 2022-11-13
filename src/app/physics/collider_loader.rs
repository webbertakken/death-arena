use crate::app::physics::collider::ColliderData;
use crate::gameplay::GameState;
use crate::AppState;
use bevy::prelude::*;
use bevy::{
    asset::{AssetLoader, LoadContext, LoadedAsset},
    prelude::*,
    reflect::TypeUuid,
    utils::BoxedFuture,
};
use bevy_rapier2d::prelude::*;
use serde::Deserialize;
use std::default::Default;

#[derive(Default)]
pub struct ColliderLoader;

impl AssetLoader for ColliderLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            // let collider_asset = from_slice::<ColliderData>(bytes)?;

            let collider_asset = match ron::de::from_bytes::<ColliderData>(bytes) {
                Ok(collider) => collider,
                Err(e) => {
                    eprintln!("failed deserializing collider from file: {}", e);
                    ColliderData::NoCollider
                }
            };

            load_context.set_default_asset(LoadedAsset::new(collider_asset));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["collider"]
    }
}
