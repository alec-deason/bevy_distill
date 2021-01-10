use crate::{AssetTypeId, Assets, AssetsRefCell};
use bevy_ecs::{Resource, Resources};
use std::cell::RefCell;
use std::collections::HashMap;
use type_uuid::TypeUuid;
use serde::de::DeserializeOwned;

pub(crate) struct AssetRegistration {
    pub ty: AssetTypeId,
    pub get_assets_storage_fn:
        fn(&Resources, &mut dyn FnMut(&dyn atelier_loader::storage::AssetStorage)),
    // component_add_fn: fn(&mut World, resources: &Resources, Entity, &dyn Property),
    // component_apply_fn: fn(&mut World, Entity, &dyn Property),
    // component_properties_fn: fn(&Archetype, usize) -> &dyn Properties,
}

impl AssetRegistration {
    pub fn of<T: TypeUuid + Resource + DeserializeOwned>() -> Self {
        Self {
            ty: AssetTypeId(<T as TypeUuid>::UUID),
            get_assets_storage_fn: |resources, cb| {
                let mut asset_storage = resources
                    .get_mut::<Assets<T>>()
                    .expect("Asset storage not found");
                let asset_storage_cell = AssetsRefCell(RefCell::new(&mut *asset_storage));
                cb(&asset_storage_cell)
            },
        }
    }
}

#[derive(Default)]
pub(crate) struct AssetTypeRegistry {
    pub registrations: HashMap<AssetTypeId, AssetRegistration>,
}

impl AssetTypeRegistry {
    pub fn register<T: Resource + TypeUuid + DeserializeOwned>(&mut self) {
        self.registrations.insert(
            AssetTypeId(<T as TypeUuid>::UUID),
            AssetRegistration::of::<T>(),
        );
    }
}
