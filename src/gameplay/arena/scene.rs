use crate::gameplay::arena::scene_loader::Sprite;
use bevy::prelude::Vec3;

#[derive(Debug, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Scale {
    pub x: String,
    pub y: String,
    pub z: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpriteDefaults {
    /// Position in 2D space
    pub position: Position,
    /// Rotation in 2D space (Z-axis)
    pub rotation: f32,
    /// Scale in 2D space
    pub scale: Scale,
    /// Opacity
    pub opacity: f32,
    /// Whether you can drag the sprite
    locked: bool,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpriteData {
    // Unique name of the sprite
    pub id: String,
    /// Relative path to the asset
    pub relative_path: String,
    /// Position in 2D space
    pub position: Position,
    /// Rotation in 2D space (Z-axis)
    pub rotation: f32,
    /// Scale in 2D space
    pub scale: Scale,
    /// Opacity
    pub opacity: f32,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Canvas {
    pub(crate) sprites: Vec<SpriteData>,
}

#[derive(Debug, serde::Deserialize, bevy::reflect::TypeUuid)]
#[serde(rename_all = "camelCase")]
#[uuid = "413be529-bfff-f1b3-9db0-4b8b380a2c46"]
pub struct Scene {
    pub name: String,
    pub version: String,
    pub description: String,
    pub assets_relative_path: String,
    pub canvas: Canvas,
    pub default_properties: SpriteDefaults,
}
