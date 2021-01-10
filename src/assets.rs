use crate::{
    update_asset_storage_system, AssetChannel, AssetLoader, AssetServer, AssetTypeRegistry,
    ChannelAssetHandler,
};
use atelier_importer::BoxedImporter;
use atelier_loader::{
    handle::{Handle, GenericHandle, AssetHandle},
    storage::{LoadHandle, IndirectionTable},
};
use bevy_app::{prelude::Events, AppBuilder};
use bevy_ecs::{FromResources, IntoSystem, ResMut, Resource, Resources};
use bevy_log::*;
use bevy_reflect::prelude::RegisterTypeBuilder;
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use type_uuid::TypeUuid;
use serde::de::DeserializeOwned;

/// Events that happen on assets of type `T`
pub enum AssetEvent<T: Resource> {
    Created { handle: Handle<T> },
    Modified { handle: Handle<T> },
    Removed { handle: Handle<T> },
}

struct AssetVersion<T> {
    asset: T,
    version: u32,
}
struct AssetState<T> {
    uncommitted: Option<AssetVersion<T>>,
    committed: Option<AssetVersion<T>>,
}
/// Stores Assets of a given type and tracks changes to them.
pub struct Assets<T: Resource> {
    assets: HashMap<LoadHandle, AssetState<T>>,
    events: Events<AssetEvent<T>>,
    indirection_table: IndirectionTable,
}

impl<T: Resource> FromResources for Assets<T> {
    fn from_resources(resources: &Resources) -> Self {
        let asset_server = resources.get::<AssetServer>().unwrap();
        Assets {
            assets: HashMap::default(),
            events: Events::default(),
            indirection_table: asset_server.loader.indirection_table(),
        }
    }
}

impl<T: Resource> Assets<T> {
    pub fn get(&self, handle: &Handle<T>) -> Option<&T> {
        let handle = handle.load_handle();
		let handle = if handle.is_indirect() {
             if let Some(handle) = self.indirection_table.resolve(handle) {
                 handle
             } else {
                 return None;
             }
         } else {
             handle
         };
         let asset = self.assets.get(&handle);
         if let Some(AssetState { committed: Some(asset_version), .. }) = asset {
            Some(&asset_version.asset)
         } else {
             None
         }
    }

    pub fn asset_event_system(
        mut events: ResMut<Events<AssetEvent<T>>>,
        mut assets: ResMut<Assets<T>>,
    ) {
        events.extend(assets.events.drain())
    }
}

/// [AppBuilder] extension methods for adding new asset types
pub trait AddAsset {
    fn add_asset<T>(&mut self) -> &mut Self
    where
        T: Resource + TypeUuid + DeserializeOwned;
    fn add_importer<TImporter, EXT: AsRef<str>>(&mut self, ext: EXT) -> &mut Self
    where
        TImporter: BoxedImporter + TypeUuid + FromResources;
}

impl AddAsset for AppBuilder {
    fn add_asset<T>(&mut self) -> &mut Self
    where
        T: Resource + TypeUuid + DeserializeOwned,
    {
        {
            let mut asset_type_registry = self
                .resources()
                .get_mut::<AssetTypeRegistry>()
                .expect("AssetTypeRegistry does not exist. Consider adding it as a resource.");
            asset_type_registry.register::<T>();
        }
        self.init_resource::<Assets<T>>()
            .register_type::<Handle<T>>()
            .add_system_to_stage(
                super::stage::ASSET_EVENTS,
                Assets::<T>::asset_event_system.system(),
            )
            .add_event::<AssetEvent<T>>()
    }

    fn add_importer<TImporter, EXT: AsRef<str>>(&mut self, ext: EXT) -> &mut Self
    where
        TImporter: BoxedImporter + TypeUuid + FromResources,
    {
        //TODO
        /*
        {
            let mut asset_server = self
                .resources()
                .get_mut::<AssetServer>()
                .expect("AssetServer does not exist. Consider adding it as a resource.");
            asset_server.add_importer(
                <TImporter as FromResources>::from_resources(self.resources()),
                ext,
            );
        }
        */
        self
    }
}

pub(crate) struct AssetsRefCell<'a, T: Resource>(pub RefCell<&'a mut Assets<T>>);

impl<'a, T: Resource + DeserializeOwned> atelier_loader::storage::AssetStorage for AssetsRefCell<'a, T> {
    fn update_asset(
        &self,
        loader_info: &dyn atelier_loader::storage::LoaderInfoProvider,
        asset_type_id: &atelier_core::AssetTypeId,
        data: Vec<u8>,
        load_handle: atelier_loader::storage::LoadHandle,
        load_op: atelier_loader::storage::AssetLoadOp,
        version: u32,
    ) -> Result<(), Box<dyn Error + Send + 'static>> {
        let mut assets = self.0.borrow_mut();
        assets.assets.insert(
            load_handle,
            AssetState {
                uncommitted: Some(AssetVersion {
                    asset: bincode::deserialize::<T>(&data).expect("failed to deserialize asset"),
                    version,
                }),
                committed: None
            });
        info!("{} bytes loaded for {:?}", data.len(), load_handle);
        // The loading process could be async, in which case you can delay
        // calling `load_op.complete` as it should only be done when the asset is usable.
        load_op.complete();
        Ok(())
    }
    fn commit_asset_version(
        &self,
        asset_type: &atelier_core::AssetTypeId,
        load_handle: atelier_loader::LoadHandle,
        version: u32,
    ) {
        let mut assets = self.0.borrow_mut();
        let state = assets.assets.get_mut(&load_handle).expect("asset not present when committing");
        let uncommitted = state.uncommitted.take().expect("Committing asset without an uncommited state");
        state.committed = Some(uncommitted);
        println!("Commit {:?}", load_handle);
    }
    fn free(
        &self,
        asset_type_id: &atelier_core::AssetTypeId,
        load_handle: atelier_loader::LoadHandle,
        version: u32,
    ) {
        let mut assets = self.0.borrow_mut();
        if let Some(asset) = assets.assets.get_mut(&load_handle) {
            if let Some(uncommitted) = &mut asset.uncommitted {
                if uncommitted.version == version {
                    asset.uncommitted = None;
                }
            }
            if let Some(committed) = &mut asset.committed {
                if committed.version == version {
                    asset.committed = None;
                }
            }
            if asset.uncommitted.is_none() && asset.committed.is_none() {
                assets.assets.remove(&load_handle);
            }
        }
        info!("Free {:?}", load_handle);
    }
}
