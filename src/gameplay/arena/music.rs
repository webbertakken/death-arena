use crate::{App, Input, KeyCode, Plugin, Query, Res, Transform, Vec3};
use bevy::prelude::*;
use bevy::{math::Vec3Swizzles, time::FixedTimestep};
use bevy_kira_audio::prelude::*;
use bevy_kira_audio::{Audio, AudioControl, MainTrack};
use std::any::Any;
use std::time::Duration;

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>, audio: Res<Audio>) {
    let music = asset_server.load("music/arena/Funky-Gameplay_v001.mp3");
    audio
        .play(music)
        .fade_in(AudioTween::new(
            Duration::from_secs(5),
            AudioEasing::OutPowi(2),
        ))
        .with_volume(0.25)
        .looped();
}
