use crate::app::physics::collider::ColliderData;

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext};

use std::default::Default;

#[derive(Default)]
pub struct ColliderLoader;

impl AssetLoader for ColliderLoader {
    type Asset = ColliderData;
    type Settings = ();
    type Error = anyhow::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let collider_asset = match ron::de::from_bytes::<ColliderData>(&bytes) {
            Ok(collider) => collider,
            Err(e) => {
                eprintln!("failed deserializing collider from file: {e}");
                ColliderData::NoCollider
            }
        };

        Ok(collider_asset)
    }

    fn extensions(&self) -> &[&str] {
        &["collider"]
    }
}
