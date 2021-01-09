use bevy::prelude::*;
use bevy_atelier::{
    image::Image,
    AssetServer, Assets, Handle, AddAsset
};
use bevy_atelier::AssetPlugin;

fn main() {
    App::build()
    .add_plugin(AssetPlugin)
    .add_asset::<bevy_atelier::image::Image>()
    .add_startup_system(load_the_thing.system())
    .add_system(use_the_thing.system())
    .run();
}


struct ThingHandle(Handle<Image>);
fn load_the_thing(
    commands: &mut Commands,
    asset_server: ResMut<AssetServer>,
) {
    std::thread::sleep(std::time::Duration::from_millis(100));
    let handle:Handle<Image> = asset_server.load("bevy_logo.png");
    println!("{:?}", handle);
    commands.insert_resource(ThingHandle(handle));
}

fn use_the_thing(
    thing_handle: Res<ThingHandle>,
    images: Res<Assets<Image>>,
) {
    println!("Is the image there? {}", images.get(&thing_handle.0).is_some());
}
