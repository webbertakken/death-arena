use crate::app::physics::collider::ColliderData;

use crate::gameplay::arena::scene::{Position, Scale, Scene, SpriteData};
use crate::gameplay::main::BOUNDS;

use crate::AppState;
use bevy::prelude::*;
use bevy::utils::HashSet;
use bevy::{
    asset::{AssetLoader, Handle, LoadContext, LoadedAsset},
    utils::BoxedFuture,
};
use bevy_rapier2d::prelude::*;

use serde_json::from_slice;
use std::default::Default;

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
    /// Handle for the sprite
    pub sprite_handle: Handle<Image>,
    /// Handle for the sprite's collider definition
    pub collider_handle: Handle<ColliderData>,
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
    // Whether it can move as an object or not.
    pub is_static: bool,
    // Weight
    pub use_size_for_weight: bool,
    pub size_to_weight_multiplier: f32,
    pub weight: f32,
}

impl From<&SpriteData> for Sprite {
    fn from(sprite_data: &SpriteData) -> Self {
        Self {
            sprite_handle: Handle::default(),
            collider_handle: Handle::default(),
            id: sprite_data.id.clone(),
            position: sprite_data.position.clone(),
            rotation: sprite_data.rotation,
            scale: sprite_data.scale.clone(),
            opacity: sprite_data.opacity,
            name: sprite_data.relative_path.clone(),
            is_static: sprite_data.is_static,
            use_size_for_weight: sprite_data.use_size_for_weight,
            size_to_weight_multiplier: sprite_data.size_to_weight_multiplier,
            weight: sprite_data.weight,
        }
    }
}

#[derive(Default, Resource)]
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

            // Remove the file extension (png, jpg, etc.)
            let file_path_without_ext = file_path.split('.').next().unwrap().to_string();
            let collider_file_path = format!("{}.collider", file_path_without_ext);

            let sprite = Sprite {
                sprite_handle: asset_server.load(&file_path),
                collider_handle: asset_server.load(&collider_file_path),
                ..sprite.into()
            };

            // info!("Loading {:?}", sprite);
            state.handles.push(sprite);

            file_path
        })
        .collect::<HashSet<String>>();

    info!("Loading {} sprites", &state.handles.len());

    // Mark as started
    state.sprites_loading_started = true;
}

pub fn move_to_next_state(
    mut commands: Commands,
    mut state: ResMut<SceneState>,
    mut app_state: ResMut<State<AppState>>,
    images: ResMut<Assets<Image>>,
    collider_assets: ResMut<Assets<ColliderData>>,
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

        for sprite in &state.handles {
            // Spawn the object
            let mut my_handle = commands.spawn_empty();

            // Main components
            my_handle.insert((
                Name::new(sprite.name.clone()),
                SpriteBundle {
                    texture: sprite.sprite_handle.clone(),
                    sprite: bevy::sprite::Sprite {
                        anchor: bevy::sprite::Anchor::Center,
                        ..default()
                    },
                    transform: Transform {
                        translation: Vec3::new(
                            -BOUNDS.x / 2.0 + sprite.position.x,
                            BOUNDS.y / 2.0 - sprite.position.y,
                            sprite.position.z,
                        ),
                        scale: Vec3::new(sprite.scale.x, sprite.scale.y, 1.0),
                        rotation: Quat::from_rotation_z(-sprite.rotation.to_radians()),
                    },
                    ..default()
                },
            ));

            // Collider
            match collider_assets.get(&sprite.collider_handle) {
                Some(ColliderData::Poly(collider_data)) => {
                    my_handle.insert(
                        Collider::convex_polyline(collider_data.clone()).unwrap_or_default(),
                    );
                }
                Some(ColliderData::NoCollider) => {
                    warn!("Sprite without collider: {}", sprite.name);
                }
                None => {
                    warn!("collider_data isn't loaded yet");
                }
            };

            // Body
            my_handle.insert(if sprite.is_static {
                RigidBody::Fixed
            } else {
                RigidBody::Dynamic
            });
        }

        app_state.overwrite_set(AppState::InGame).unwrap();
        state.sprites_loading_finished = true;
    }
}
