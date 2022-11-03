use bevy::app::{App, Plugin};
use bevy::asset::AssetServer;
use bevy::prelude::{default, Commands, Res, WindowDescriptor};

#[derive(Default)]
pub struct SetupPlugin;

impl Plugin for SetupPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(create_window_resource())
            .add_startup_system(setup);
    }
}

pub fn setup(commands: Commands, asset_server: Res<AssetServer>) {
    println!("Hello, world!");
}

fn create_window_resource() -> WindowDescriptor {
    WindowDescriptor {
        width: 1400.0,
        height: 800.0,
        title: "Death Arena".to_string(),
        canvas: Some("#game".to_owned()),
        fit_canvas_to_parent: true,
        ..default()
    }
}
