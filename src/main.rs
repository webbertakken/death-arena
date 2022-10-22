#![allow(dead_code, unused_variables, unused_imports)]
use bevy::prelude::*;
use bevy_inspector_egui::WorldInspectorPlugin;
use bevy_kira_audio::prelude::*;
use gameplay::GameplayPlugins;

mod core;
mod gameplay;

fn main() {
    core::init();
    App::new()
        .insert_resource(WindowDescriptor {
            width: 1400.0,
            height: 800.0,
            title: "Death Arena".to_string(),
            canvas: Some("#game".to_owned()),
            fit_canvas_to_parent: true,
            ..default()
        })
        .add_plugins(DefaultPlugins)
        .add_plugins(GameplayPlugins)
        .add_plugin(WorldInspectorPlugin::new())
        .run();
}
