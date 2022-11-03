use bevy::app::{App, Plugin};
use bevy::asset::AssetServer;
use bevy::log::LogSettings;
use bevy::prelude::*;

#[derive(Default)]
pub struct InitPlugin;

impl Plugin for InitPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(window_resource())
            .insert_resource(log_settings())
            .insert_resource(background_color())
            .add_system(setup);
    }
}

fn background_color() -> ClearColor {
    ClearColor(Color::rgb(0.1, 0.1, 0.27))
}

fn log_settings() -> LogSettings {
    LogSettings {
        filter: "info,bevy_render=0,symphonia_core=warn,symphonia_format_ogg=warn,symphonia_bundle_mp3=warn,wgpu_core=warn,wgpu_hal=warn".into(),
        level: bevy::log::Level::DEBUG,
    }
}

fn setup(commands: Commands, asset_server: Res<AssetServer>) {
    println!("Hello, world! from setup");
}

fn window_resource() -> WindowDescriptor {
    WindowDescriptor {
        width: 1400.0,
        height: 800.0,
        title: "Death Arena".to_string(),
        canvas: Some("#game".to_owned()),
        fit_canvas_to_parent: true,
        ..default()
    }
}
