use bevy::app::PluginGroupBuilder;
use bevy::log::LogPlugin;
use bevy::prelude::*;

pub trait Configure {
    fn configure() -> PluginGroupBuilder;
}

impl Configure for DefaultPlugins {
    fn configure() -> PluginGroupBuilder {
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: (1400.0, 800.0).into(),
                    title: "Death Arena".to_string(),
                    canvas: Some("#game".to_owned()),
                    fit_canvas_to_parent: true,
                    ..default()
                }),
                ..default()
            })
            .set(LogPlugin {
                filter: "info,bevy_render=0,symphonia_core=warn,symphonia_format_ogg=warn,symphonia_bundle_mp3=warn,wgpu_core=warn,wgpu_hal=warn".into(),
                level: bevy::log::Level::DEBUG,
                ..default()
            })
            .set(ImagePlugin::default_nearest())
    }
}
