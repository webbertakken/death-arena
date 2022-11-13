use crate::app::physics::collider::ColliderData;

use bevy::{
    asset::{AssetLoader, LoadContext, LoadedAsset},
    utils::BoxedFuture,
};

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
