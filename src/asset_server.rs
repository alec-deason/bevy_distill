use crate::{
    AssetLoadError, AssetLoadRequestHandler, AssetTypeId, AssetTypeRegistry, Handle, HandleId,
    HandleUntyped, LoadRequest, HANDLE_ALLOCATOR,
};
use anyhow::Result;
use atelier_daemon::AssetDaemon;
use atelier_importer::BoxedImporter;
pub use atelier_loader::storage::LoadStatus;
use atelier_loader::{
    crossbeam_channel::{unbounded, Receiver, Sender},
    handle::{AssetHandle, RefOp},
    rpc_io::RpcIO,
    storage::{AssetLoadOp, DefaultIndirectionResolver, LoadHandle, LoaderInfoProvider},
    Loader,
};
use bevy_ecs::{Res, Resource, Resources};
use crossbeam_channel::TryRecvError;
use parking_lot::RwLock;
use std::{
    collections::{HashMap, HashSet},
    env,
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
};
use thiserror::Error;
use type_uuid::{self, TypeUuid};

/// The type used for asset versioning
pub type AssetVersion = usize;

/// Errors that occur while loading assets with an AssetServer
#[derive(Error, Debug)]
pub enum AssetServerError {
    #[error("Asset folder path is not a directory.")]
    AssetFolderNotADirectory(String),
    #[error("Invalid root path")]
    InvalidRootPath,
    #[error("No AssetHandler found for the given extension.")]
    MissingAssetHandler,
    #[error("No AssetLoader found for the given extension.")]
    MissingAssetLoader,
    #[error("No asset registration found for a loaded.")]
    MissingAssetRegistration(AssetTypeId),
    #[error("Encountered an error while loading an asset.")]
    AssetLoadError(#[from] AssetLoadError),
    #[error("Encountered an io error.")]
    Io(#[from] io::Error),
    #[error("Failed to watch asset folder.")]
    AssetWatchError { path: PathBuf },
}

struct LoaderThread {
    // NOTE: these must remain private. the LoaderThread Arc counters are used to determine thread liveness
    // if there is one reference, the loader thread is dead. if there are two references, the loader thread is active
    requests: Arc<RwLock<Vec<LoadRequest>>>,
}

/// Info about a specific asset, such as its path and its current load state
#[derive(Debug)]
pub struct AssetInfo {
    pub handle_id: HandleId,
    pub path: PathBuf,
    pub load_state: LoadStatus,
}

enum DaemonState {
    Building(),
}

/// Loads assets from the filesystem on background threads
pub struct AssetServer {
    asset_folders: RwLock<Vec<PathBuf>>,
    loader_threads: RwLock<Vec<LoaderThread>>,
    max_loader_threads: usize,
    asset_handlers: Arc<RwLock<Vec<Box<dyn AssetLoadRequestHandler>>>>,
    // TODO: this is a hack to enable retrieving generic AssetLoader<T>s. there must be a better way!
    loaders: Vec<Resources>,
    daemon: AssetDaemon,
    loader: Loader,
    ref_op_tx: Sender<RefOp>,
    ref_op_rx: Receiver<RefOp>,
}

impl Default for AssetServer {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        AssetServer {
            max_loader_threads: 4,
            asset_folders: Default::default(),
            loader_threads: Default::default(),
            asset_handlers: Default::default(),
            loaders: Default::default(),
            daemon: AssetDaemon::default(),
            loader: Loader::new_with_handle_allocator(
                Box::new(RpcIO::default()),
                Arc::new(&HANDLE_ALLOCATOR),
            ),
            ref_op_tx: tx,
            ref_op_rx: rx,
        }
    }
}

impl AssetServer {
    pub(crate) fn ref_op_tx(&self) -> Sender<RefOp> {
        self.ref_op_tx.clone()
    }
    pub fn add_importer<T: BoxedImporter + TypeUuid, EXT: AsRef<str>>(
        &mut self,
        importer: T,
        ext: EXT,
    ) {
        self.daemon.add_importer(ext.as_ref(), importer);
    }

    // pub fn get_handle<T, P: AsRef<Path>>(&self, path: P) -> Option<Handle<T>> {
    //     self.asset_info_paths
    //         .read()
    //         .get(path.as_ref())
    //         .map(|handle_id| Handle::from(*handle_id))
    // }

    // TODO: add type checking here. people shouldn't be able to request a Handle<Texture> for a Mesh asset
    pub fn load<T, P: AsRef<Path>>(&self, path: P) -> Result<Handle<T>, AssetServerError> {
        self.load_untyped(path).map(Handle::from)
    }

