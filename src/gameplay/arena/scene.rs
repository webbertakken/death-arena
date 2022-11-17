#![allow(dead_code)]

use crate::core::serde::parse_float;

#[derive(Debug, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    #[serde(deserialize_with = "parse_float")]
    pub x: f32,
    #[serde(deserialize_with = "parse_float")]
    pub y: f32,
    #[serde(deserialize_with = "parse_float")]
    pub z: f32,
}

#[derive(Debug, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Scale {
    #[serde(deserialize_with = "parse_float")]
    pub x: f32,
    #[serde(deserialize_with = "parse_float")]
    pub y: f32,
    #[serde(deserialize_with = "parse_float")]
    pub z: f32,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpriteDefaults {
    /// Position in 2D space
    pub position: Position,
    /// Rotation in 2D space (Z-axis)
    #[serde(deserialize_with = "parse_float")]
    pub rotation: f32,
    /// Scale in 2D space
    pub scale: Scale,
    /// Opacity
    #[serde(deserialize_with = "parse_float")]
    pub opacity: f32,
    /// Whether you can drag the sprite
    locked: bool,
    // Whether it can move as an object or not.
    pub is_static: bool,
    // Weight
    pub use_size_for_weight: bool,
    #[serde(deserialize_with = "parse_float")]
    pub size_to_weight_multiplier: f32,
    #[serde(deserialize_with = "parse_float")]
    pub weight: f32,
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
    #[serde(deserialize_with = "parse_float")]
    pub rotation: f32,
    /// Scale in 2D space
    pub scale: Scale,
    /// Opacity
    #[serde(deserialize_with = "parse_float")]
    pub opacity: f32,
    // Whether it can move as an object or not.
    pub is_static: bool,
    // Weight
    pub use_size_for_weight: bool,
    #[serde(deserialize_with = "parse_float")]
    pub size_to_weight_multiplier: f32,
    #[serde(deserialize_with = "parse_float")]
    pub weight: f32,
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
