mod asset_server;
mod asset_type_registry;
mod assets;
mod handle;
mod load_request;
mod loader;
pub mod image;

pub use asset_server::*;
use asset_type_registry::*;
pub use assets::*;
pub use handle::*;
pub use load_request::*;
pub use loader::*;


/// The names of asset stages in an App Schedule
pub mod stage {
    pub const LOAD_ASSETS: &str = "load_assets";
    pub const ASSET_EVENTS: &str = "asset_events";
}

pub mod prelude {
    pub use crate::{AddAsset, AssetEvent, AssetServer, Assets, Handle, HandleUntyped};
}
pub use atelier_core::AssetTypeId;

use bevy_app::{prelude::Plugin, AppBuilder};
use bevy_ecs::{IntoSystem, SystemStage};

/// Adds support for Assets to an App. Assets are typed collections with change tracking, which are added as App Resources.
/// Examples of assets: textures, sounds, 3d models, maps, scenes
#[derive(Default)]
pub struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.add_stage_before(
            bevy_app::stage::PRE_UPDATE,
            stage::LOAD_ASSETS,
            SystemStage::parallel(),
        )
        .add_stage_after(
            bevy_app::stage::POST_UPDATE,
            stage::ASSET_EVENTS,
            SystemStage::parallel(),
        )
        .init_resource::<AssetServer>()
        .init_resource::<AssetTypeRegistry>()
        .add_system_to_stage(stage::LOAD_ASSETS, AssetServer::process_system.system());
    }
}
