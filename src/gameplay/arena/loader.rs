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

    let arena = &arenas[0];
    let arena_path = format!("textures/arenas/{}", arena.path);
    let floor_path = format!("{}/floor.jpg", arena_path);
    let arena_floor_handle = asset_server.load("textures/arenas/book_ctf_1/floor.jpg");

    // Arena floor
    commands
        .spawn_bundle(SpriteBundle {
            texture: arena_floor_handle,
            ..default()
        })
        .insert(Arena);
}
