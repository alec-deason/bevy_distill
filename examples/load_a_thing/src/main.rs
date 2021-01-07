use bevy::prelude::*;
use bevy_atelier::{
    image::Image,
    AssetServer, Assets, Handle
};
use bevy_atelier::AssetPlugin;

fn main() {
    App::build()
    .add_plugin(AssetPlugin)
    .add_startup_system(load_the_thing.system())
    .add_system(use_the_thing.system())
    .run();
}


struct ThingHandle(Handle<Image>);
fn load_the_thing(
    commands: &mut Commands,
    asset_server: ResMut<AssetServer>,
) {
    let handle:Handle<Image> = asset_server.load("bevy_logo.png").unwrap();
    println!("{:?}", handle);
    commands.insert_resource(ThingHandle(handle));
}

fn use_the_thing(
    thing_handle: Res<ThingHandle>,
    images: Res<Assets<Image>>,
) {
    println!("Is the image there? {}", images.get(&thing_handle.0).is_some());
}
