use crate::gameplay::arena::loader::Arena;
use crate::gameplay::arena::scene::{Position, Scale, Scene, SpriteData};
use crate::gameplay::main::BOUNDS;
use crate::gameplay::GameState;
use crate::AppState;
use bevy::prelude::*;
use bevy::utils::HashSet;
use bevy::{
    asset::{AssetLoader, Handle, LoadContext, LoadedAsset},
    prelude::*,
    reflect::TypeUuid,
    utils::BoxedFuture,
};
use serde::Deserialize;
use serde_json::from_slice;
use std::default::Default;
use std::iter::Map;

#[derive(Default)]
pub struct SceneLoader;

impl AssetLoader for SceneLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            let scene_asset = from_slice::<Scene>(bytes)?;
            load_context.set_default_asset(LoadedAsset::new(scene_asset));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["2dtf"]
    }
}

#[derive(Debug)]
pub struct Sprite {
    pub handle: Handle<Image>,
    /// Unique identifier for this sprite instance
    pub id: String,
    /// Position in 2D space
    pub position: Position,
    /// Rotation in 2D space (Z-axis)
    pub rotation: f32,
    /// Scale in 2D space
    pub scale: Scale,
    /// Opacity
    pub opacity: f32,
    /// Name
    pub name: String,
}

impl From<&SpriteData> for Sprite {
    fn from(sprite_data: &SpriteData) -> Self {
        Self {
            handle: Handle::default(),
            id: sprite_data.id.clone(),
            position: sprite_data.position.clone(),
            rotation: sprite_data.rotation,
            scale: sprite_data.scale.clone(),
            opacity: sprite_data.opacity,
            name: sprite_data.relative_path.clone(),
        }
    }
}

#[derive(Default)]
pub struct SceneState {
    pub handle: Handle<Scene>,
    pub printed: bool,
    pub sprites_loading_started: bool,
    pub sprites_loading_finished: bool,
    pub handles: Vec<Sprite>,
    pub paths: HashSet<String>,
}

pub fn load(mut state: ResMut<SceneState>, asset_server: Res<AssetServer>) {
    state.handle = asset_server.load("textures/church-ctf.2dtf");
}

pub fn load_sprites_from_scene(
    mut commands: Commands,
    mut state: ResMut<SceneState>,
    scenes: ResMut<Assets<Scene>>,
    asset_server: Res<AssetServer>,
) {
    // Only load the sprites once, after scene file is loaded
    let scene = scenes.get(&state.handle);
    if state.sprites_loading_started || scene.is_none() {
        return;
    }

    // Scene
    let scene = scene.unwrap();
    info!("Loading scene: {:?}", &scene.name);

    // Sprites
    state.paths = scene
        .canvas
        .sprites
        .iter()
        .map(|sprite| -> String {
            let file_path = format!("textures/{}", &sprite.relative_path);

            let sprite = Sprite {
                handle: asset_server.load(&file_path),
                ..sprite.into()
            };

            // info!("Loading {:?}", sprite);
            state.handles.push(sprite);

            file_path
        })
        .collect::<HashSet<String>>();

    info!("Loading {} sprites", &state.handles.len());

    /////
    // Debug
    /////

    for sprite in &state.handles {
        // Arena floor
        commands
            .spawn_bundle(SpriteBundle {
                texture: sprite.handle.clone(),
                sprite: bevy::sprite::Sprite {
                    anchor: bevy::sprite::Anchor::Center,
                    ..default()
                },
                transform: Transform {
                    translation: Vec3::new(
                        -BOUNDS.x / 2.0 + sprite.position.x,
                        BOUNDS.y - sprite.position.y,
                        sprite.position.z,
                    ),
                    scale: Vec3::new(
                        sprite.scale.x.parse::<f32>().unwrap(),
                        sprite.scale.y.parse::<f32>().unwrap(),
                        1.0,
                    ),
                    rotation: Quat::from_rotation_z(sprite.rotation),
                },
                ..default()
            })
            .insert(Name::new(sprite.name.clone()));
    }

    // Mark as started
    state.sprites_loading_started = true;
}

pub fn move_to_next_state(
    mut state: ResMut<SceneState>,
    mut app_state: ResMut<State<AppState>>,
    images: ResMut<Assets<Image>>,
) {
    if !state.sprites_loading_started || state.sprites_loading_finished {
        return;
    }

    // todo - filter only for arena sprites
    let current = images.len();
    let total = state.paths.len();
    info!("Loaded {} of {}", current, total);

    // Todo - actually check it
    if total > 0 && current >= total {
        info!("All images loaded");
        app_state.overwrite_set(AppState::InGame).unwrap();
        // game_state.overwrite_set(GameState::Intro).unwrap();
        state.sprites_loading_finished = true;
    }
}