    pub fn load_untyped<P: AsRef<Path>>(&self, path: P) -> Result<HandleUntyped, AssetServerError> {
        Err(AssetServerError::InvalidRootPath)
        //     let path = path.as_ref();
        //     if let Some(ref extension) = path.extension() {
        //         if let Some(index) = self.extension_to_handler_index.get(
        //             extension
        //                 .to_str()
        //                 .expect("Extension should be a valid string."),
        //         ) {
        //             let mut new_version = 0;
        //             let handle_id = {
        //                 let mut asset_info = self.asset_info.write();
        //                 let mut asset_info_paths = self.asset_info_paths.write();
        //                 if let Some(asset_info) = asset_info_paths
        //                     .get(path)
        //                     .and_then(|handle_id| asset_info.get_mut(&handle_id))
        //                 {
        //                     asset_info.load_state =
        //                         if let LoadStatus::Loaded(_version) = asset_info.load_state {
        //                             new_version += 1;
        //                             LoadStatus::Loading(new_version)
        //                         } else {
        //                             LoadStatus::Loading(new_version)
        //                         };
        //                     asset_info.handle_id
        //                 } else {
        //                     let handle_id = HandleId::new();
        //                     asset_info.insert(
        //                         handle_id,
        //                         AssetInfo {
        //                             handle_id,
        //                             path: path.to_owned(),
        //                             load_state: LoadStatus::Loading(new_version),
        //                         },
        //                     );
        //                     asset_info_paths.insert(path.to_owned(), handle_id);
        //                     handle_id
        //                 }
        //             };

        //             self.send_request_to_loader_thread(LoadRequest {
        //                 handle_id,
        //                 path: path.to_owned(),
        //                 handler_index: *index,
        //                 version: new_version,
        //             });

        //             // TODO: watching each asset explicitly is a simpler implementation, its possible it would be more efficient to watch
        //             // folders instead (when possible)
        //             #[cfg(feature = "filesystem_watcher")]
        //             Self::watch_path_for_changes(&mut self.filesystem_watcher.write(), path)?;
        //             Ok(handle_id)
        //         } else {
        //             Err(AssetServerError::MissingAssetHandler)
        //         }
        //     } else {
        //         Err(AssetServerError::MissingAssetHandler)
        //     }
    }

    pub fn get_load_state_untyped(&self, handle_id: HandleUntyped) -> LoadStatus {
        self.loader.get_load_status(handle_id.handle.load_handle())
    }

    pub fn get_load_state<T>(&self, handle: Handle<T>) -> LoadStatus {
        self.loader.get_load_status(handle.handle.load_handle())
    }

    // pub fn get_group_load_state(&self, handle_ids: &[HandleId]) -> Option<LoadStatus> {
    //     let mut load_state = LoadStatus::Loaded(0);
    //     for handle_id in handle_ids.iter() {
    //         match self.get_load_state_untyped(*handle_id) {
    //             Some(LoadStatus::Loaded(_)) => continue,
    //             Some(LoadStatus::Loading(_)) => {
    //                 load_state = LoadStatus::Loading(0);
    //             }
    //             Some(LoadStatus::Failed(_)) => return Some(LoadStatus::Failed(0)),
    //             None => return None,
    //         }
    //     }

    //     Some(load_state)
    // }

    // fn send_request_to_loader_thread(&self, load_request: LoadRequest) {
    //     // NOTE: This lock makes the call to Arc::strong_count safe. Removing (or reordering) it could result in undefined behavior
    //     let mut loader_threads = self.loader_threads.write();
    //     if loader_threads.len() < self.max_loader_threads {
    //         let loader_thread = LoaderThread {
    //             requests: Arc::new(RwLock::new(vec![load_request])),
    //         };
    //         let requests = loader_thread.requests.clone();
    //         loader_threads.push(loader_thread);
    //         Self::start_thread(self.asset_handlers.clone(), requests);
    //     } else {
    //         let most_free_thread = loader_threads
    //             .iter()
    //             .min_by_key(|l| l.requests.read().len())
    //             .unwrap();
    //         let mut requests = most_free_thread.requests.write();
    //         requests.push(load_request);
    //         // if most free thread only has one reference, the thread as spun down. if so, we need to spin it back up!
    //         if Arc::strong_count(&most_free_thread.requests) == 1 {
    //             Self::start_thread(
    //                 self.asset_handlers.clone(),
    //                 most_free_thread.requests.clone(),
    //             );
    //         }
    //     }
    // }

    // fn start_thread(
    //     request_handlers: Arc<RwLock<Vec<Box<dyn AssetLoadRequestHandler>>>>,
    //     requests: Arc<RwLock<Vec<LoadRequest>>>,
    // ) {
    //     thread::spawn(move || {
    //         loop {
    //             let request = {
    //                 let mut current_requests = requests.write();
    //                 if current_requests.len() == 0 {
    //                     // if there are no requests, spin down the thread
    //                     break;
    //                 }

