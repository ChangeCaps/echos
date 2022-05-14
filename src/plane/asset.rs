use bevy::{
    asset::{AssetLoader, LoadedAsset},
    prelude::warn,
};
use serde::Deserialize;

use super::PlaneDescriptor;

pub struct PlaneAssetLoader;

impl AssetLoader for PlaneAssetLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut bevy::asset::LoadContext,
    ) -> bevy::asset::BoxedFuture<'a, Result<(), anyhow::Error>> {
        Box::pin(async {
            let mut deserializer = ron::Deserializer::from_bytes(bytes)?;

            let plane_descriptor = match PlaneDescriptor::deserialize(&mut deserializer) {
                Ok(d) => d,
                Err(err) => {
                    warn!("error loading plane: {}", err);

                    PlaneDescriptor::default()
                }
            };

            load_context.set_default_asset(LoadedAsset::new(plane_descriptor));

            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["plane.ron"]
    }
}
