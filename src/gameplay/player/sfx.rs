use crate::gameplay::player::Player;
use bevy::prelude::*;
use bevy_kira_audio::prelude::*;
use std::time::Duration;

#[derive(Resource)]
pub struct SfxEngineRevving(Handle<AudioInstance>);

pub fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    player_query: Query<&Transform, With<Player>>,
) {
    let engine_sound: Handle<AudioSource> = asset_server.load("sfx/car/engine.ogg");
    // Engine
    let engine_revving_handle = audio.play(engine_sound).with_volume(0.0).looped().handle();
    commands.insert_resource(SfxEngineRevving(engine_revving_handle));
}

pub fn engine_revving_system(
    keyboard_input: Res<Input<KeyCode>>,
    player_query: Query<&Transform, With<Player>>,
    handle: Res<SfxEngineRevving>,
    mut audio_instances: ResMut<Assets<AudioInstance>>,
) {
    let engine_volume = 0.2;
    if let Some(instance) = audio_instances.get_mut(&handle.0) {
        let mut rev_speed = 0.0;
        if keyboard_input.any_pressed([KeyCode::W, KeyCode::Up]) {
            rev_speed = 1.0;
            instance.set_volume(
                rev_speed * engine_volume,
                AudioTween::linear(Duration::from_millis(1)),
            );
        } else if keyboard_input.any_pressed([KeyCode::S, KeyCode::Down]) {
            rev_speed = 0.75;
            instance.set_volume(
                rev_speed * engine_volume,
                AudioTween::linear(Duration::from_millis(1)),
            );
        } else if keyboard_input.any_pressed([
            KeyCode::A,
            KeyCode::Left,
            KeyCode::D,
            KeyCode::Right,
        ]) {
            rev_speed = 0.6;
            instance.set_volume(
                rev_speed * engine_volume,
                AudioTween::linear(Duration::from_millis(1)),
            );
        } else {
            instance.set_volume(
                rev_speed * engine_volume,
                AudioTween::linear(Duration::from_millis(100)),
            );
        }
    }
}