    //                 current_requests.pop().unwrap()
    //             };

    //             let handlers = request_handlers.read();
    //             let request_handler = &handlers[request.handler_index];
    //             request_handler.handle_request(&request);
    //         }
    //     });
    // }

    // fn load_assets_in_folder_recursive(
    //     &self,
    //     path: &Path,
    // ) -> Result<Vec<HandleId>, AssetServerError> {
    //     if !path.is_dir() {
    //         return Err(AssetServerError::AssetFolderNotADirectory(
    //             path.to_str().unwrap().to_string(),
    //         ));
    //     }

    //     let root_path = self.get_root_path()?;
    //     let mut handle_ids = Vec::new();
    //     for entry in fs::read_dir(path)? {
    //         let entry = entry?;
    //         let child_path = entry.path();
    //         if child_path.is_dir() {
    //             handle_ids.extend(self.load_assets_in_folder_recursive(&child_path)?);
    //         } else {
    //             let relative_child_path = child_path.strip_prefix(&root_path).unwrap();
    //             let handle = match self.load_untyped(
    //                 relative_child_path
    //                     .to_str()
    //                     .expect("Path should be a valid string"),
    //             ) {
    //                 Ok(handle) => handle,
    //                 Err(AssetServerError::MissingAssetHandler) => continue,
    //                 Err(err) => return Err(err),
    //             };

    //             handle_ids.push(handle);
    //         }
    //     }

    //     Ok(handle_ids)
    // }

    pub fn process_system(_world: &mut bevy_ecs::World, resources: &mut Resources) {
        let mut asset_server = resources
            .get_mut::<Self>()
            .expect("AssetServer does not exist. Consider adding it as a resource.");
        let asset_type_registry = resources
            .get::<AssetTypeRegistry>()
            .expect("AssetTypeRegistry does not exist. Consider adding it as a resource.");
        let resolver = AssetStorageResolver(&*asset_type_registry, resources);
        asset_server
            .loader
            .process(&resolver, &DefaultIndirectionResolver)
            //TODO: Should this panic?
            .unwrap();
    }
}

struct AssetStorageResolver<'a, 'b>(&'a AssetTypeRegistry, &'b Resources);

impl<'a, 'b> atelier_loader::storage::AssetStorage for AssetStorageResolver<'a, 'b> {
    fn update_asset(
        &self,
        loader_info: &dyn LoaderInfoProvider,
        asset_type_id: &AssetTypeId,
        data: Vec<u8>,
        load_handle: LoadHandle,
        load_op: AssetLoadOp,
        version: u32,
    ) -> Result<(), Box<dyn Error + Send + 'static>> {
        if let Some(registration) = self.0.registrations.get(asset_type_id) {
            let mut result = None;
            let result_ref = &mut result;
            let mut load_op_arg = Some(load_op);
            (registration.get_assets_storage_fn)(
                self.1,
                &mut |storage: &dyn atelier_loader::storage::AssetStorage| {
                    *result_ref = Some(storage.update_asset(
                        loader_info,
                        asset_type_id,
                        //FIXME: This seems like a bad clone
                        data.clone(),
                        load_handle,
                        load_op_arg.take().unwrap(),
                        version,
                    ));
                },
            );
            result.unwrap()
        } else {
            log::error!(
                "Loaded asset type ID {:?} but it was not registered",
                asset_type_id
            );
            Err(Box::new(AssetServerError::MissingAssetRegistration(
                *asset_type_id,
            )))
        }
    }
    fn commit_asset_version(
        &self,
        asset_type_id: &atelier_core::AssetTypeId,
        load_handle: atelier_loader::LoadHandle,
        version: u32,
    ) {
        if let Some(registration) = self.0.registrations.get(asset_type_id) {
            (registration.get_assets_storage_fn)(
                self.1,
                &mut |storage: &dyn atelier_loader::storage::AssetStorage| {
                    storage.commit_asset_version(asset_type_id, load_handle, version);
                },
            );
        } else {
            log::error!(
                "Loaded asset type ID {:?} but it was not registered",
                asset_type_id
            );
        }
    }
    fn free(
        &self,
        asset_type_id: &atelier_core::AssetTypeId,
        load_handle: atelier_loader::LoadHandle,
        version: u32,
    ) {
        if let Some(registration) = self.0.registrations.get(asset_type_id) {
            (registration.get_assets_storage_fn)(
                self.1,
                &mut |storage: &dyn atelier_loader::storage::AssetStorage| {
                    storage.free(asset_type_id, load_handle, version);
                },
            );
        } else {
            log::error!(
                "Loaded asset type ID {:?} but it was not registered",
                asset_type_id
            );
        }
    }
}
