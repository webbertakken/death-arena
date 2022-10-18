use crate::{App, Input, KeyCode, Plugin, Query, Res, Transform, Vec3};
use bevy::prelude::*;
use bevy::{math::Vec3Swizzles, time::FixedTimestep};
use bevy_kira_audio::prelude::*;
use bevy_kira_audio::{Audio, AudioControl, MainTrack};
use std::any::Any;
use std::time::Duration;

enum Arenas {
    BookCtf1,
}

struct ArenaData {
    name: Arenas,
    path: String,
}

#[derive(Component)]
pub struct Arena;

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>, audio: Res<Audio>) {
    let arenas = [ArenaData {
        name: Arenas::BookCtf1,
        path: "book_ctf_1".to_string(),
    }];

    let music = asset_server.load("music/arena/Funky-Gameplay_v001.mp3");
    audio
        .play(music)
        .fade_in(AudioTween::new(
            Duration::from_secs(5),
            AudioEasing::OutPowi(2),
        ))
        .with_volume(0.25)
        .looped();

    let arena = &arenas[0];
    let arena_path = format!("textures/arenas/{}", arena.path);
    let floor_path = format!("{}/floor.jpg", arena_path);
    let arena_floor_handle = asset_server.load("textures/arenas/book_ctf_1/floor.jpg");

    // 2D orthographic camera
    commands.spawn_bundle(Camera2dBundle::default());

    // Arena floor
    commands
        .spawn_bundle(SpriteBundle {
            texture: arena_floor_handle,
            ..default()
        })
        .insert(Arena);
}
