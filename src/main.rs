use bevy::prelude::*;
use gameplay::GameplayPlugins;

mod gameplay;

fn main() {
    App::new()
        .insert_resource(WindowDescriptor {
            width: 1400.0,
            height: 800.0,
            title: "Death Arena".to_string(),
            canvas: Some("#game".to_owned()),
            ..Default::default()
        })
        .add_plugins(DefaultPlugins)
        .add_plugins(GameplayPlugins)
        .run();
}
