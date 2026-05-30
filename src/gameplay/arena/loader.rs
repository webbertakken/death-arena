use crate::gameplay::arena::scene::Scene;
use bevy::prelude::*;

use bevy_kira_audio::Audio;
use rand::random;

struct ArenaData {
    path: String,
}

#[derive(Component)]
pub struct Arena;

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>, audio: Res<Audio>) {
    let arenas = [ArenaData {
        path: "textures/church-ctf.2dtf".to_string(),
    }];

    // Pick a random arena from the list
    let arena_id = random::<usize>() % arenas.len();
    let arena = &arenas[arena_id];

    // Spawn the Arena
    commands.spawn((Arena, Name::new("Arena")));

    // Load Scene for that Arena
    let _scene: Handle<Scene> = asset_server.load(&arena.path);
}
