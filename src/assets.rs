use crate::{
    update_asset_storage_system, AssetChannel, AssetLoader, AssetServer, AssetTypeRegistry,
    ChannelAssetHandler, HANDLE_ALLOCATOR,
};
use atelier_importer::BoxedImporter;
use atelier_loader::{
    handle::{AssetHandle, GenericHandle, Handle, RefOp},
    storage::{IndirectionTable, LoadHandle, HandleAllocator},
    crossbeam_channel::{unbounded, Receiver, Sender},
};
use bevy_app::{prelude::Events, AppBuilder};
use bevy_ecs::{FromResources, IntoSystem, ResMut, Resource, Resources};
use bevy_log::*;
use bevy_reflect::prelude::RegisterTypeBuilder;
use serde::de::DeserializeOwned;
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use type_uuid::TypeUuid;

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
/// Stores Assets of a given type and tracks changes to them.
pub struct Assets<T: Resource> {
    uncommitted: HashMap<LoadHandle, AssetVersion<T>>,
    committed: HashMap<LoadHandle, AssetVersion<T>>,
    runtime_assets: HashMap<LoadHandle, T>,
    events: Events<AssetEvent<T>>,
    indirection_table: IndirectionTable,
    ref_op_tx: Sender<RefOp>,
}

impl<T: Resource> FromResources for Assets<T> {
    fn from_resources(resources: &Resources) -> Self {
        let asset_server = resources.get::<AssetServer>().unwrap();
        Assets {
            uncommitted: HashMap::default(),
            committed: HashMap::default(),
            runtime_assets: HashMap::default(),
            events: Events::default(),
            indirection_table: asset_server.loader.indirection_table(),
            ref_op_tx: asset_server.ref_op_tx(),
        }
    }
}

pub trait RuntimeLoadHandle {
    fn is_runtime(&self) -> bool;
    fn set_runtime(&self) -> LoadHandle;
}
impl RuntimeLoadHandle for LoadHandle {
    fn is_runtime(&self) -> bool {
        (self.0 & (1 << 62)) == 1 << 62
    }

    fn set_runtime(&self) -> LoadHandle {
        LoadHandle(self.0 | (1 << 63))
    }
}

impl<T: Resource> Assets<T> {
    pub fn add(&mut self, asset: T) -> Handle<T> {
        // TODO: All these methods need to emit events
        let load_handle = HANDLE_ALLOCATOR.alloc().set_runtime();
        self.runtime_assets.insert(load_handle, asset);
        Handle::<T>::new(self.ref_op_tx.clone(), load_handle).into()
    }

    pub fn set(&mut self, handle: &Handle<T>, asset: T) -> Handle<T> {
        if handle.load_handle().is_runtime() {
            self.runtime_assets.insert(handle.load_handle(), asset);
            handle.clone()
        } else {
            // TODO: Is this reasonable behavior? A new handle is issued but the old one remains valid
            // The atelier managed asset will be dropped if the origonal handle is dropped and that's the last reference.
            self.add(asset)
        }
    }

    pub fn set_untracked(&mut self, handle: LoadHandle, asset: T) {
        self.runtime_assets.insert(handle, asset);
    }

    pub fn contains(&self, handle: &Handle<T>) -> bool {
        let handle = handle.load_handle();
        self.runtime_assets.contains_key(&handle) || self.committed.contains_key(&handle)
    }

    fn resolve_handle(&self, handle: &Handle<T>) -> Option<LoadHandle> {
        let handle = handle.load_handle();
        if handle.is_indirect() {
            if let Some(handle) = self.indirection_table.resolve(handle) {
                Some(handle)
            } else {
                None
            }
        } else {
            Some(handle)
        }
    }

    pub fn get(&self, handle: &Handle<T>) -> Option<&T> {
        if let Some(handle) = self.resolve_handle(handle) {
            let asset = self.committed.get(&handle);
            if let Some(asset_version) = asset {
                Some(&asset_version.asset)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, handle: &Handle<T>) -> Option<&mut T> {
        if let Some(handle) = self.resolve_handle(handle) {
            let asset = self.committed.get_mut(&handle);
            if let Some(asset_version) = asset {
                Some(&mut asset_version.asset)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn get_handle(&self, handle: &Handle<T>) -> Handle<T> {
        unimplemented!();
    }

    pub fn get_or_insert_with(
        &mut self,
        handle: &Handle<T>,
        insert_fn: impl FnOnce() -> T,
    ) -> &mut T {
        unimplemented!();
    }

    /*
    pub fn iter(&self) -> impl Iterator<Item = (&Handle<T>, &T)> {
        unimplemented!()
    }

    pub fn ids(&self) -> impl Iterator<Item = &Handle<T>> + '_ {
        unimplemented!()
    }
    */

    pub fn remove(&mut self, handle: &Handle<T>) -> Option<T> {
        unimplemented!();
    }

    /// Clears the inner asset map, removing all key-value pairs.
    ///
    /// Keeps the allocated memory for reuse.
    pub fn clear(&mut self) {
        unimplemented!();
    }

    /// Reserves capacity for at least additional more elements to be inserted into the assets.
    ///
    /// The collection may reserve more space to avoid frequent reallocations.
    pub fn reserve(&mut self, additional: usize) {
        unimplemented!();
    }

    /// Shrinks the capacity of the asset map as much as possible.
    ///
    /// It will drop down as much as possible while maintaining the internal rules and possibly
    /// leaving some space in accordance with the resize policy.
    pub fn shrink_to_fit(&mut self) {
        unimplemented!();
    }

    pub fn len(&self) -> usize {
        self.committed.len()
    }

    pub fn is_empty(&self) -> bool {
        self.committed.is_empty()
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

impl<'a, T: Resource + DeserializeOwned> atelier_loader::storage::AssetStorage
    for AssetsRefCell<'a, T>
{
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
        assets.uncommitted.insert(
            load_handle,
            AssetVersion {
                asset: bincode::deserialize::<T>(&data).expect("failed to deserialize asset"),
                version,
            },
        );
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
        let uncommitted = assets
            .uncommitted
            .remove(&load_handle)
            .expect("asset not present when committing");
        assets.committed.insert(load_handle, uncommitted);
    }
    fn free(
        &self,
        asset_type_id: &atelier_core::AssetTypeId,
        load_handle: atelier_loader::LoadHandle,
        version: u32,
    ) {
        let mut assets = self.0.borrow_mut();
        if let Some(asset_version) = assets.uncommitted.get(&load_handle) {
            if asset_version.version == version {
                assets.uncommitted.remove(&load_handle);
            }
        }
        if let Some(asset_version) = assets.committed.get(&load_handle) {
            if asset_version.version == version {
                assets.committed.remove(&load_handle);
            }
        }
        info!("Free {:?}", load_handle);
    }
}
