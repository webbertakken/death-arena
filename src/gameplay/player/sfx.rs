use crate::app::audio::{AudioOutput, Playback, Sound, SoundInstance};
use bevy::prelude::*;
use std::time::Duration;

#[derive(Resource)]
pub struct SfxEngineRevving(SoundInstance);

pub fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut audio: NonSendMut<AudioOutput>,
) {
    let engine_sound: Handle<Sound> = asset_server.load("sfx/car/engine.ogg");
    // Engine
    let engine_revving = audio.play(
        engine_sound,
        Playback {
            volume: 0.0,
            looped: true,
            fade_in: None,
        },
    );
    commands.insert_resource(SfxEngineRevving(engine_revving));
}

pub fn engine_revving_system(
    keyboard_input: Res<Input<KeyCode>>,
    engine: Res<SfxEngineRevving>,
    mut audio: NonSendMut<AudioOutput>,
) {
    let engine_volume = 0.2;
    let (rev_speed, fade) = if keyboard_input.any_pressed([KeyCode::W, KeyCode::Up]) {
        (1.0, Duration::from_millis(1))
    } else if keyboard_input.any_pressed([KeyCode::S, KeyCode::Down]) {
        (0.75, Duration::from_millis(1))
    } else if keyboard_input.any_pressed([KeyCode::A, KeyCode::Left, KeyCode::D, KeyCode::Right]) {
        (0.6, Duration::from_millis(1))
    } else {
        (0.0, Duration::from_millis(100))
    };
    audio.set_volume(engine.0, rev_speed * engine_volume, fade);
}
