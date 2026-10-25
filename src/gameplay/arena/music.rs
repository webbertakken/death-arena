use crate::app::audio::{AudioOutput, Easing, Playback, Sound, Tween};
use bevy::prelude::*;

use std::time::Duration;

pub fn setup(asset_server: Res<AssetServer>, mut audio: NonSendMut<AudioOutput>) {
    let music: Handle<Sound> = asset_server.load("music/arena/Funky-Gameplay_v001.mp3");
    audio.play(
        music,
        Playback {
            volume: 0.25,
            looped: true,
            fade_in: Some(Tween {
                duration: Duration::from_secs(5),
                easing: Easing::OutPowi(2),
                ..Tween::default()
            }),
        },
    );
}
