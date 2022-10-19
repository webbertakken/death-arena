use crate::gameplay::player::Player;
use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use std::time::Duration;

pub struct SfxEngineRevving(Handle<AudioInstance>);

pub fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    player_query: Query<&Transform, With<Player>>,
) {
    // Engine
    let engine_revving_handle = audio
        .play(asset_server.load("sfx/car/engine.ogg"))
        .with_volume(0.01)
        .looped()
        .handle();
    commands.insert_resource(SfxEngineRevving(engine_revving_handle));
}

pub fn engine_revving_system(
    keyboard_input: Res<Input<KeyCode>>,
    player_query: Query<&Transform, With<Player>>,
    handle: Res<SfxEngineRevving>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
) {
    let mut is_revving = false;
    if keyboard_input.any_pressed([KeyCode::Up, KeyCode::W, KeyCode::Down, KeyCode::S]) {
        is_revving = true;
    }

    if let Some(instance) = audio_instances.get_mut(&handle.0) {
        if is_revving {
            instance.set_volume(0.2, AudioTween::linear(Duration::from_millis(1)));
        } else {
            instance.set_volume(0.0, AudioTween::linear(Duration::from_millis(100)));
        }
    }
}
