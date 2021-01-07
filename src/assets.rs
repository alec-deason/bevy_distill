use crate::{
    update_asset_storage_system, AssetChannel, AssetLoader, AssetServer, AssetTypeRegistry,
    ChannelAssetHandler, Handle, HandleId, HandleUntyped,
};
use atelier_importer::BoxedImporter;
use bevy_app::{prelude::Events, AppBuilder};
use bevy_ecs::{FromResources, IntoSystem, ResMut, Resource};
use bevy_reflect::prelude::RegisterTypeBuilder;
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
struct AssetState<T> {
    uncommitted: Option<AssetVersion<T>>,
    committed: Option<AssetVersion<T>>,
}
/// Stores Assets of a given type and tracks changes to them.
pub struct Assets<T: Resource> {
    assets: HashMap<HandleId, AssetState<T>>,
    events: Events<AssetEvent<T>>,
}

impl<T: Resource> Default for Assets<T> {
    fn default() -> Self {
        Assets {
            assets: HashMap::default(),
            events: Events::default(),
        }
    }
}

impl<T: Resource> Assets<T> {
    // pub fn add(&mut self, asset: T) -> Handle<T> {
    //     let handle = Handle::new();
    //     self.assets.insert(handle, asset);
    //     self.events.send(AssetEvent::Created { handle });
    //     handle
    // }

    // pub fn set(&mut self, handle: Handle<T>, asset: T) {
    //     let exists = self.assets.contains_key(handle.id());
    //     self.assets.insert(handle, asset);

    //     if exists {
    //         self.events.send(AssetEvent::Modified { handle });
    //     } else {
    //         self.events.send(AssetEvent::Created { handle });
    //     }
    // }

    // pub fn add_default(&mut self, asset: T) -> Handle<T> {
    //     let handle = HandleId::default();
    //     let exists = self.assets.contains_key(&handle);
    //     self.assets.insert(handle, asset);
    //     if exists {
    //         self.events.send(AssetEvent::Modified { handle });
    //     } else {
    //         self.events.send(AssetEvent::Created { handle });
    //     }
    //     handle
    // }

    pub fn get_with_id(&self, id: HandleId) -> Option<&T> {
        self.assets
            .get(&id)
            .map(|a| a.committed.as_ref())
            .unwrap_or(None)
            .map(|a| &a.asset)
    }

    pub fn get_id_mut(&mut self, id: HandleId) -> Option<&mut T> {
        // self.events.send(AssetEvent::Modified { handle: *handle });
        self.assets
            .get_mut(&id)
            .map(|a| a.committed.as_mut())
            .unwrap_or(None)
            .map(|a| &mut a.asset)
    }

    pub fn get(&self, handle: &Handle<T>) -> Option<&T> {
        self.get_with_id(handle.id())
    }

    pub fn get_mut(&mut self, handle: &Handle<T>) -> Option<&mut T> {
        self.get_id_mut(handle.id())
    }

    // pub fn get_or_insert_with(
    //     &mut self,
    //     handle: Handle<T>,
    //     insert_fn: impl FnOnce() -> T,
    // ) -> &mut T {
    //     let mut event = None;
    //     let borrowed = self.assets.entry(handle.id()).or_insert_with(|| {
    //         event = Some(AssetEvent::Created { handle });
    //         insert_fn()
    //     });

    //     if let Some(event) = event {
    //         self.events.send(event);
    //     }
    //     borrowed
    // }

    // pub fn iter(&self) -> impl Iterator<Item = (&Handle<T>, &T)> {
    //     self.assets.iter().map(|(k, v)| (k, v))
    // }

    // pub fn remove(&mut self, handle: &Handle<T>) -> Option<T> {
    //     self.assets.remove(&handle.id())
    // }

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
        T: Resource + TypeUuid;
    fn add_importer<TImporter, EXT: AsRef<str>>(&mut self, ext: EXT) -> &mut Self
    where
        TImporter: BoxedImporter + TypeUuid + FromResources;
}

impl AddAsset for AppBuilder {
    fn add_asset<T>(&mut self) -> &mut Self
    where
        T: Resource + TypeUuid,
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
            .register_type::<HandleUntyped>()
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
        self
    }
}

pub(crate) struct AssetsRefCell<'a, T: Resource>(pub RefCell<&'a mut Assets<T>>);
impl<'a, T: Resource> atelier_loader::storage::AssetStorage for AssetsRefCell<'a, T> {
    fn update_asset(
        &self,
        loader_info: &dyn atelier_loader::storage::LoaderInfoProvider,
        asset_type_id: &atelier_core::AssetTypeId,
        data: Vec<u8>,
        load_handle: atelier_loader::storage::LoadHandle,
        load_op: atelier_loader::storage::AssetLoadOp,
        version: u32,
    ) -> Result<(), Box<dyn Error + Send + 'static>> {
        todo!()
    }
    fn commit_asset_version(
        &self,
        asset_type: &atelier_core::AssetTypeId,
        load_handle: atelier_loader::LoadHandle,
        version: u32,
    ) {
        todo!()
    }
    fn free(
        &self,
        asset_type_id: &atelier_core::AssetTypeId,
        load_handle: atelier_loader::LoadHandle,
        version: u32,
    ) {
        todo!()
    }
}
