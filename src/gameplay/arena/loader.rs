use crate::gameplay::arena::scene::Scene;
use crate::{App, Input, KeyCode, Plugin, Query, Res, Transform, Vec3};
use bevy::prelude::*;
use bevy::reflect::erased_serde::__private::serde;
use bevy::{math::Vec3Swizzles, time::FixedTimestep};
use bevy_kira_audio::prelude::*;
use bevy_kira_audio::{Audio, AudioControl, MainTrack};
use rand::random;
use std::any::Any;
use std::borrow::BorrowMut;
use std::fs;
use std::ops::DerefMut;
use std::path::Path;
use std::time::Duration;

enum Arenas {
    ChurchCtf,
}

struct ArenaData {
    name: Arenas,
    path: String,
}

#[derive(Component)]
pub struct Arena;

pub fn setup(commands: Commands, asset_server: Res<AssetServer>, audio: Res<Audio>) {
    let arenas = [ArenaData {
        name: Arenas::ChurchCtf,
        path: "/assets/textures/church-ctf.2dtf".to_string(),
    }];

    // Pick a random arena from the list
    let arena_id = random::<usize>() % arenas.len();
    let arena = &arenas[arena_id];

    // Load Scene for that Arena
    let scene: Handle<Scene> = asset_server.load(&arena.path);
}
