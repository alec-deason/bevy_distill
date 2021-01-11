use atelier_loader::handle::Handle;
use bevy::{
    app::{AppExit, ScheduleRunnerSettings},
    prelude::*,
    reflect::ReflectPlugin,
    utils::Duration,
};
use bevy_atelier::AssetPlugin;
use bevy_atelier::{image::Image, AddAsset, AssetServer, AssetServerSettings, Assets};

fn main() {
    let mut app = App::build();
    app
        // Try creating a packfile using atelier-cli and uncommenting this line
        // .add_resource(AssetServerSettings::default_packfile())
        .add_plugins(MinimalPlugins)
        .add_resource(ScheduleRunnerSettings::run_loop(Duration::from_secs_f64(
            1.0 / 60.0,
        )))
        .add_resource(bevy::log::LogSettings {
            level: bevy::log::Level::INFO,
            ..Default::default()
        })
        .add_plugin(AssetPlugin)
        .add_asset::<bevy_atelier::image::Image>()
        .add_startup_system(load_the_thing.system())
        .add_system(use_the_thing.system());
    if cfg!(feature = "atelier-daemon-headless") {
        loop {}
    } else {
        app.run();
    }
}

struct ThingHandle(Handle<Image>);
fn load_the_thing(commands: &mut Commands, asset_server: ResMut<AssetServer>) {
    std::thread::sleep(std::time::Duration::from_millis(100));
    let handle: Handle<Image> = asset_server.load("bevy_logo.png");
    println!("{:?}", handle);
    commands.insert_resource(ThingHandle(handle));
}

fn use_the_thing(
    thing_handle: Res<ThingHandle>,
    images: Res<Assets<Image>>,
    mut app_exit: ResMut<Events<AppExit>>,
) {
    if let Some(image) = images.get(&thing_handle.0) {
        println!("Asset found!");
        app_exit.send(AppExit);
    }
}
