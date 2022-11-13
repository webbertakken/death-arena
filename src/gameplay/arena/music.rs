use crate::Res;
use bevy::prelude::*;

use bevy_kira_audio::prelude::*;
use bevy_kira_audio::{Audio, AudioControl};

use std::time::Duration;

pub fn setup(commands: Commands, asset_server: Res<AssetServer>, audio: Res<Audio>) {
    let music: Handle<AudioSource> = asset_server.load("music/arena/Funky-Gameplay_v001.mp3");
    audio
        .play(music)
        .fade_in(AudioTween::new(
            Duration::from_secs(5),
            AudioEasing::OutPowi(2),
        ))
        .with_volume(0.25)
        .looped();
}
