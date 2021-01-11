mod asset_server;
mod asset_type_registry;
mod assets;
pub mod image;
mod load_request;
mod loader;

use std::path::PathBuf;
pub use asset_server::*;
use asset_type_registry::*;
pub use assets::*;
pub use load_request::*;
pub use loader::*;

/// The names of asset stages in an App Schedule
pub mod stage {
    pub const LOAD_ASSETS: &str = "load_assets";
    pub const ASSET_EVENTS: &str = "asset_events";
}

pub mod prelude {
    pub use crate::{AddAsset, AssetEvent, AssetServer, Assets};
}
pub use atelier_core::AssetTypeId;
use atelier_loader::storage::{AtomicHandleAllocator, LoadHandle};

use bevy_app::{prelude::Plugin, AppBuilder};
use bevy_ecs::{IntoSystem, SystemStage};
use bevy_reflect::RegisterTypeBuilder;

pub(crate) static HANDLE_ALLOCATOR: AtomicHandleAllocator = AtomicHandleAllocator::new(2);

/// Adds support for Assets to an App. Assets are typed collections with change tracking, which are added as App Resources.
/// Examples of assets: textures, sounds, 3d models, maps, scenes
#[derive(Default)]
pub struct AssetPlugin;


impl Plugin for AssetPlugin {
    fn build(&self, app: &mut AppBuilder) {
        let settings = app.app.resources.get::<AssetServerSettings>().map(|s| (*s).clone()).unwrap_or_default();
        let asset_server = AssetServer::new(&settings).unwrap();
        app
            .register_type::<LoadHandle>()
            .init_resource::<AssetTypeRegistry>()
            .add_resource(asset_server)
            .add_stage_before(
                bevy_app::stage::PRE_UPDATE,
                stage::LOAD_ASSETS,
                SystemStage::parallel(),
            )
            .add_stage_after(
                bevy_app::stage::POST_UPDATE,
                stage::ASSET_EVENTS,
                SystemStage::parallel(),
            )
            .add_system_to_stage(stage::LOAD_ASSETS, AssetServer::process_system.system());
    }
}
